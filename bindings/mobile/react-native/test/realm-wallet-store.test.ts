import assert from "node:assert/strict";
import test from "node:test";

import {
  FRESNICA_ACCOUNT_SCHEMA_NAME,
  FRESNICA_ACCOUNT_SIGNER_REFERENCE_SCHEMA_NAME,
  FRESNICA_SIGNER_SCHEMA_NAME,
  FRESNICA_WALLET_REALM_SCHEMAS,
  FRESNICA_WALLET_REALM_SCHEMA_VERSION,
  RealmWalletStore,
  type RealmLike,
  type RealmResultsLike,
} from "../src/realm-wallet-store.ts";
import type {
  AccountRecord,
  ProtectedSoftwareSignerRecord,
} from "../src/wallet-lifecycle.ts";

class FakeResults implements RealmResultsLike {
  private readonly values: readonly Record<string, unknown>[];

  constructor(values: readonly Record<string, unknown>[]) {
    this.values = values;
  }

  get length(): number {
    return this.values.length;
  }

  *[Symbol.iterator](): Iterator<unknown> {
    yield* this.values;
  }

  filtered(query: string, ...args: unknown[]): RealmResultsLike {
    const match = /^(accountId|signerId) == \$0$/.exec(query);
    if (match === null) {
      throw new Error(`Unsupported fake Realm query: ${query}`);
    }
    const field = match[1] as "accountId" | "signerId";
    return new FakeResults(this.values.filter((value) => value[field] === args[0]));
  }
}

class FakeRealm implements RealmLike {
  private tables = new Map<string, Map<string, Record<string, unknown>>>();

  write<T>(work: () => T): T {
    const before = structuredClone(this.tables);
    try {
      return work();
    } catch (error) {
      this.tables = before;
      throw error;
    }
  }

  objectForPrimaryKey(type: string, primaryKey: string): unknown | null {
    return this.table(type).get(primaryKey) ?? null;
  }

  objects(type: string): RealmResultsLike {
    return new FakeResults([...this.table(type).values()]);
  }

  create(type: string, values: Record<string, unknown>): unknown {
    const id = values.id;
    assert.equal(typeof id, "string");
    if (this.table(type).has(id)) {
      throw new Error(`duplicate primary key: ${type}/${id}`);
    }
    const managed = { ...values };
    this.table(type).set(id, managed);
    return managed;
  }

  delete(value: unknown): void {
    for (const table of this.tables.values()) {
      for (const [id, object] of table) {
        if (object === value) {
          table.delete(id);
          return;
        }
      }
    }
  }

  private table(type: string): Map<string, Record<string, unknown>> {
    let table = this.tables.get(type);
    if (table === undefined) {
      table = new Map();
      this.tables.set(type, table);
    }
    return table;
  }
}

function account(id: string): AccountRecord {
  return {
    id,
    address: `G${id.toUpperCase()}`,
    kind: "classic",
    network: "public",
    name: null,
  };
}

function signer(id: string): ProtectedSoftwareSignerRecord {
  return {
    id,
    kind: "protected-software",
    signerPublicKey: `G${id.toUpperCase()}`,
    envelopeJson: `{"id":"${id}"}`,
  };
}

test("Realm schemas persist account, signer and reference separately without wallet-type coupling", () => {
  assert.equal(FRESNICA_WALLET_REALM_SCHEMA_VERSION, 1);
  assert.deepEqual(
    FRESNICA_WALLET_REALM_SCHEMAS.map((schema) => schema.name),
    [
      FRESNICA_ACCOUNT_SCHEMA_NAME,
      FRESNICA_SIGNER_SCHEMA_NAME,
      FRESNICA_ACCOUNT_SIGNER_REFERENCE_SCHEMA_NAME,
    ],
  );

  const fields = FRESNICA_WALLET_REALM_SCHEMAS.flatMap((schema) =>
    Object.keys(schema.properties),
  );
  assert.equal(fields.includes("watchOnly"), false);
  assert.equal(fields.includes("walletType"), false);
  assert.equal(fields.includes("secret"), false);
  assert.equal(fields.includes("privateKey"), false);
});

test("RealmWalletStore writes and reads plain account/signer/reference snapshots", async () => {
  const realm = new FakeRealm();
  const store = new RealmWalletStore(realm);
  const accountA = account("account-a");
  const signerA = signer("signer-a");

  await store.transaction((transaction) => {
    transaction.putAccount(accountA);
    transaction.putSigner(signerA);
    transaction.putSignerReference({ accountId: accountA.id, signerId: signerA.id });
  });

  assert.deepEqual(await store.getAccount(accountA.id), accountA);
  assert.deepEqual(await store.listProtectedSoftwareSigners(), [signerA]);
  assert.deepEqual(await store.listSignerReferencesForAccount(accountA.id), [
    { accountId: accountA.id, signerId: signerA.id },
  ]);

  const returned = await store.getAccount(accountA.id);
  assert.notEqual(returned, realm.objectForPrimaryKey(FRESNICA_ACCOUNT_SCHEMA_NAME, accountA.id));
});

test("RealmWalletStore counts shared signer references before deleting an orphan", async () => {
  const realm = new FakeRealm();
  const store = new RealmWalletStore(realm);
  const accountA = account("account-a");
  const accountB = account("account-b");
  const sharedSigner = signer("shared");

  await store.transaction((transaction) => {
    transaction.putAccount(accountA);
    transaction.putAccount(accountB);
    transaction.putSigner(sharedSigner);
    transaction.putSignerReference({ accountId: accountA.id, signerId: sharedSigner.id });
    transaction.putSignerReference({ accountId: accountB.id, signerId: sharedSigner.id });
  });

  await store.transaction((transaction) => {
    transaction.deleteSignerReference(accountA.id, sharedSigner.id);
    assert.equal(transaction.countSignerReferences(sharedSigner.id), 1);
  });

  assert.deepEqual(await store.listSignerReferencesForAccount(accountB.id), [
    { accountId: accountB.id, signerId: sharedSigner.id },
  ]);
  assert.deepEqual(await store.listProtectedSoftwareSigners(), [sharedSigner]);
});

test("RealmWalletStore transaction rollback keeps the pre-write graph intact", async () => {
  const realm = new FakeRealm();
  const store = new RealmWalletStore(realm);
  const accountA = account("account-a");

  await assert.rejects(
    store.transaction((transaction) => {
      transaction.putAccount(accountA);
      transaction.putSigner(signer("signer-a"));
      throw new Error("abort");
    }),
    /abort/,
  );

  assert.equal(await store.getAccount(accountA.id), null);
  assert.deepEqual(await store.listProtectedSoftwareSigners(), []);
});
