import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

const outDir = resolve(process.argv[2] ?? new URL('../build/web', import.meta.url).pathname);
const modulePath = resolve(outDir, 'fresnica_sdk.js');
const wasmPath = resolve(outDir, 'fresnica_sdk_bg.wasm');
const vectorPath = new URL('../../../spec/test-vectors/transaction-signing-v1.json', import.meta.url);

const wasm = await import(pathToFileURL(modulePath).href);
wasm.initSync({ module: readFileSync(wasmPath) });

const vector = JSON.parse(readFileSync(vectorPath, 'utf8'));
const testCase = vector.cases[0];
const unsigned = Uint8Array.from(Buffer.from(testCase.unsigned_xdr_base64, 'base64'));
const expectedSigned = Buffer.from(testCase.signed_xdr_base64, 'base64');
const expectedHash = Buffer.from(testCase.transaction_hash_hex, 'hex');
const signature = Uint8Array.from(Buffer.from(testCase.signature_hex, 'hex'));

const sdk = new wasm.FresnicaWasmSdk();
try {
  const version = sdk.version();
  assert.equal(version.wasmBindingApiVersion, 1);
  assert.ok(version.sdkApiVersion >= 2);
  assert.ok(version.coreClientApiVersion >= 1);

  const identity = sdk.parseAccount(testCase.public_key);
  assert.equal(identity.kind, 'classic');
  assert.equal(identity.address, testCase.public_key);
  assert.equal(identity.publicKey, testCase.public_key);

  const protectedSigner = sdk.protectSecret(
    testCase.secret,
    'browser-passcode',
    testCase.public_key,
  );
  assert.equal(protectedSigner.signerPublicKey, testCase.public_key);
  assert.equal(protectedSigner.envelopeJson.includes(testCase.secret), false);

  const signed = sdk.signTransactionXdrWithPasscode(
    protectedSigner.envelopeJson,
    'browser-passcode',
    testCase.public_key,
    unsigned,
    testCase.network_passphrase,
  );
  assert.deepEqual(Buffer.from(signed), expectedSigned);

  assert.throws(
    () => sdk.signTransactionXdrWithPasscode(
      protectedSigner.envelopeJson,
      'wrong-passcode',
      testCase.public_key,
      unsigned,
      testCase.network_passphrase,
    ),
    (error) => {
      assert.equal(error.name, 'FresnicaSdkError');
      assert.equal(error.code, 'invalid-passcode');
      return true;
    },
  );

  const request = sdk.prepareEd25519Signing(unsigned, testCase.network_passphrase);
  assert.ok(request.transactionHash instanceof Uint8Array);
  assert.ok(request.transactionXdr instanceof Uint8Array);
  assert.deepEqual(Buffer.from(request.transactionHash), expectedHash);
  assert.deepEqual(Buffer.from(request.transactionXdr), Buffer.from(unsigned));
  assert.equal(request.networkPassphrase, testCase.network_passphrase);

  const externallySigned = sdk.applyEd25519Signature(
    unsigned,
    testCase.network_passphrase,
    testCase.public_key,
    signature,
  );
  assert.deepEqual(Buffer.from(externallySigned), expectedSigned);

  const revealed = sdk.reveal(
    protectedSigner.envelopeJson,
    'browser-passcode',
    testCase.public_key,
  );
  assert.equal(revealed.kind, 'secret');
  assert.equal(revealed.secret, testCase.secret);

  console.log('Fresnica WASM runtime conformance: OK');
} finally {
  sdk.free();
}
