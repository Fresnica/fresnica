import assert from "node:assert/strict";
import test from "node:test";

import {
  WalletAccountProvisioningCoordinator,
  type FresnicaAccountProvisioningBridge,
  type ProvisioningGeneratedMnemonic,
  type ProvisioningProtectedSigner,
} from "../src/account-provisioning.ts";
import {
  type AccountRecord,
  type AccountSignerReference,
  type ProtectedSoftwareSignerRecord,
  type WalletStore,
  type WalletStoreTransaction,
} from "../src/wallet-lifecycle.ts";

class MemoryStore implements WalletStore {
  accounts = new Map<string, AccountRecord>();
  signers = new Map<string, ProtectedSoftwareSignerRecord>();
  references: AccountSignerReference[] = [];

  async getAccount(accountId: string): Promise<AccountRecord | null> {
    return this.accounts.get(accountId) ?? null;
  }

  async listProtectedSoftwareSigners(): Promise<readonly ProtectedSoftwareSignerRecord[]> {
    return [...this.signers.values()];
  }

  async listSignerReferencesForAccount(accountId: string): Promise<readonly AccountSignerReference[]> {
    return this.references.filter((reference) => reference.accountId === accountId);
  }

  async transaction<T>(work: (transaction: WalletStoreTransaction) => T): Promise<T> {
    const accountsBefore = new Map(this.accounts);
    const signersBefore = new Map(this.signers);
    const referencesBefore = [...this.references];
    const transaction: WalletStoreTransaction = {
      getAccount: (id) => this.accounts.get(id) ?? null,
      getSigner: (id) => this.signers.get(id) ?? null,
      listSignerReferencesForAccount: (id) =>
        this.references.filter((reference) => reference.accountId === id),
      countSignerReferences: (id) =>
        this.references.filter((reference) => reference.signerId === id).length,
      putAccount: (account) => void this.accounts.set(account.id, account),
      putSigner: (signer) => void this.signers.set(signer.id, signer),
      putSignerReference: (reference) => void this.references.push(reference),
      deleteSignerReference: () => undefined,
      deleteSigner: () => undefined,
    };
    try {
      return work(transaction);
    } catch (error) {
      this.accounts = accountsBefore;
      this.signers = signersBefore;
      this.references = referencesBefore;
      throw error;
    }
  }
}

class FakeCore implements FresnicaAccountProvisioningBridge {
  protectedSigner: ProvisioningProtectedSigner = {
    signerPublicKey: "GMASTER",
    envelopeJson: '{"protected":true}',
  };
  generated: ProvisioningGeneratedMnemonic = {
    signer: this.protectedSigner,
    mnemonic: "one two three four",
    language: "english",
    index: 0,
  };
  protectSecretCalls: unknown[][] = [];
  protectMnemonicCalls: unknown[][] = [];

  async parseAccount(address: string) {
    return { kind: "classic" as const, address, publicKey: address };
  }

  async protectSecret(secret: string, appPasscode: string, expected: string | null) {
    this.protectSecretCalls.push([secret, appPasscode, expected]);
    return this.protectedSigner;
  }

  async protectMnemonic(
    mnemonic: string,
    mnemonicPassphrase: string,
    index: number,
    language: string | null,
    appPasscode: string,
    expected: string | null,
  ) {
    this.protectMnemonicCalls.push([
      mnemonic,
      mnemonicPassphrase,
      index,
      language,
      appPasscode,
      expected,
    ]);
    return this.protectedSigner;
  }

  async generateMnemonic(): Promise<ProvisioningGeneratedMnemonic> {
    return this.generated;
  }
}

function ids(...values: string[]) {
  const queue = [...values];
  return () => {
    const value = queue.shift();
    assert.notEqual(value, undefined);
    return value as string;
  };
}

test("secret import persists account, signer and reference atomically", async () => {
  const core = new FakeCore();
  const store = new MemoryStore();
  const coordinator = new WalletAccountProvisioningCoordinator(
    core,
    store,
    ids("account-1", "signer-1"),
  );

  const result = await coordinator.importSecretAccount({
    secret: "SSECRET",
    appPasscode: "2468",
    network: " public ",
    name: " Main ",
  });

  assert.deepEqual(core.protectSecretCalls, [["SSECRET", "2468", null]]);
  assert.deepEqual(result.account, {
    id: "account-1",
    address: "GMASTER",
    kind: "classic",
    network: "public",
    name: "Main",
  });
  assert.equal(store.signers.get("signer-1")?.envelopeJson, '{"protected":true}');
  assert.deepEqual(store.references, [{ accountId: "account-1", signerId: "signer-1" }]);
});

test("mnemonic import sends no expected account identity and persists no mnemonic plaintext", async () => {
  const core = new FakeCore();
  const store = new MemoryStore();
  const coordinator = new WalletAccountProvisioningCoordinator(
    core,
    store,
    ids("account-1", "signer-1"),
  );

  await coordinator.importMnemonicAccount({
    mnemonic: "one two three four",
    mnemonicPassphrase: "extra",
    index: 7,
    language: "english",
    appPasscode: "1357",
    network: "testnet",
  });

  assert.deepEqual(core.protectMnemonicCalls, [
    ["one two three four", "extra", 7, "english", "1357", null],
  ]);
  const persisted = JSON.stringify({
    accounts: [...store.accounts.values()],
    signers: [...store.signers.values()],
    references: store.references,
  });
  assert.equal(persisted.includes("one two three four"), false);
  assert.equal(persisted.includes("extra"), false);
});

test("generated mnemonic is returned once while persistence contains only the protected envelope", async () => {
  const core = new FakeCore();
  const store = new MemoryStore();
  core.generated = {
    signer: {
      signerPublicKey: "GGENERATED",
      envelopeJson: '{"generated":"protected"}',
    },
    mnemonic: "alpha beta gamma delta",
    language: "english",
    index: 3,
  };
  const coordinator = new WalletAccountProvisioningCoordinator(
    core,
    store,
    ids("account-1", "signer-1"),
  );

  const result = await coordinator.generateMnemonicAccount({
    language: "english",
    strength: 128,
    mnemonicPassphrase: "",
    index: 3,
    appPasscode: "2468",
    network: "public",
  });

  assert.equal(result.mnemonic, "alpha beta gamma delta");
  assert.equal(result.account.address, "GGENERATED");
  const persisted = JSON.stringify([...store.signers.values()]);
  assert.equal(persisted.includes("alpha beta gamma delta"), false);
  assert.equal(store.signers.get("signer-1")?.envelopeJson, '{"generated":"protected"}');
});

test("record collision rolls back the whole account/signer/reference graph", async () => {
  const core = new FakeCore();
  const store = new MemoryStore();
  store.signers.set("signer-existing", {
    id: "signer-existing",
    kind: "protected-software",
    signerPublicKey: "GEXISTING",
    envelopeJson: "existing",
  });
  const coordinator = new WalletAccountProvisioningCoordinator(
    core,
    store,
    ids("account-new", "signer-existing"),
  );

  await assert.rejects(
    coordinator.importSecretAccount({
      secret: "SSECRET",
      appPasscode: "2468",
      network: "public",
    }),
    /Signer record id already exists/,
  );

  assert.equal(store.accounts.has("account-new"), false);
  assert.equal(store.references.length, 0);
  assert.equal(store.signers.get("signer-existing")?.envelopeJson, "existing");
});
