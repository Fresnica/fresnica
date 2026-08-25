import type { WalletStore } from "./wallet-lifecycle.ts";

export type ExportedSigningMaterialKind = "secret" | "mnemonic";

export interface ExportedSigningMaterial {
  kind: ExportedSigningMaterialKind;
  secret: string | null;
  mnemonic: string | null;
  mnemonicPassphrase: string | null;
  index: number | null;
  language: string | null;
}

export interface FresnicaSignerExportBridge {
  reveal(
    envelopeJson: string,
    freshAppPasscode: string,
    expectedSignerPublicKey: string,
  ): Promise<ExportedSigningMaterial>;
}

export type WalletSignerExportErrorCode = "signer-not-found";

export class WalletSignerExportError extends Error {
  readonly code: WalletSignerExportErrorCode;

  constructor(code: WalletSignerExportErrorCode, message: string) {
    super(message);
    this.name = "WalletSignerExportError";
    this.code = code;
  }
}

/**
 * Explicit software-signer Reveal / Export coordinator.
 *
 * This is intentionally signer-centric: an AccountRecord may have zero, one or many signer
 * capabilities. The caller chooses the protected software signer being exported. A fresh app
 * passcode goes directly to Core; system-auth WalletUnlockKey material is never accepted here.
 * Returned plaintext is never written back through WalletStore.
 */
export class WalletSignerExportCoordinator {
  private readonly core: FresnicaSignerExportBridge;
  private readonly store: WalletStore;

  constructor(core: FresnicaSignerExportBridge, store: WalletStore) {
    this.core = core;
    this.store = store;
  }

  async revealProtectedSoftwareSigner(
    signerId: string,
    freshAppPasscode: string,
  ): Promise<ExportedSigningMaterial> {
    const signer = await this.store.transaction((transaction) => transaction.getSigner(signerId));
    if (signer === null) {
      throw new WalletSignerExportError("signer-not-found", `Signer not found: ${signerId}`);
    }

    return this.core.reveal(
      signer.envelopeJson,
      freshAppPasscode,
      signer.signerPublicKey,
    );
  }
}
