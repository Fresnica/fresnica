import type {
  AccountRecord,
  AccountSignerReference,
  FresnicaCoreLifecycleBridge,
  ProtectedSoftwareSignerRecord,
  RecordIdFactory,
  WalletStore,
} from "./wallet-lifecycle.ts";

export const PORTABLE_BACKUP_FORMAT = "fresnica-wallet-backup";
export const PORTABLE_BACKUP_VERSION = 2;

export interface PortableBackupAccount {
  ref: string;
  address: string;
  suggestedNetwork: string;
  name: string | null;
}

export interface PortableBackupSigner {
  ref: string;
  kind: "protected-software";
  signerPublicKey: string;
  envelope: Record<string, unknown>;
}

export interface PortableBackupReference {
  accountRef: string;
  signerRef: string;
}

export interface PortableRecoverySource {
  ref: string;
  kind: "mnemonic";
  signerRefs: readonly string[];
}

export interface PortableWalletBackup {
  format: typeof PORTABLE_BACKUP_FORMAT;
  version: typeof PORTABLE_BACKUP_VERSION;
  accounts: readonly PortableBackupAccount[];
  signers: readonly PortableBackupSigner[];
  accountSignerReferences: readonly PortableBackupReference[];
  recoverySources: readonly PortableRecoverySource[];
}

export interface PortableBackupSnapshot {
  accounts: readonly AccountRecord[];
  signers: readonly ProtectedSoftwareSignerRecord[];
  references: readonly AccountSignerReference[];
  recoverySources?: readonly {
    kind: "mnemonic";
    signerIds: readonly string[];
  }[];
}

export type PendingReferenceReason =
  | "ledger-authorization-required"
  | "provider-authorization-required";

export interface PendingPortableReference extends AccountSignerReference {
  accountRef: string;
  signerRef: string;
  reason: PendingReferenceReason;
}

export interface RecoverySourceHint {
  ref: string;
  kind: "mnemonic";
  signerIds: readonly string[];
}

export interface StagedPortableRestore {
  accounts: readonly AccountRecord[];
  signers: readonly ProtectedSoftwareSignerRecord[];
  activeReferences: readonly AccountSignerReference[];
  pendingReferences: readonly PendingPortableReference[];
  recoverySourceHints: readonly RecoverySourceHint[];
}

export interface StagePortableRestoreInput {
  backup: PortableWalletBackup | unknown;
  backupPasscode: string;
  appPasscode: string;
  targetNetworks: Readonly<Record<string, string>>;
}

export type PortableBackupErrorCode =
  | "invalid-backup"
  | "network-unconfirmed"
  | "record-id-collision"
  | "invalid-reference";

export class PortableBackupError extends Error {
  readonly code: PortableBackupErrorCode;

  constructor(code: PortableBackupErrorCode, message: string) {
    super(message);
    this.name = "PortableBackupError";
    this.code = code;
  }
}

export function createPortableBackup(snapshot: PortableBackupSnapshot): PortableWalletBackup {
  const accountRefs = new Map(snapshot.accounts.map((account, index) => [account.id, `a${index + 1}`]));
  const signerRefs = new Map(snapshot.signers.map((signer, index) => [signer.id, `s${index + 1}`]));

  const backup: PortableWalletBackup = {
    format: PORTABLE_BACKUP_FORMAT,
    version: PORTABLE_BACKUP_VERSION,
    accounts: snapshot.accounts.map((account) => ({
      ref: accountRefs.get(account.id)!,
      address: account.address,
      suggestedNetwork: account.network,
      name: account.name,
    })),
    signers: snapshot.signers.map((signer) => ({
      ref: signerRefs.get(signer.id)!,
      kind: "protected-software",
      signerPublicKey: signer.signerPublicKey,
      envelope: parseEnvelopeJson(signer.envelopeJson),
    })),
    accountSignerReferences: snapshot.references.map((reference) => ({
      accountRef: requireMappedRef(accountRefs, reference.accountId, "account"),
      signerRef: requireMappedRef(signerRefs, reference.signerId, "signer"),
    })),
    recoverySources: (snapshot.recoverySources ?? []).map((source, index) => ({
      ref: `r${index + 1}`,
      kind: source.kind,
      signerRefs: source.signerIds.map((id) => requireMappedRef(signerRefs, id, "signer")),
    })),
  };
  return parsePortableBackup(backup);
}

export function encodePortableBackup(backup: PortableWalletBackup): string {
  return `${JSON.stringify(parsePortableBackup(backup), null, 2)}\n`;
}

export function decodePortableBackup(text: string): PortableWalletBackup {
  try {
    return parsePortableBackup(JSON.parse(text));
  } catch (error) {
    if (error instanceof PortableBackupError) throw error;
    throw new PortableBackupError("invalid-backup", "Portable backup is not valid JSON");
  }
}

export function parsePortableBackup(value: unknown): PortableWalletBackup {
  const root = object(value, "backup");
  exactKeys(root, [
    "format",
    "version",
    "accounts",
    "signers",
    "accountSignerReferences",
    "recoverySources",
  ]);
  if (root.format !== PORTABLE_BACKUP_FORMAT || root.version !== PORTABLE_BACKUP_VERSION) {
    throw new PortableBackupError("invalid-backup", "Unsupported portable backup format");
  }

  const accounts = array(root.accounts, "accounts").map(parseAccount);
  const signers = array(root.signers, "signers").map(parseSigner);
  const accountSignerReferences = array(
    root.accountSignerReferences,
    "accountSignerReferences",
  ).map(parseReference);
  const recoverySources = array(root.recoverySources, "recoverySources").map(parseRecoverySource);

  const accountRefs = uniqueRefs(accounts, "account");
  const signerRefs = uniqueRefs(signers, "signer");
  uniqueRefs(recoverySources, "recovery source");
  const relations = new Set<string>();
  for (const reference of accountSignerReferences) {
    if (!accountRefs.has(reference.accountRef) || !signerRefs.has(reference.signerRef)) {
      throw new PortableBackupError("invalid-backup", "Portable backup contains a dangling signer reference");
    }
    const key = relationKey(reference.accountRef, reference.signerRef);
    if (!relations.add(key)) {
      throw new PortableBackupError("invalid-backup", "Portable backup contains a duplicate signer reference");
    }
  }
  for (const source of recoverySources) {
    if (source.signerRefs.length === 0 || source.signerRefs.some((ref) => !signerRefs.has(ref))) {
      throw new PortableBackupError("invalid-backup", "Portable backup contains an invalid recovery-source reference");
    }
  }

  return {
    format: PORTABLE_BACKUP_FORMAT,
    version: PORTABLE_BACKUP_VERSION,
    accounts,
    signers,
    accountSignerReferences,
    recoverySources,
  };
}

export async function stagePortableRestore(
  core: FresnicaCoreLifecycleBridge,
  createId: RecordIdFactory,
  input: StagePortableRestoreInput,
): Promise<StagedPortableRestore> {
  const backup = parsePortableBackup(input.backup);
  const accounts = new Map<string, AccountRecord>();
  const signers = new Map<string, ProtectedSoftwareSignerRecord>();

  for (const source of backup.accounts) {
    const network = input.targetNetworks[source.ref]?.trim();
    if (!network) {
      throw new PortableBackupError(
        "network-unconfirmed",
        `Target network must be confirmed for ${source.ref}`,
      );
    }
    const identity = await core.parseAccount(source.address);
    if (identity.address !== source.address) {
      throw new PortableBackupError("invalid-backup", `Non-canonical account identity: ${source.ref}`);
    }
    accounts.set(source.ref, {
      id: requireRecordId(createId("account"), "account"),
      address: identity.address,
      kind: identity.kind,
      network,
      name: normalizeName(source.name),
    });
  }

  for (const source of backup.signers) {
    const identity = await core.parseAccount(source.signerPublicKey);
    if (identity.kind !== "classic" || identity.address !== source.signerPublicKey) {
      throw new PortableBackupError("invalid-backup", `Invalid software signer identity: ${source.ref}`);
    }
    const protectedSigner = await core.reprotect(
      JSON.stringify(source.envelope),
      input.backupPasscode,
      input.appPasscode,
      source.signerPublicKey,
    );
    if (protectedSigner.signerPublicKey !== source.signerPublicKey) {
      throw new PortableBackupError("invalid-backup", `Protected signer identity changed: ${source.ref}`);
    }
    signers.set(source.ref, {
      id: requireRecordId(createId("signer"), "signer"),
      kind: "protected-software",
      signerPublicKey: protectedSigner.signerPublicKey,
      envelopeJson: protectedSigner.envelopeJson,
    });
  }

  const activeReferences: AccountSignerReference[] = [];
  const pendingReferences: PendingPortableReference[] = [];
  for (const source of backup.accountSignerReferences) {
    const account = accounts.get(source.accountRef)!;
    const signer = signers.get(source.signerRef)!;
    if (account.kind === "classic" && account.address === signer.signerPublicKey) {
      activeReferences.push({ accountId: account.id, signerId: signer.id });
    } else {
      pendingReferences.push({
        accountId: account.id,
        signerId: signer.id,
        accountRef: source.accountRef,
        signerRef: source.signerRef,
        reason:
          account.kind === "classic"
            ? "ledger-authorization-required"
            : "provider-authorization-required",
      });
    }
  }

  return {
    accounts: [...accounts.values()],
    signers: [...signers.values()],
    activeReferences,
    pendingReferences,
    recoverySourceHints: backup.recoverySources.map((source) => ({
      ref: source.ref,
      kind: source.kind,
      signerIds: source.signerRefs.map((ref) => signers.get(ref)!.id),
    })),
  };
}

export async function commitPortableRestore(
  store: WalletStore,
  staged: StagedPortableRestore,
  validatedPendingReferences: readonly AccountSignerReference[] = [],
): Promise<void> {
  const pending = new Set(
    staged.pendingReferences.map((reference) => relationKey(reference.accountId, reference.signerId)),
  );
  for (const reference of validatedPendingReferences) {
    if (!pending.has(relationKey(reference.accountId, reference.signerId))) {
      throw new PortableBackupError("invalid-reference", "Validated restore reference is not pending");
    }
  }

  await store.transaction((transaction) => {
    for (const account of staged.accounts) {
      if (transaction.getAccount(account.id) !== null) {
        throw new PortableBackupError("record-id-collision", `Account record id already exists: ${account.id}`);
      }
      transaction.putAccount(account);
    }
    for (const signer of staged.signers) {
      if (transaction.getSigner(signer.id) !== null) {
        throw new PortableBackupError("record-id-collision", `Signer record id already exists: ${signer.id}`);
      }
      transaction.putSigner(signer);
    }
    for (const reference of [...staged.activeReferences, ...validatedPendingReferences]) {
      transaction.putSignerReference(reference);
    }
  });
}

function parseAccount(value: unknown): PortableBackupAccount {
  const raw = object(value, "account");
  exactKeys(raw, ["ref", "address", "suggestedNetwork", "name"]);
  return {
    ref: nonEmptyString(raw.ref, "account ref"),
    address: nonEmptyString(raw.address, "account address"),
    suggestedNetwork: nonEmptyString(raw.suggestedNetwork, "suggested network"),
    name: nullableString(raw.name, "account name"),
  };
}

function parseSigner(value: unknown): PortableBackupSigner {
  const raw = object(value, "signer");
  exactKeys(raw, ["ref", "kind", "signerPublicKey", "envelope"]);
  if (raw.kind !== "protected-software") {
    throw new PortableBackupError("invalid-backup", "Unsupported portable signer kind");
  }
  return {
    ref: nonEmptyString(raw.ref, "signer ref"),
    kind: "protected-software",
    signerPublicKey: nonEmptyString(raw.signerPublicKey, "signer public key"),
    envelope: object(raw.envelope, "signer envelope"),
  };
}

function parseReference(value: unknown): PortableBackupReference {
  const raw = object(value, "account signer reference");
  exactKeys(raw, ["accountRef", "signerRef"]);
  return {
    accountRef: nonEmptyString(raw.accountRef, "account ref"),
    signerRef: nonEmptyString(raw.signerRef, "signer ref"),
  };
}

function parseRecoverySource(value: unknown): PortableRecoverySource {
  const raw = object(value, "recovery source");
  exactKeys(raw, ["ref", "kind", "signerRefs"]);
  if (raw.kind !== "mnemonic") {
    throw new PortableBackupError("invalid-backup", "Unsupported recovery source kind");
  }
  return {
    ref: nonEmptyString(raw.ref, "recovery source ref"),
    kind: "mnemonic",
    signerRefs: array(raw.signerRefs, "recovery signer refs").map((ref) =>
      nonEmptyString(ref, "recovery signer ref"),
    ),
  };
}

function object(value: unknown, label: string): Record<string, unknown> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new PortableBackupError("invalid-backup", `${label} must be an object`);
  }
  return value as Record<string, unknown>;
}

function array(value: unknown, label: string): unknown[] {
  if (!Array.isArray(value)) {
    throw new PortableBackupError("invalid-backup", `${label} must be an array`);
  }
  return value;
}

function exactKeys(value: Record<string, unknown>, allowed: readonly string[]): void {
  const allowedKeys = new Set(allowed);
  const unexpected = Object.keys(value).find((key) => !allowedKeys.has(key));
  if (unexpected !== undefined) {
    throw new PortableBackupError("invalid-backup", `Unexpected portable backup field: ${unexpected}`);
  }
}

function nonEmptyString(value: unknown, label: string): string {
  if (typeof value !== "string" || value.trim().length === 0) {
    throw new PortableBackupError("invalid-backup", `${label} must be a non-empty string`);
  }
  return value;
}

function nullableString(value: unknown, label: string): string | null {
  if (value === null) return null;
  if (typeof value !== "string") {
    throw new PortableBackupError("invalid-backup", `${label} must be a string or null`);
  }
  return value;
}

function uniqueRefs(values: readonly { ref: string }[], label: string): Set<string> {
  const refs = new Set<string>();
  for (const value of values) {
    if (!refs.add(value.ref)) {
      throw new PortableBackupError("invalid-backup", `Duplicate ${label} ref: ${value.ref}`);
    }
  }
  return refs;
}

function requireMappedRef(
  refs: ReadonlyMap<string, string>,
  id: string,
  kind: "account" | "signer",
): string {
  const ref = refs.get(id);
  if (ref === undefined) {
    throw new PortableBackupError("invalid-backup", `Backup ${kind} reference points outside snapshot`);
  }
  return ref;
}

function parseEnvelopeJson(value: string): Record<string, unknown> {
  try {
    return object(JSON.parse(value), "signer envelope");
  } catch (error) {
    if (error instanceof PortableBackupError) throw error;
    throw new PortableBackupError("invalid-backup", "Signer envelope is not valid JSON");
  }
}

function requireRecordId(value: string, kind: "account" | "signer"): string {
  const id = value.trim();
  if (id.length === 0) {
    throw new PortableBackupError("invalid-backup", `${kind} record id must not be empty`);
  }
  return id;
}

function normalizeName(value: string | null): string | null {
  if (value === null) return null;
  const name = value.trim();
  return name.length === 0 ? null : name;
}

function relationKey(accountId: string, signerId: string): string {
  return JSON.stringify([accountId, signerId]);
}
