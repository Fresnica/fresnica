import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

import {
  commitPortableRestore,
  createPortableBackup,
  decodePortableBackup,
  encodePortableBackup,
  parsePortableBackup,
  PortableBackupError,
  stagePortableRestore,
  type PortableWalletBackup,
} from "../src/portable-backup.ts";
import type {
  AccountRecord,
  AccountSignerReference,
  FresnicaCoreLifecycleBridge,
  NativeAccountIdentity,
  NativeProtectedSoftwareSigner,
  ProtectedSoftwareSignerRecord,
  WalletStore,
  WalletStoreTransaction,
} from "../src/wallet-lifecycle.ts";

class FakeCore implements FresnicaCoreLifecycleBridge {
  async parseAccount(address: string): Promise<NativeAccountIdentity> {
    if (!address.startsWith("G") && !address.startsWith("C")) {
      throw new Error("invalid account");
    }
    return {
      kind: address.startsWith("G") ? "classic" : "contract",
      address,
      publicKey: address.startsWith("G") ? address : null,
    };
  }

  async reprotect(
    envelopeJson: string,
    currentPasscode: string,
    newPasscode: string,
    expectedSignerPublicKey: string,
  ): Promise<NativeProtectedSoftwareSigner> {
    const envelope = JSON.parse(envelopeJson) as { signerPublicKey?: string };
    if (envelope.signerPublicKey !== expectedSignerPublicKey) {
      throw new Error("identity mismatch");
    }
    return {
      signerPublicKey: expectedSignerPublicKey,
      envelopeJson: JSON.stringify({
        signerPublicKey: expectedSignerPublicKey,
        refreshed: true,
        changedPasscode: currentPasscode !== newPasscode,
      }),
    };
  }

  async protectSecret(): Promise<NativeProtectedSoftwareSigner> {
    throw new Error("not used");
  }

  async protectMnemonic(): Promise<NativeProtectedSoftwareSigner> {
    throw new Error("not used");
  }

  async hasSystemAuth(): Promise<boolean> {
    return false;
  }

  async removeSystemAuth(): Promise<true> {
    return true;
  }
}

class MemoryStore implements WalletStore, WalletStoreTransaction {
  readonly accounts = new Map<string, AccountRecord>();
  readonly signers = new Map<string, ProtectedSoftwareSignerRecord>();
  readonly references: AccountSignerReference[] = [];

  async getAccount(accountId: string): Promise<AccountRecord | null>;
  getAccount(accountId: string): AccountRecord | null;
  getAccount(accountId: string): AccountRecord | null {
    return this.accounts.get(accountId) ?? null;
  }

  async listProtectedSoftwareSigners(): Promise<readonly ProtectedSoftwareSignerRecord[]> {
    return [...this.signers.values()];
  }

  async listSignerReferencesForAccount(
    accountId: string,
  ): Promise<readonly AccountSignerReference[]>;
  listSignerReferencesForAccount(accountId: string): readonly AccountSignerReference[];
  listSignerReferencesForAccount(accountId: string): readonly AccountSignerReference[] {
    return this.references.filter((reference) => reference.accountId === accountId);
  }

  async transaction<T>(work: (transaction: WalletStoreTransaction) => T): Promise<T> {
    const before = {
      accounts: structuredClone(this.accounts),
      signers: structuredClone(this.signers),
      references: structuredClone(this.references),
    };
    try {
      return work(this);
    } catch (error) {
      this.accounts.clear();
      for (const [id, account] of before.accounts) this.accounts.set(id, account);
      this.signers.clear();
      for (const [id, signer] of before.signers) this.signers.set(id, signer);
      this.references.splice(0, this.references.length, ...before.references);
      throw error;
    }
  }

  getSigner(signerId: string): ProtectedSoftwareSignerRecord | null {
    return this.signers.get(signerId) ?? null;
  }

  countSignerReferences(signerId: string): number {
    return this.references.filter((reference) => reference.signerId === signerId).length;
  }

  putAccount(account: AccountRecord): void {
    this.accounts.set(account.id, { ...account });
  }

  putSigner(signer: ProtectedSoftwareSignerRecord): void {
    this.signers.set(signer.id, { ...signer });
  }

  putSignerReference(reference: AccountSignerReference): void {
    if (
      !this.references.some(
        (candidate) =>
          candidate.accountId === reference.accountId && candidate.signerId === reference.signerId,
      )
    ) {
      this.references.push({ ...reference });
    }
  }

  deleteSignerReference(accountId: string, signerId: string): void {
    const index = this.references.findIndex(
      (reference) => reference.accountId === accountId && reference.signerId === signerId,
    );
    if (index >= 0) this.references.splice(index, 1);
  }

  deleteSigner(signerId: string): void {
    this.signers.delete(signerId);
  }
}

function portableBackup(): PortableWalletBackup {
  return parsePortableBackup({
    format: "fresnica-wallet-backup",
    version: 2,
    accounts: [
      { ref: "a1", address: "GMASTER", suggestedNetwork: "mainnet", name: "Main" },
      { ref: "a2", address: "GDELEGATEDACCOUNT", suggestedNetwork: "mainnet", name: null },
      { ref: "a3", address: "CCONTRACT", suggestedNetwork: "testnet", name: "Contract" },
      { ref: "a4", address: "GWATCH", suggestedNetwork: "mainnet", name: "Watch" },
    ],
    signers: [
      {
        ref: "s1",
        kind: "protected-software",
        signerPublicKey: "GMASTER",
        envelope: { signerPublicKey: "GMASTER" },
      },
      {
        ref: "s2",
        kind: "protected-software",
        signerPublicKey: "GDELEGATED",
        envelope: { signerPublicKey: "GDELEGATED" },
      },
    ],
    accountSignerReferences: [
      { accountRef: "a1", signerRef: "s1" },
      { accountRef: "a2", signerRef: "s2" },
      { accountRef: "a3", signerRef: "s2" },
    ],
    recoverySources: [{ ref: "r1", kind: "mnemonic", signerRefs: ["s1", "s2"] }],
  });
}

function ids(): (kind: "account" | "signer") => string {
  let account = 0;
  let signer = 0;
  return (kind) => (kind === "account" ? `account-${++account}` : `signer-${++signer}`);
}

function targetNetworks(): Record<string, string> {
  return { a1: "testnet", a2: "mainnet", a3: "testnet", a4: "mainnet" };
}

test("portable backup strips installation ids and roundtrips the strict v2 shape", () => {
  const backup = createPortableBackup({
    accounts: [
      { id: "local-account", address: "GMASTER", kind: "classic", network: "mainnet", name: "Main" },
    ],
    signers: [
      {
        id: "local-signer",
        kind: "protected-software",
        signerPublicKey: "GMASTER",
        envelopeJson: JSON.stringify({ signerPublicKey: "GMASTER" }),
      },
    ],
    references: [{ accountId: "local-account", signerId: "local-signer" }],
    recoverySources: [{ kind: "mnemonic", signerIds: ["local-signer"] }],
  });

  const text = encodePortableBackup(backup);
  assert.equal(text.includes("local-account"), false);
  assert.equal(text.includes("local-signer"), false);
  assert.deepEqual(decodePortableBackup(text), backup);
  assert.deepEqual(backup.accountSignerReferences, [{ accountRef: "a1", signerRef: "s1" }]);
});

test("portable backup v2 cross-language fixture matches the parser contract", () => {
  const fixture = JSON.parse(
    readFileSync(
      new URL("../../../../spec/test-vectors/portable-backup-v2.json", import.meta.url),
      "utf8",
    ),
  ) as { backup: unknown };

  const backup = parsePortableBackup(fixture.backup);
  assert.equal(backup.accounts.length, 4);
  assert.equal(backup.signers.length, 2);
  assert.equal(backup.accountSignerReferences.length, 3);
  assert.equal(backup.recoverySources.length, 1);
});

test("portable backup rejects unknown fields so device-bound auth cannot hitchhike", () => {
  assert.throws(
    () =>
      parsePortableBackup({
        ...portableBackup(),
        unlockKey: "device-bound-secret",
      }),
    (error: unknown) =>
      error instanceof PortableBackupError &&
      error.code === "invalid-backup" &&
      /Unexpected portable backup field/.test(error.message),
  );
});

test("restore requires an explicit target network instead of trusting the backup hint", async () => {
  await assert.rejects(
    stagePortableRestore(new FakeCore(), ids(), {
      backup: portableBackup(),
      backupPasscode: "old",
      appPasscode: "new",
      targetNetworks: {},
    }),
    (error: unknown) =>
      error instanceof PortableBackupError && error.code === "network-unconfirmed",
  );
});

test("restore re-protects signers and activates only directly proven master-key references", async () => {
  const staged = await stagePortableRestore(new FakeCore(), ids(), {
    backup: portableBackup(),
    backupPasscode: "old",
    appPasscode: "new",
    targetNetworks: targetNetworks(),
  });

  assert.equal(staged.accounts[0].network, "testnet");
  assert.equal(staged.accounts[3].name, "Watch");
  assert.equal(staged.signers.every((signer) => signer.envelopeJson.includes('"refreshed":true')), true);
  assert.deepEqual(staged.activeReferences, [{ accountId: "account-1", signerId: "signer-1" }]);
  assert.deepEqual(
    staged.pendingReferences.map((reference) => [reference.accountRef, reference.signerRef, reference.reason]),
    [
      ["a2", "s2", "ledger-authorization-required"],
      ["a3", "s2", "provider-authorization-required"],
    ],
  );
  assert.deepEqual(staged.recoverySourceHints, [
    { ref: "r1", kind: "mnemonic", signerIds: ["signer-1", "signer-2"] },
  ]);
});

test("restore rejects a protected signer whose envelope does not match its declared identity", async () => {
  const backup = portableBackup();
  const tampered = {
    ...backup,
    signers: backup.signers.map((signer) =>
      signer.ref === "s1" ? { ...signer, signerPublicKey: "GOTHER" } : signer,
    ),
  };

  await assert.rejects(
    stagePortableRestore(new FakeCore(), ids(), {
      backup: tampered,
      backupPasscode: "old",
      appPasscode: "new",
      targetNetworks: targetNetworks(),
    }),
    /identity mismatch/,
  );
});

test("commit activates only pending relationships that the host validates", async () => {
  const staged = await stagePortableRestore(new FakeCore(), ids(), {
    backup: portableBackup(),
    backupPasscode: "old",
    appPasscode: "new",
    targetNetworks: targetNetworks(),
  });
  const store = new MemoryStore();
  const checked: unknown[] = [];

  await commitPortableRestore(store, staged, async (account, signerPublicKey, reason) => {
    checked.push([account.address, account.network, signerPublicKey, reason]);
    return reason === "ledger-authorization-required";
  });

  assert.deepEqual(checked, [
    ["GDELEGATEDACCOUNT", "mainnet", "GDELEGATED", "ledger-authorization-required"],
    ["CCONTRACT", "testnet", "GDELEGATED", "provider-authorization-required"],
  ]);
  assert.deepEqual(store.references, [
    { accountId: "account-1", signerId: "signer-1" },
    { accountId: "account-2", signerId: "signer-2" },
  ]);
});

test("commit keeps pending relationships inactive without a host validator", async () => {
  const staged = await stagePortableRestore(new FakeCore(), ids(), {
    backup: portableBackup(),
    backupPasscode: "old",
    appPasscode: "new",
    targetNetworks: targetNetworks(),
  });
  const store = new MemoryStore();

  await commitPortableRestore(store, staged);

  assert.deepEqual(store.references, [{ accountId: "account-1", signerId: "signer-1" }]);
});

test("commit rejects an inconsistent pending staging graph before persistence", async () => {
  const staged = await stagePortableRestore(new FakeCore(), ids(), {
    backup: portableBackup(),
    backupPasscode: "old",
    appPasscode: "new",
    targetNetworks: targetNetworks(),
  });
  const tampered = {
    ...staged,
    pendingReferences: [
      { ...staged.pendingReferences[0]!, signerId: "missing-signer" },
    ],
  };
  const store = new MemoryStore();

  await assert.rejects(
    commitPortableRestore(store, tampered, async () => true),
    (error: unknown) => error instanceof PortableBackupError && error.code === "invalid-reference",
  );
  assert.equal(store.accounts.size, 0);
  assert.equal(store.signers.size, 0);
});
