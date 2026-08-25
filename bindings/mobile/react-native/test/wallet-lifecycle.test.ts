import assert from "node:assert/strict";
import test from "node:test";

import {
  WalletLifecycleCoordinator,
  WalletLifecycleError,
  type AccountRecord,
  type AccountSignerReference,
  type FresnicaCoreLifecycleBridge,
  type NativeAccountIdentity,
  type NativeProtectedSoftwareSigner,
  type ProtectedSoftwareSignerRecord,
  type WalletStore,
  type WalletStoreTransaction,
} from "../src/wallet-lifecycle.ts";

class MemoryStore implements WalletStore {
  accounts = new Map<string, AccountRecord>();
  signers = new Map<string, ProtectedSoftwareSignerRecord>();
  references: AccountSignerReference[] = [];
  beforeTransaction: (() => void) | null = null;

  async getAccount(accountId: string): Promise<AccountRecord | null> {
    return this.accounts.get(accountId) ?? null;
  }

  async listProtectedSoftwareSigners(): Promise<readonly ProtectedSoftwareSignerRecord[]> {
    return [...this.signers.values()].map((signer) => ({ ...signer }));
  }

  async listSignerReferencesForAccount(
    accountId: string,
  ): Promise<readonly AccountSignerReference[]> {
    return this.references.filter((reference) => reference.accountId === accountId);
  }

  async transaction<T>(work: (transaction: WalletStoreTransaction) => T): Promise<T> {
    this.beforeTransaction?.();
    this.beforeTransaction = null;
    const accountsBefore = new Map(this.accounts);
    const signersBefore = new Map(this.signers);
    const referencesBefore = [...this.references];

    const transaction: WalletStoreTransaction = {
      getAccount: (accountId) => this.accounts.get(accountId) ?? null,
      getSigner: (signerId) => this.signers.get(signerId) ?? null,
      listSignerReferencesForAccount: (accountId) =>
        this.references.filter((reference) => reference.accountId === accountId),
      countSignerReferences: (signerId) =>
        this.references.filter((reference) => reference.signerId === signerId).length,
      putAccount: (account) => {
        this.accounts.set(account.id, account);
      },
      putSigner: (signer) => {
        this.signers.set(signer.id, signer);
      },
      putSignerReference: (reference) => {
        this.references.push(reference);
      },
      deleteSignerReference: (accountId, signerId) => {
        this.references = this.references.filter(
          (reference) =>
            reference.accountId !== accountId || reference.signerId !== signerId,
        );
      },
      deleteSigner: (signerId) => {
        this.signers.delete(signerId);
      },
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

class FakeCore implements FresnicaCoreLifecycleBridge {
  identities = new Map<string, NativeAccountIdentity>();
  protectedSigner: NativeProtectedSoftwareSigner = {
    signerPublicKey: "GMASTER",
    envelopeJson: '{"protected":true}',
  };
  systemAuth = new Set<string>();
  failCleanupFor = new Set<string>();
  protectSecretCalls: unknown[][] = [];
  protectMnemonicCalls: unknown[][] = [];
  reprotectCalls: unknown[][] = [];
  protectSecretError: Error | null = null;
  reprotectErrorFor = new Set<string>();

  async parseAccount(address: string): Promise<NativeAccountIdentity> {
    const identity = this.identities.get(address);
    if (identity === undefined) {
      throw new Error(`Unknown test identity: ${address}`);
    }
    return identity;
  }

  async protectSecret(
    secret: string,
    appPasscode: string,
    expectedSignerPublicKey: string,
  ): Promise<NativeProtectedSoftwareSigner> {
    this.protectSecretCalls.push([secret, appPasscode, expectedSignerPublicKey]);
    if (this.protectSecretError !== null) {
      throw this.protectSecretError;
    }
    return this.protectedSigner;
  }

  async protectMnemonic(
    mnemonic: string,
    mnemonicPassphrase: string,
    index: number,
    language: string | null,
    appPasscode: string,
    expectedSignerPublicKey: string,
  ): Promise<NativeProtectedSoftwareSigner> {
    this.protectMnemonicCalls.push([
      mnemonic,
      mnemonicPassphrase,
      index,
      language,
      appPasscode,
      expectedSignerPublicKey,
    ]);
    return this.protectedSigner;
  }

  async reprotect(
    envelopeJson: string,
    currentPasscode: string,
    newPasscode: string,
    expectedSignerPublicKey: string,
  ): Promise<NativeProtectedSoftwareSigner> {
    this.reprotectCalls.push([
      envelopeJson,
      currentPasscode,
      newPasscode,
      expectedSignerPublicKey,
    ]);
    if (this.reprotectErrorFor.has(expectedSignerPublicKey)) {
      throw new Error(`reprotect failed for ${expectedSignerPublicKey}`);
    }
    return {
      signerPublicKey: expectedSignerPublicKey,
      envelopeJson: `${envelopeJson}:new`,
    };
  }

  async hasSystemAuth(expectedSignerPublicKey: string): Promise<boolean> {
    if (this.failCleanupFor.has(expectedSignerPublicKey)) {
      throw new Error("native auth lookup failed");
    }
    return this.systemAuth.has(expectedSignerPublicKey);
  }

  async removeSystemAuth(expectedSignerPublicKey: string): Promise<true> {
    if (this.failCleanupFor.has(expectedSignerPublicKey)) {
      throw new Error("native auth removal failed");
    }
    this.systemAuth.delete(expectedSignerPublicKey);
    return true;
  }
}

function ids(...values: string[]): (kind: "account" | "signer") => string {
  const queue = [...values];
  return () => {
    const next = queue.shift();
    assert.notEqual(next, undefined, "test id queue exhausted");
    return next as string;
  };
}

function setupWatchOnly(kind: "classic" | "contract" = "classic") {
  const core = new FakeCore();
  const store = new MemoryStore();
  const address = kind === "classic" ? "GACCOUNT" : "CCONTRACT";
  core.identities.set(address, {
    kind,
    address,
    publicKey: kind === "classic" ? address : null,
  });
  if (kind === "classic") {
    core.protectedSigner = {
      signerPublicKey: address,
      envelopeJson: '{"protected":true}',
    };
  }
  const coordinator = new WalletLifecycleCoordinator(core, store, ids("account-1", "signer-1"));
  return { core, store, coordinator, address };
}

function signer(id: string, publicKey: string, envelopeJson: string): ProtectedSoftwareSignerRecord {
  return {
    id,
    kind: "protected-software",
    signerPublicKey: publicKey,
    envelopeJson,
  };
}

test("watch-only account persists only account identity and derives local-signer state", async () => {
  const { coordinator, store, address } = setupWatchOnly();

  const account = await coordinator.addWatchOnly({
    address,
    network: " public ",
    name: "  Savings  ",
  });

  assert.deepEqual(account, {
    id: "account-1",
    address: "GACCOUNT",
    kind: "classic",
    network: "public",
    name: "Savings",
  });
  assert.equal(store.signers.size, 0);
  assert.equal(store.references.length, 0);
  assert.equal(await coordinator.hasLocalSigner(account.id), false);
});

test("classic watch-only secret upgrade asks Core to verify the existing account identity", async () => {
  const { coordinator, core, store, address } = setupWatchOnly();
  const account = await coordinator.addWatchOnly({ address, network: "public", name: "Keep me" });
  core.protectedSigner = {
    signerPublicKey: address,
    envelopeJson: '{"new":"envelope"}',
  };

  const upgraded = await coordinator.upgradeClassicWatchOnlyWithSecret(
    account.id,
    "SSECRET",
    "2468",
  );

  assert.deepEqual(core.protectSecretCalls, [["SSECRET", "2468", address]]);
  assert.equal(upgraded.account.id, account.id);
  assert.equal(upgraded.account.name, "Keep me");
  assert.deepEqual(upgraded.signer, {
    id: "signer-1",
    kind: "protected-software",
    signerPublicKey: address,
    envelopeJson: '{"new":"envelope"}',
  });
  assert.deepEqual(store.references, [{ accountId: account.id, signerId: "signer-1" }]);
  assert.equal(await coordinator.hasLocalSigner(account.id), true);
});

test("mnemonic upgrade passes derivation inputs and expected signer identity to Core", async () => {
  const { coordinator, core, address } = setupWatchOnly();
  const account = await coordinator.addWatchOnly({ address, network: "testnet" });

  await coordinator.upgradeClassicWatchOnlyWithMnemonic(account.id, {
    mnemonic: "one two three",
    mnemonicPassphrase: "extra",
    index: 7,
    language: "english",
    appPasscode: "1357",
  });

  assert.deepEqual(core.protectMnemonicCalls, [
    ["one two three", "extra", 7, "english", "1357", address],
  ]);
});

test("Core identity mismatch leaves watch-only persistence unchanged", async () => {
  const { coordinator, core, store, address } = setupWatchOnly();
  const account = await coordinator.addWatchOnly({ address, network: "public" });
  core.protectSecretError = new Error("identity-mismatch");

  await assert.rejects(
    coordinator.upgradeClassicWatchOnlyWithSecret(account.id, "SWRONG", "2468"),
    /identity-mismatch/,
  );

  assert.equal(store.signers.size, 0);
  assert.equal(store.references.length, 0);
  assert.equal(await coordinator.hasLocalSigner(account.id), false);
});

test("contract watch-only account rejects Ed25519 software-signer upgrade before Core protection", async () => {
  const { coordinator, core, address } = setupWatchOnly("contract");
  const account = await coordinator.addWatchOnly({ address, network: "public" });

  await assert.rejects(
    coordinator.upgradeClassicWatchOnlyWithSecret(account.id, "SSECRET", "2468"),
    (error: unknown) =>
      error instanceof WalletLifecycleError && error.code === "unsupported-account-kind",
  );

  assert.equal(core.protectSecretCalls.length, 0);
});

test("app-passcode rotation stages every signer before atomically replacing envelopes", async () => {
  const core = new FakeCore();
  const store = new MemoryStore();
  store.signers.set("signer-a", signer("signer-a", "GA", "old-a"));
  store.signers.set("signer-b", signer("signer-b", "GB", "old-b"));
  core.systemAuth.add("GA");
  const coordinator = new WalletLifecycleCoordinator(core, store, ids("unused"));

  const result = await coordinator.reprotectAllProtectedSigners("old-pass", "new-pass");

  assert.deepEqual(core.reprotectCalls, [
    ["old-a", "old-pass", "new-pass", "GA"],
    ["old-b", "old-pass", "new-pass", "GB"],
  ]);
  assert.equal(store.signers.get("signer-a")?.envelopeJson, "old-a:new");
  assert.equal(store.signers.get("signer-b")?.envelopeJson, "old-b:new");
  assert.deepEqual(result.updatedSignerIds, ["signer-a", "signer-b"]);
  assert.deepEqual(result.systemAuthReenrollment, ["GA"]);
  assert.deepEqual(result.pendingSystemAuthCleanup, []);
  assert.equal(core.systemAuth.has("GA"), false);
});

test("app-passcode rotation failure during staging leaves all persisted envelopes unchanged", async () => {
  const core = new FakeCore();
  const store = new MemoryStore();
  store.signers.set("signer-a", signer("signer-a", "GA", "old-a"));
  store.signers.set("signer-b", signer("signer-b", "GB", "old-b"));
  core.reprotectErrorFor.add("GB");
  const coordinator = new WalletLifecycleCoordinator(core, store, ids("unused"));

  await assert.rejects(
    coordinator.reprotectAllProtectedSigners("old-pass", "new-pass"),
    /reprotect failed for GB/,
  );

  assert.equal(store.signers.get("signer-a")?.envelopeJson, "old-a");
  assert.equal(store.signers.get("signer-b")?.envelopeJson, "old-b");
  assert.equal(core.systemAuth.size, 0);
});

test("app-passcode rotation aborts when signer persistence changes after staging", async () => {
  const core = new FakeCore();
  const store = new MemoryStore();
  store.signers.set("signer-a", signer("signer-a", "GA", "old-a"));
  store.beforeTransaction = () => {
    store.signers.set("signer-a", signer("signer-a", "GA", "concurrent"));
  };
  const coordinator = new WalletLifecycleCoordinator(core, store, ids("unused"));

  await assert.rejects(
    coordinator.reprotectAllProtectedSigners("old-pass", "new-pass"),
    (error: unknown) =>
      error instanceof WalletLifecycleError && error.code === "signer-changed",
  );

  assert.equal(store.signers.get("signer-a")?.envelopeJson, "concurrent");
});

test("app-passcode rotation reports stale system-auth cleanup for retry after commit", async () => {
  const core = new FakeCore();
  const store = new MemoryStore();
  store.signers.set("signer-a", signer("signer-a", "GA", "old-a"));
  core.systemAuth.add("GA");
  core.failCleanupFor.add("GA");
  const coordinator = new WalletLifecycleCoordinator(core, store, ids("unused"));

  const result = await coordinator.reprotectAllProtectedSigners("old-pass", "new-pass");

  assert.equal(store.signers.get("signer-a")?.envelopeJson, "old-a:new");
  assert.deepEqual(result.systemAuthReenrollment, []);
  assert.deepEqual(result.pendingSystemAuthCleanup, ["GA"]);
});

test("downgrade preserves account, deletes orphan signer, and removes system auth", async () => {
  const { coordinator, core, store, address } = setupWatchOnly();
  const account = await coordinator.addWatchOnly({ address, network: "public", name: "Persistent" });
  await coordinator.upgradeClassicWatchOnlyWithSecret(account.id, "SSECRET", "2468");
  core.systemAuth.add(address);

  const result = await coordinator.downgradeToWatchOnly(account.id);

  assert.equal(store.accounts.get(account.id)?.name, "Persistent");
  assert.equal(store.references.length, 0);
  assert.equal(store.signers.size, 0);
  assert.deepEqual(result.removedSignerIds, ["signer-1"]);
  assert.deepEqual(result.pendingSystemAuthCleanup, []);
  assert.equal(core.systemAuth.has(address), false);
  assert.equal(await coordinator.hasLocalSigner(account.id), false);
});

test("downgrade keeps a signer and system auth when another account still references it", async () => {
  const core = new FakeCore();
  const store = new MemoryStore();
  const sharedSigner = signer("shared-signer", "GDELEGATE", '{"shared":true}');
  const accountA: AccountRecord = {
    id: "account-a",
    address: "GACCOUNT_A",
    kind: "classic",
    network: "public",
    name: null,
  };
  const accountB: AccountRecord = {
    id: "account-b",
    address: "GACCOUNT_B",
    kind: "classic",
    network: "public",
    name: null,
  };
  store.accounts.set(accountA.id, accountA);
  store.accounts.set(accountB.id, accountB);
  store.signers.set(sharedSigner.id, sharedSigner);
  store.references.push(
    { accountId: accountA.id, signerId: sharedSigner.id },
    { accountId: accountB.id, signerId: sharedSigner.id },
  );
  core.systemAuth.add(sharedSigner.signerPublicKey);
  const coordinator = new WalletLifecycleCoordinator(core, store, ids("unused"));

  const result = await coordinator.downgradeToWatchOnly(accountA.id);

  assert.deepEqual(result.removedSignerIds, []);
  assert.deepEqual(result.pendingSystemAuthCleanup, []);
  assert.equal(store.signers.has(sharedSigner.id), true);
  assert.deepEqual(store.references, [{ accountId: accountB.id, signerId: sharedSigner.id }]);
  assert.equal(core.systemAuth.has(sharedSigner.signerPublicKey), true);
});

test("downgrade reports post-commit system-auth cleanup failures without restoring secret material", async () => {
  const { coordinator, core, store, address } = setupWatchOnly();
  const account = await coordinator.addWatchOnly({ address, network: "public" });
  await coordinator.upgradeClassicWatchOnlyWithSecret(account.id, "SSECRET", "2468");
  core.systemAuth.add(address);
  core.failCleanupFor.add(address);

  const result = await coordinator.downgradeToWatchOnly(account.id);

  assert.equal(store.references.length, 0);
  assert.equal(store.signers.size, 0);
  assert.deepEqual(result.pendingSystemAuthCleanup, [address]);
});
