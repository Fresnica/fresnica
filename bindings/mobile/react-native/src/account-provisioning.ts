import type {
  AccountRecord,
  ProtectedSoftwareSignerRecord,
  RecordIdFactory,
  WalletStore,
} from "./wallet-lifecycle.ts";

export type WalletAccountProvisioningErrorCode =
  | "invalid-input"
  | "unsupported-account-kind"
  | "record-id-collision";

export class WalletAccountProvisioningError extends Error {
  readonly code: WalletAccountProvisioningErrorCode;

  constructor(code: WalletAccountProvisioningErrorCode, message: string) {
    super(message);
    this.name = "WalletAccountProvisioningError";
    this.code = code;
  }
}

export interface ProvisioningProtectedSigner {
  signerPublicKey: string;
  envelopeJson: string;
}

export interface ProvisioningGeneratedMnemonic {
  signer: ProvisioningProtectedSigner;
  mnemonic: string;
  language: string;
  index: number;
}

/** High-level native methods needed to provision a new protected software-signer account. */
export interface FresnicaAccountProvisioningBridge {
  parseAccount(address: string): Promise<{
    kind: "classic" | "contract";
    address: string;
    publicKey: string | null;
  }>;
  protectSecret(
    secret: string,
    appPasscode: string,
    expectedSignerPublicKey: string | null,
  ): Promise<ProvisioningProtectedSigner>;
  protectMnemonic(
    mnemonic: string,
    mnemonicPassphrase: string,
    index: number,
    language: string | null,
    appPasscode: string,
    expectedSignerPublicKey: string | null,
  ): Promise<ProvisioningProtectedSigner>;
  generateMnemonic(
    language: string,
    strength: number,
    mnemonicPassphrase: string,
    index: number,
    appPasscode: string,
  ): Promise<ProvisioningGeneratedMnemonic>;
}

export interface ImportSecretAccountInput {
  secret: string;
  appPasscode: string;
  network: string;
  name?: string | null;
}

export interface ImportMnemonicAccountInput {
  mnemonic: string;
  mnemonicPassphrase: string;
  index: number;
  language?: string | null;
  appPasscode: string;
  network: string;
  name?: string | null;
}

export interface GenerateMnemonicAccountInput {
  language: string;
  strength: number;
  mnemonicPassphrase: string;
  index: number;
  appPasscode: string;
  network: string;
  name?: string | null;
}

export interface NewProtectedAccountResult {
  account: AccountRecord;
  signer: ProtectedSoftwareSignerRecord;
}

export interface GeneratedProtectedAccountResult extends NewProtectedAccountResult {
  /** One-time recovery material. The coordinator never persists this value. */
  mnemonic: string;
  language: string;
  index: number;
}

/**
 * Creates a new classic AccountRecord + protected SignerRecord + reference atomically.
 *
 * Core owns recovery-material validation and derives the signer identity. The persistence layer
 * sees only the resulting G address and opaque protected envelope. System-auth enrollment remains
 * an explicit follow-up action after this database transaction commits.
 */
export class WalletAccountProvisioningCoordinator {
  private readonly core: FresnicaAccountProvisioningBridge;
  private readonly store: WalletStore;
  private readonly createId: RecordIdFactory;

  constructor(
    core: FresnicaAccountProvisioningBridge,
    store: WalletStore,
    createId: RecordIdFactory,
  ) {
    this.core = core;
    this.store = store;
    this.createId = createId;
  }

  async importSecretAccount(
    input: ImportSecretAccountInput,
  ): Promise<NewProtectedAccountResult> {
    const signer = await this.core.protectSecret(input.secret, input.appPasscode, null);
    return this.persistNewMasterSignerAccount(
      signer,
      requireNonEmpty(input.network, "network"),
      normalizeName(input.name),
    );
  }

  async importMnemonicAccount(
    input: ImportMnemonicAccountInput,
  ): Promise<NewProtectedAccountResult> {
    const signer = await this.core.protectMnemonic(
      input.mnemonic,
      input.mnemonicPassphrase,
      input.index,
      input.language ?? null,
      input.appPasscode,
      null,
    );
    return this.persistNewMasterSignerAccount(
      signer,
      requireNonEmpty(input.network, "network"),
      normalizeName(input.name),
    );
  }

  async generateMnemonicAccount(
    input: GenerateMnemonicAccountInput,
  ): Promise<GeneratedProtectedAccountResult> {
    const generated = await this.core.generateMnemonic(
      input.language,
      input.strength,
      input.mnemonicPassphrase,
      input.index,
      input.appPasscode,
    );
    const persisted = await this.persistNewMasterSignerAccount(
      generated.signer,
      requireNonEmpty(input.network, "network"),
      normalizeName(input.name),
    );
    return {
      ...persisted,
      mnemonic: generated.mnemonic,
      language: generated.language,
      index: generated.index,
    };
  }

  private async persistNewMasterSignerAccount(
    protectedSigner: ProvisioningProtectedSigner,
    network: string,
    name: string | null,
  ): Promise<NewProtectedAccountResult> {
    const identity = await this.core.parseAccount(protectedSigner.signerPublicKey);
    if (identity.kind !== "classic") {
      throw new WalletAccountProvisioningError(
        "unsupported-account-kind",
        "An Ed25519 software signer can create only a classic Stellar account identity",
      );
    }

    const account: AccountRecord = {
      id: requireRecordId(this.createId("account"), "account"),
      address: identity.address,
      kind: "classic",
      network,
      name,
    };
    const signer: ProtectedSoftwareSignerRecord = {
      id: requireRecordId(this.createId("signer"), "signer"),
      kind: "protected-software",
      signerPublicKey: protectedSigner.signerPublicKey,
      envelopeJson: protectedSigner.envelopeJson,
    };

    return this.store.transaction((transaction) => {
      if (transaction.getAccount(account.id) !== null) {
        throw new WalletAccountProvisioningError(
          "record-id-collision",
          `Account record id already exists: ${account.id}`,
        );
      }
      if (transaction.getSigner(signer.id) !== null) {
        throw new WalletAccountProvisioningError(
          "record-id-collision",
          `Signer record id already exists: ${signer.id}`,
        );
      }

      transaction.putAccount(account);
      transaction.putSigner(signer);
      transaction.putSignerReference({ accountId: account.id, signerId: signer.id });
      return { account, signer };
    });
  }
}

function normalizeName(value: string | null | undefined): string | null {
  if (value === null || value === undefined) return null;
  const normalized = value.trim();
  return normalized.length === 0 ? null : normalized;
}

function requireNonEmpty(value: string, field: string): string {
  const normalized = value.trim();
  if (normalized.length === 0) {
    throw new WalletAccountProvisioningError("invalid-input", `${field} must not be empty`);
  }
  return normalized;
}

function requireRecordId(value: string, kind: "account" | "signer"): string {
  const normalized = value.trim();
  if (normalized.length === 0) {
    throw new WalletAccountProvisioningError("invalid-input", `${kind} record id must not be empty`);
  }
  return normalized;
}
