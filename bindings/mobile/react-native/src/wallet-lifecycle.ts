export type AccountKind = "classic" | "contract";

export interface NativeAccountIdentity {
  kind: AccountKind;
  address: string;
  publicKey: string | null;
}

export interface NativeProtectedSoftwareSigner {
  signerPublicKey: string;
  envelopeJson: string;
}

export interface FresnicaCoreLifecycleBridge {
  parseAccount(address: string): Promise<NativeAccountIdentity>;
  protectSecret(
    secret: string,
    appPasscode: string,
    expectedSignerPublicKey: string,
  ): Promise<NativeProtectedSoftwareSigner>;
  protectMnemonic(
    mnemonic: string,
    mnemonicPassphrase: string,
    index: number,
    language: string | null,
    appPasscode: string,
    expectedSignerPublicKey: string,
  ): Promise<NativeProtectedSoftwareSigner>;
  reprotect(
    envelopeJson: string,
    currentPasscode: string,
    newPasscode: string,
    expectedSignerPublicKey: string,
  ): Promise<NativeProtectedSoftwareSigner>;
  hasSystemAuth(expectedSignerPublicKey: string): Promise<boolean>;
  removeSystemAuth(expectedSignerPublicKey: string): Promise<true>;
}

export interface AccountRecord {
  id: string;
  address: string;
  kind: AccountKind;
  network: string;
  name: string | null;
}

export interface ProtectedSoftwareSignerRecord {
  id: string;
  kind: "protected-software";
  signerPublicKey: string;
  envelopeJson: string;
}

export interface AccountSignerReference {
  accountId: string;
  signerId: string;
}

export interface WalletStoreReader {
  getAccount(accountId: string): AccountRecord | null;
  getSigner(signerId: string): ProtectedSoftwareSignerRecord | null;
  listSignerReferencesForAccount(accountId: string): readonly AccountSignerReference[];
  countSignerReferences(signerId: string): number;
}

export interface WalletStoreTransaction extends WalletStoreReader {
  putAccount(account: AccountRecord): void;
  putSigner(signer: ProtectedSoftwareSignerRecord): void;
  putSignerReference(reference: AccountSignerReference): void;
  deleteSignerReference(accountId: string, signerId: string): void;
  deleteSigner(signerId: string): void;
}

export interface WalletStore {
  getAccount(accountId: string): Promise<AccountRecord | null>;
  listProtectedSoftwareSigners(): Promise<readonly ProtectedSoftwareSignerRecord[]>;
  listSignerReferencesForAccount(accountId: string): Promise<readonly AccountSignerReference[]>;
  transaction<T>(work: (transaction: WalletStoreTransaction) => T): Promise<T>;
}

export type RecordIdFactory = (kind: "account" | "signer") => string;

export type WalletLifecycleErrorCode =
  | "invalid-input"
  | "account-not-found"
  | "account-not-watch-only"
  | "unsupported-account-kind"
  | "record-id-collision"
  | "account-changed"
  | "signer-changed";

export class WalletLifecycleError extends Error {
  readonly code: WalletLifecycleErrorCode;

  constructor(code: WalletLifecycleErrorCode, message: string) {
    super(message);
    this.name = "WalletLifecycleError";
    this.code = code;
  }
}

export interface AddWatchOnlyInput {
  address: string;
  network: string;
  name?: string | null;
}

export interface UpgradeMnemonicInput {
  mnemonic: string;
  mnemonicPassphrase: string;
  index: number;
  language?: string | null;
  appPasscode: string;
}

export interface UpgradeResult {
  account: AccountRecord;
  signer: ProtectedSoftwareSignerRecord;
}

export interface DowngradeResult {
  account: AccountRecord;
  removedSignerIds: readonly string[];
  pendingSystemAuthCleanup: readonly string[];
}

export interface ReprotectAllResult {
  updatedSignerIds: readonly string[];
  systemAuthReenrollment: readonly string[];
  pendingSystemAuthCleanup: readonly string[];
}

interface WatchOnlySnapshot {
  account: AccountRecord;
}

interface DowngradeCommit {
  account: AccountRecord;
  removedSignerIds: string[];
  orphanedSignerPublicKeys: string[];
}

interface StagedReprotect {
  before: ProtectedSoftwareSignerRecord;
  after: ProtectedSoftwareSignerRecord;
}

/**
 * React Native-side account/signer lifecycle coordinator.
 *
 * It owns no crypto and no database implementation. Core validates Stellar identity and signing
 * material; WalletStore owns persistence. Watch-only is derived from the absence of local signer
 * references and is never persisted as a wallet type flag.
 */
export class WalletLifecycleCoordinator {
  private readonly core: FresnicaCoreLifecycleBridge;
  private readonly store: WalletStore;
  private readonly createId: RecordIdFactory;

  constructor(
    core: FresnicaCoreLifecycleBridge,
    store: WalletStore,
    createId: RecordIdFactory,
  ) {
    this.core = core;
    this.store = store;
    this.createId = createId;
  }

  async addWatchOnly(input: AddWatchOnlyInput): Promise<AccountRecord> {
    const network = requireNonEmpty(input.network, "network");
    const identity = await this.core.parseAccount(input.address);
    const account: AccountRecord = {
      id: requireRecordId(this.createId("account"), "account"),
      address: identity.address,
      kind: identity.kind,
      network,
      name: normalizeName(input.name),
    };

    await this.store.transaction((transaction) => {
      if (transaction.getAccount(account.id) !== null) {
        throw new WalletLifecycleError(
          "record-id-collision",
          `Account record id already exists: ${account.id}`,
        );
      }
      transaction.putAccount(account);
    });

    return account;
  }

  async hasLocalSigner(accountId: string): Promise<boolean> {
    const account = await this.store.getAccount(accountId);
    if (account === null) {
      throw new WalletLifecycleError("account-not-found", `Account not found: ${accountId}`);
    }
    const references = await this.store.listSignerReferencesForAccount(accountId);
    return references.length > 0;
  }

  async upgradeClassicWatchOnlyWithSecret(
    accountId: string,
    secret: string,
    appPasscode: string,
  ): Promise<UpgradeResult> {
    const snapshot = await this.requireClassicWatchOnly(accountId);
    const protectedSigner = await this.core.protectSecret(
      secret,
      appPasscode,
      snapshot.account.address,
    );
    return this.attachProtectedSigner(snapshot, protectedSigner);
  }

  async upgradeClassicWatchOnlyWithMnemonic(
    accountId: string,
    input: UpgradeMnemonicInput,
  ): Promise<UpgradeResult> {
    const snapshot = await this.requireClassicWatchOnly(accountId);
    const protectedSigner = await this.core.protectMnemonic(
      input.mnemonic,
      input.mnemonicPassphrase,
      input.index,
      input.language ?? null,
      input.appPasscode,
      snapshot.account.address,
    );
    return this.attachProtectedSigner(snapshot, protectedSigner);
  }

  /**
   * Re-protects every local protected-software signer before committing any new envelope.
   *
   * All Core work is staged first. The database swap is then one WalletStore transaction. Only
   * after that commit succeeds are stale system-auth unlock keys removed. A failed cleanup is
   * reported for retry and never rolls signer envelopes back to the old passcode.
   */
  async reprotectAllProtectedSigners(
    currentPasscode: string,
    newPasscode: string,
  ): Promise<ReprotectAllResult> {
    const before = [...(await this.store.listProtectedSoftwareSigners())];
    const staged: StagedReprotect[] = [];

    for (const signer of before) {
      const protectedSigner = await this.core.reprotect(
        signer.envelopeJson,
        currentPasscode,
        newPasscode,
        signer.signerPublicKey,
      );
      staged.push({
        before: signer,
        after: {
          ...signer,
          signerPublicKey: protectedSigner.signerPublicKey,
          envelopeJson: protectedSigner.envelopeJson,
        },
      });
    }

    await this.store.transaction((transaction) => {
      for (const item of staged) {
        const current = transaction.getSigner(item.before.id);
        if (current === null || !sameSigner(current, item.before)) {
          throw new WalletLifecycleError(
            "signer-changed",
            `Signer changed while passcode rotation was being staged: ${item.before.id}`,
          );
        }
      }
      for (const item of staged) {
        transaction.putSigner(item.after);
      }
    });

    const systemAuthReenrollment: string[] = [];
    const pendingSystemAuthCleanup: string[] = [];
    for (const signerPublicKey of unique(staged.map((item) => item.after.signerPublicKey))) {
      try {
        if (await this.core.hasSystemAuth(signerPublicKey)) {
          await this.core.removeSystemAuth(signerPublicKey);
          systemAuthReenrollment.push(signerPublicKey);
        }
      } catch {
        pendingSystemAuthCleanup.push(signerPublicKey);
      }
    }

    return {
      updatedSignerIds: staged.map((item) => item.after.id),
      systemAuthReenrollment,
      pendingSystemAuthCleanup,
    };
  }

  async downgradeToWatchOnly(accountId: string): Promise<DowngradeResult> {
    const committed = await this.store.transaction((transaction) => {
      const account = transaction.getAccount(accountId);
      if (account === null) {
        throw new WalletLifecycleError("account-not-found", `Account not found: ${accountId}`);
      }

      const references = [...transaction.listSignerReferencesForAccount(accountId)];
      const removedSignerIds: string[] = [];
      const orphanedSignerPublicKeys: string[] = [];

      for (const reference of references) {
        const signer = transaction.getSigner(reference.signerId);
        if (signer === null) {
          throw new WalletLifecycleError(
            "account-changed",
            `Signer reference points to missing signer: ${reference.signerId}`,
          );
        }

        transaction.deleteSignerReference(reference.accountId, reference.signerId);
        if (transaction.countSignerReferences(reference.signerId) === 0) {
          transaction.deleteSigner(reference.signerId);
          removedSignerIds.push(reference.signerId);
          orphanedSignerPublicKeys.push(signer.signerPublicKey);
        }
      }

      const result: DowngradeCommit = {
        account,
        removedSignerIds,
        orphanedSignerPublicKeys,
      };
      return result;
    });

    const pendingSystemAuthCleanup: string[] = [];
    for (const signerPublicKey of unique(committed.orphanedSignerPublicKeys)) {
      try {
        if (await this.core.hasSystemAuth(signerPublicKey)) {
          await this.core.removeSystemAuth(signerPublicKey);
        }
      } catch {
        pendingSystemAuthCleanup.push(signerPublicKey);
      }
    }

    return {
      account: committed.account,
      removedSignerIds: committed.removedSignerIds,
      pendingSystemAuthCleanup,
    };
  }

  private async requireClassicWatchOnly(accountId: string): Promise<WatchOnlySnapshot> {
    const account = await this.store.getAccount(accountId);
    if (account === null) {
      throw new WalletLifecycleError("account-not-found", `Account not found: ${accountId}`);
    }
    if (account.kind !== "classic") {
      throw new WalletLifecycleError(
        "unsupported-account-kind",
        "Contract accounts cannot be upgraded by attaching an Ed25519 software signer",
      );
    }

    const references = await this.store.listSignerReferencesForAccount(accountId);
    if (references.length !== 0) {
      throw new WalletLifecycleError(
        "account-not-watch-only",
        `Account already has ${references.length} local signer reference(s)`,
      );
    }

    return { account };
  }

  private async attachProtectedSigner(
    snapshot: WatchOnlySnapshot,
    protectedSigner: NativeProtectedSoftwareSigner,
  ): Promise<UpgradeResult> {
    const signer: ProtectedSoftwareSignerRecord = {
      id: requireRecordId(this.createId("signer"), "signer"),
      kind: "protected-software",
      signerPublicKey: protectedSigner.signerPublicKey,
      envelopeJson: protectedSigner.envelopeJson,
    };

    return this.store.transaction((transaction) => {
      const currentAccount = transaction.getAccount(snapshot.account.id);
      if (currentAccount === null) {
        throw new WalletLifecycleError(
          "account-changed",
          `Account disappeared during upgrade: ${snapshot.account.id}`,
        );
      }
      if (!sameAccountIdentity(currentAccount, snapshot.account)) {
        throw new WalletLifecycleError(
          "account-changed",
          `Account identity changed during upgrade: ${snapshot.account.id}`,
        );
      }
      if (transaction.listSignerReferencesForAccount(snapshot.account.id).length !== 0) {
        throw new WalletLifecycleError(
          "account-not-watch-only",
          "Account gained a local signer while upgrade material was being prepared",
        );
      }
      if (transaction.getSigner(signer.id) !== null) {
        throw new WalletLifecycleError(
          "record-id-collision",
          `Signer record id already exists: ${signer.id}`,
        );
      }

      transaction.putSigner(signer);
      transaction.putSignerReference({
        accountId: snapshot.account.id,
        signerId: signer.id,
      });

      return {
        account: currentAccount,
        signer,
      };
    });
  }
}

function sameAccountIdentity(left: AccountRecord, right: AccountRecord): boolean {
  return (
    left.id === right.id &&
    left.address === right.address &&
    left.kind === right.kind &&
    left.network === right.network
  );
}

function sameSigner(
  left: ProtectedSoftwareSignerRecord,
  right: ProtectedSoftwareSignerRecord,
): boolean {
  return (
    left.id === right.id &&
    left.kind === right.kind &&
    left.signerPublicKey === right.signerPublicKey &&
    left.envelopeJson === right.envelopeJson
  );
}

function normalizeName(value: string | null | undefined): string | null {
  if (value === null || value === undefined) {
    return null;
  }
  const normalized = value.trim();
  return normalized.length === 0 ? null : normalized;
}

function requireNonEmpty(value: string, field: string): string {
  const normalized = value.trim();
  if (normalized.length === 0) {
    throw new WalletLifecycleError("invalid-input", `${field} must not be empty`);
  }
  return normalized;
}

function requireRecordId(value: string, kind: "account" | "signer"): string {
  const normalized = value.trim();
  if (normalized.length === 0) {
    throw new WalletLifecycleError("invalid-input", `${kind} record id must not be empty`);
  }
  return normalized;
}

function unique(values: readonly string[]): string[] {
  return [...new Set(values)];
}
