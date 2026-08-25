import assert from "node:assert/strict";
import test from "node:test";

import {
  WalletSignerExportCoordinator,
  WalletSignerExportError,
  type ExportedSigningMaterial,
  type FresnicaSignerExportBridge,
} from "../src/signer-export.ts";
import type {
  AccountRecord,
  AccountSignerReference,
  ProtectedSoftwareSignerRecord,
  WalletStore,
  WalletStoreTransaction,
} from "../src/wallet-lifecycle.ts";

class ExportStore implements WalletStore {
  signers = new Map<string, ProtectedSoftwareSignerRecord>();
  transactionCount = 0;

  async getAccount(): Promise<AccountRecord | null> {
    return null;
  }

  async listProtectedSoftwareSigners(): Promise<readonly ProtectedSoftwareSignerRecord[]> {
    return [...this.signers.values()];
  }

  async listSignerReferencesForAccount(): Promise<readonly AccountSignerReference[]> {
    return [];
  }

  async transaction<T>(work: (transaction: WalletStoreTransaction) => T): Promise<T> {
    this.transactionCount += 1;
    const transaction: WalletStoreTransaction = {
      getAccount: () => null,
      getSigner: (id) => this.signers.get(id) ?? null,
      listSignerReferencesForAccount: () => [],
      countSignerReferences: () => 0,
      putAccount: () => undefined,
      putSigner: () => undefined,
      putSignerReference: () => undefined,
      deleteSignerReference: () => undefined,
      deleteSigner: () => undefined,
    };
    return work(transaction);
  }
}

class FakeExportCore implements FresnicaSignerExportBridge {
  calls: unknown[][] = [];
  result: ExportedSigningMaterial = {
    kind: "secret",
    secret: "SSECRET",
    mnemonic: null,
    mnemonicPassphrase: null,
    index: null,
    language: null,
  };
  error: Error | null = null;

  async reveal(
    envelopeJson: string,
    freshAppPasscode: string,
    expectedSignerPublicKey: string,
  ): Promise<ExportedSigningMaterial> {
    this.calls.push([envelopeJson, freshAppPasscode, expectedSignerPublicKey]);
    if (this.error !== null) throw this.error;
    return this.result;
  }
}

function protectedSigner(): ProtectedSoftwareSignerRecord {
  return {
    id: "signer-1",
    kind: "protected-software",
    signerPublicKey: "GSIGNER",
    envelopeJson: '{"opaque":"envelope"}',
  };
}

test("explicit reveal passes the stored envelope and fresh passcode to Core", async () => {
  const core = new FakeExportCore();
  const store = new ExportStore();
  store.signers.set("signer-1", protectedSigner());
  const coordinator = new WalletSignerExportCoordinator(core, store);

  const result = await coordinator.revealProtectedSoftwareSigner("signer-1", "fresh-pass");

  assert.equal(result.secret, "SSECRET");
  assert.deepEqual(core.calls, [
    ['{"opaque":"envelope"}', "fresh-pass", "GSIGNER"],
  ]);
});

test("mnemonic reveal returns plaintext without mutating persisted signer data", async () => {
  const core = new FakeExportCore();
  const store = new ExportStore();
  store.signers.set("signer-1", protectedSigner());
  core.result = {
    kind: "mnemonic",
    secret: null,
    mnemonic: "alpha beta gamma delta",
    mnemonicPassphrase: "extra",
    index: 4,
    language: "english",
  };
  const before = JSON.stringify([...store.signers.values()]);
  const coordinator = new WalletSignerExportCoordinator(core, store);

  const result = await coordinator.revealProtectedSoftwareSigner("signer-1", "fresh-pass");

  assert.equal(result.mnemonic, "alpha beta gamma delta");
  assert.equal(JSON.stringify([...store.signers.values()]), before);
});

test("missing signer fails before any Core reveal call", async () => {
  const core = new FakeExportCore();
  const store = new ExportStore();
  const coordinator = new WalletSignerExportCoordinator(core, store);

  await assert.rejects(
    coordinator.revealProtectedSoftwareSigner("missing", "fresh-pass"),
    (error: unknown) =>
      error instanceof WalletSignerExportError && error.code === "signer-not-found",
  );
  assert.equal(core.calls.length, 0);
});

test("Core invalid-passcode failure propagates and leaves the envelope unchanged", async () => {
  const core = new FakeExportCore();
  const store = new ExportStore();
  store.signers.set("signer-1", protectedSigner());
  core.error = new Error("invalid-passcode");
  const before = store.signers.get("signer-1")?.envelopeJson;
  const coordinator = new WalletSignerExportCoordinator(core, store);

  await assert.rejects(
    coordinator.revealProtectedSoftwareSigner("signer-1", "wrong"),
    /invalid-passcode/,
  );
  assert.equal(store.signers.get("signer-1")?.envelopeJson, before);
});
