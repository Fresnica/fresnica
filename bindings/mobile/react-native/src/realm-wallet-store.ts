import type {
  AccountRecord,
  AccountSignerReference,
  ProtectedSoftwareSignerRecord,
  WalletStore,
  WalletStoreTransaction,
} from "./wallet-lifecycle.ts";

export const FRESNICA_WALLET_REALM_SCHEMA_VERSION = 1;

export const FRESNICA_ACCOUNT_SCHEMA_NAME = "FresnicaAccountRecord";
export const FRESNICA_SIGNER_SCHEMA_NAME = "FresnicaSignerRecord";
export const FRESNICA_ACCOUNT_SIGNER_REFERENCE_SCHEMA_NAME =
  "FresnicaAccountSignerReference";

export interface RealmPropertySchema {
  type: "string";
  optional?: boolean;
  indexed?: boolean;
}

export interface RealmObjectSchema {
  name: string;
  primaryKey?: string;
  properties: Record<string, RealmPropertySchema>;
}

export const FresnicaAccountRealmSchema: RealmObjectSchema = {
  name: FRESNICA_ACCOUNT_SCHEMA_NAME,
  primaryKey: "id",
  properties: {
    id: { type: "string" },
    address: { type: "string", indexed: true },
    kind: { type: "string" },
    network: { type: "string", indexed: true },
    name: { type: "string", optional: true },
  },
};

export const FresnicaSignerRealmSchema: RealmObjectSchema = {
  name: FRESNICA_SIGNER_SCHEMA_NAME,
  primaryKey: "id",
  properties: {
    id: { type: "string" },
    kind: { type: "string" },
    signerPublicKey: { type: "string", indexed: true },
    envelopeJson: { type: "string" },
  },
};

export const FresnicaAccountSignerReferenceRealmSchema: RealmObjectSchema = {
  name: FRESNICA_ACCOUNT_SIGNER_REFERENCE_SCHEMA_NAME,
  primaryKey: "id",
  properties: {
    id: { type: "string" },
    accountId: { type: "string", indexed: true },
    signerId: { type: "string", indexed: true },
  },
};

export const FRESNICA_WALLET_REALM_SCHEMAS: readonly RealmObjectSchema[] = [
  FresnicaAccountRealmSchema,
  FresnicaSignerRealmSchema,
  FresnicaAccountSignerReferenceRealmSchema,
];

/**
 * Minimal structural subset of Realm JS used by the wallet store.
 *
 * The mobile host owns the Realm instance, configuration and migrations. Keeping that lifecycle
 * outside this package lets Fresnica merge these schemas into an existing application Realm and
 * keeps the Account/Signer product model independent from a specific Realm package version.
 */
export interface RealmResultsLike extends Iterable<unknown> {
  readonly length: number;
  filtered(query: string, ...args: unknown[]): RealmResultsLike;
}

export interface RealmLike {
  write<T>(work: () => T): T;
  objectForPrimaryKey(type: string, primaryKey: string): unknown | null;
  objects(type: string): RealmResultsLike;
  create(type: string, values: Record<string, unknown>): unknown;
  delete(value: unknown): void;
}

interface ManagedAccountRecord {
  id: string;
  address: string;
  kind: string;
  network: string;
  name: string | null;
}

interface ManagedSignerRecord {
  id: string;
  kind: string;
  signerPublicKey: string;
  envelopeJson: string;
}

interface ManagedSignerReference {
  id: string;
  accountId: string;
  signerId: string;
}

/** WalletStore implementation backed by a host-owned Realm database. */
export class RealmWalletStore implements WalletStore {
  private readonly realm: RealmLike;

  constructor(realm: RealmLike) {
    this.realm = realm;
  }

  async getAccount(accountId: string): Promise<AccountRecord | null> {
    return readAccount(this.realm, accountId);
  }

  async listProtectedSoftwareSigners(): Promise<readonly ProtectedSoftwareSignerRecord[]> {
    return [...this.realm.objects(FRESNICA_SIGNER_SCHEMA_NAME)].map((value) =>
      copySigner(value as ManagedSignerRecord),
    );
  }

  async listSignerReferencesForAccount(
    accountId: string,
  ): Promise<readonly AccountSignerReference[]> {
    return readReferencesForAccount(this.realm, accountId);
  }

  async transaction<T>(work: (transaction: WalletStoreTransaction) => T): Promise<T> {
    return this.realm.write(() => work(new RealmWalletStoreTransaction(this.realm)));
  }
}

class RealmWalletStoreTransaction implements WalletStoreTransaction {
  private readonly realm: RealmLike;

  constructor(realm: RealmLike) {
    this.realm = realm;
  }

  getAccount(accountId: string): AccountRecord | null {
    return readAccount(this.realm, accountId);
  }

  getSigner(signerId: string): ProtectedSoftwareSignerRecord | null {
    return readSigner(this.realm, signerId);
  }

  listSignerReferencesForAccount(accountId: string): readonly AccountSignerReference[] {
    return readReferencesForAccount(this.realm, accountId);
  }

  countSignerReferences(signerId: string): number {
    return this.realm
      .objects(FRESNICA_ACCOUNT_SIGNER_REFERENCE_SCHEMA_NAME)
      .filtered("signerId == $0", signerId).length;
  }

  putAccount(account: AccountRecord): void {
    const existing = this.realm.objectForPrimaryKey(
      FRESNICA_ACCOUNT_SCHEMA_NAME,
      account.id,
    ) as ManagedAccountRecord | null;
    if (existing === null) {
      this.realm.create(FRESNICA_ACCOUNT_SCHEMA_NAME, { ...account });
      return;
    }
    existing.address = account.address;
    existing.kind = account.kind;
    existing.network = account.network;
    existing.name = account.name;
  }

  putSigner(signer: ProtectedSoftwareSignerRecord): void {
    const existing = this.realm.objectForPrimaryKey(
      FRESNICA_SIGNER_SCHEMA_NAME,
      signer.id,
    ) as ManagedSignerRecord | null;
    if (existing === null) {
      this.realm.create(FRESNICA_SIGNER_SCHEMA_NAME, { ...signer });
      return;
    }
    existing.kind = signer.kind;
    existing.signerPublicKey = signer.signerPublicKey;
    existing.envelopeJson = signer.envelopeJson;
  }

  putSignerReference(reference: AccountSignerReference): void {
    const id = referenceId(reference.accountId, reference.signerId);
    if (
      this.realm.objectForPrimaryKey(
        FRESNICA_ACCOUNT_SIGNER_REFERENCE_SCHEMA_NAME,
        id,
      ) !== null
    ) {
      return;
    }
    this.realm.create(FRESNICA_ACCOUNT_SIGNER_REFERENCE_SCHEMA_NAME, {
      id,
      accountId: reference.accountId,
      signerId: reference.signerId,
    });
  }

  deleteSignerReference(accountId: string, signerId: string): void {
    const existing = this.realm.objectForPrimaryKey(
      FRESNICA_ACCOUNT_SIGNER_REFERENCE_SCHEMA_NAME,
      referenceId(accountId, signerId),
    );
    if (existing !== null) {
      this.realm.delete(existing);
    }
  }

  deleteSigner(signerId: string): void {
    const existing = this.realm.objectForPrimaryKey(FRESNICA_SIGNER_SCHEMA_NAME, signerId);
    if (existing !== null) {
      this.realm.delete(existing);
    }
  }
}

function readAccount(realm: RealmLike, accountId: string): AccountRecord | null {
  const value = realm.objectForPrimaryKey(
    FRESNICA_ACCOUNT_SCHEMA_NAME,
    accountId,
  ) as ManagedAccountRecord | null;
  return value === null ? null : copyAccount(value);
}

function readSigner(
  realm: RealmLike,
  signerId: string,
): ProtectedSoftwareSignerRecord | null {
  const value = realm.objectForPrimaryKey(
    FRESNICA_SIGNER_SCHEMA_NAME,
    signerId,
  ) as ManagedSignerRecord | null;
  return value === null ? null : copySigner(value);
}

function readReferencesForAccount(
  realm: RealmLike,
  accountId: string,
): AccountSignerReference[] {
  return [
    ...realm
      .objects(FRESNICA_ACCOUNT_SIGNER_REFERENCE_SCHEMA_NAME)
      .filtered("accountId == $0", accountId),
  ].map((value) => {
    const reference = value as ManagedSignerReference;
    return { accountId: reference.accountId, signerId: reference.signerId };
  });
}

function copyAccount(value: ManagedAccountRecord): AccountRecord {
  if (value.kind !== "classic" && value.kind !== "contract") {
    throw new Error(`Invalid persisted Fresnica account kind: ${value.kind}`);
  }
  return {
    id: value.id,
    address: value.address,
    kind: value.kind,
    network: value.network,
    name: value.name,
  };
}

function copySigner(value: ManagedSignerRecord): ProtectedSoftwareSignerRecord {
  if (value.kind !== "protected-software") {
    throw new Error(`Invalid persisted Fresnica signer kind: ${value.kind}`);
  }
  return {
    id: value.id,
    kind: value.kind,
    signerPublicKey: value.signerPublicKey,
    envelopeJson: value.envelopeJson,
  };
}

function referenceId(accountId: string, signerId: string): string {
  return JSON.stringify([accountId, signerId]);
}
