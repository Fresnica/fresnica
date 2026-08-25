import { Buffer } from 'buffer';
import {
  Address,
  buildAuthorizationEntryPreimage,
  hash,
  xdr,
} from '@stellar/stellar-sdk';

import {
  SMART_ACCOUNT_KIT_PROVIDER_ID,
  SMART_ACCOUNT_KIT_VERSION,
  STELLAR_TESTNET_SMART_ACCOUNT,
} from './config.mjs';
import { SMART_ACCOUNT_AUTH_FIXTURE_SCHEMA } from './conformance-recorder.mjs';

function requireText(value, field) {
  if (typeof value !== 'string' || value.trim() === '') {
    throw new TypeError(`${field} is required`);
  }
  return value;
}

function equalBytes(left, right) {
  return Buffer.compare(Buffer.from(left), Buffer.from(right)) === 0;
}

function decodeBase64Url(value) {
  const normalized = requireText(value, 'base64url value')
    .replace(/-/g, '+')
    .replace(/_/g, '/');
  const padding = '='.repeat((4 - (normalized.length % 4)) % 4);
  return Buffer.from(normalized + padding, 'base64');
}

function encodeBase64Url(value) {
  return Buffer.from(value)
    .toString('base64')
    .replace(/=/g, '')
    .replace(/\+/g, '-')
    .replace(/\//g, '_');
}

function addressCredentials(credentials) {
  switch (credentials.switch().name) {
    case 'sorobanCredentialsAddress':
      return credentials.address();
    case 'sorobanCredentialsAddressV2':
      return credentials.addressV2();
    case 'sorobanCredentialsAddressWithDelegates':
      throw new Error('ADDRESS_WITH_DELEGATES fixture validation is not implemented');
    default:
      throw new Error(`unsupported auth credential type: ${credentials.switch().name}`);
  }
}

function mapBySymbol(value, field) {
  if (value.switch().name !== 'scvMap') {
    throw new Error(`${field} is not an ScMap`);
  }
  const result = new Map();
  for (const entry of value.map() ?? []) {
    const key = entry.key();
    if (key.switch().name === 'scvSymbol') {
      result.set(key.sym().toString(), entry.val());
    }
  }
  return result;
}

function readContextRuleIds(signature) {
  const payload = mapBySymbol(signature, 'AuthPayload');
  const value = payload.get('context_rule_ids');
  if (!value || value.switch().name !== 'scvVec') {
    throw new Error('AuthPayload.context_rule_ids is missing or not a vector');
  }
  const ids = (value.vec() ?? []).map((item) => {
    if (item.switch().name !== 'scvU32') {
      throw new Error('AuthPayload.context_rule_ids contains a non-u32 value');
    }
    return item.u32();
  });
  if (ids.length === 0) {
    throw new Error('AuthPayload.context_rule_ids must not be empty');
  }
  return ids;
}

function readSignerEntries(signature) {
  const payload = mapBySymbol(signature, 'AuthPayload');
  const value = payload.get('signers');
  if (!value || value.switch().name !== 'scvMap') {
    throw new Error('AuthPayload.signers is missing or not a map');
  }
  return value.map() ?? [];
}

function parseExternalSigner(value) {
  if (value.switch().name !== 'scvVec') return null;
  const items = value.vec() ?? [];
  if (items.length < 3 || items[0].switch().name !== 'scvSymbol') return null;
  if (items[0].sym().toString() !== 'External') return null;
  if (items[1].switch().name !== 'scvAddress' || items[2].switch().name !== 'scvBytes') {
    throw new Error('External signer has invalid verifier/key data');
  }
  return {
    verifier: Address.fromScAddress(items[1].address()).toString(),
    keyData: Buffer.from(items[2].bytes()),
  };
}

function parseWebAuthnSignature(value) {
  if (value.switch().name !== 'scvBytes') {
    throw new Error('AuthPayload signer signature is not bytes');
  }
  const encoded = xdr.ScVal.fromXDR(Buffer.from(value.bytes()));
  const fields = mapBySymbol(encoded, 'WebAuthnSigData');
  const readBytes = (name) => {
    const item = fields.get(name);
    if (!item || item.switch().name !== 'scvBytes') {
      throw new Error(`WebAuthnSigData.${name} is missing or not bytes`);
    }
    return Buffer.from(item.bytes());
  };
  return {
    authenticatorData: readBytes('authenticator_data'),
    clientData: readBytes('client_data'),
    signature: readBytes('signature'),
  };
}

function authDigest(networkPassphrase, entry, expiration, contextRuleIds) {
  const preimage = buildAuthorizationEntryPreimage(entry, expiration, networkPassphrase);
  const signaturePayload = hash(preimage.toXDR());
  const ruleIdsXdr = xdr.ScVal.scvVec(
    contextRuleIds.map((id) => xdr.ScVal.scvU32(id)),
  ).toXDR();
  return {
    signaturePayload,
    authDigest: hash(Buffer.concat([signaturePayload, ruleIdsXdr])),
  };
}

function requireAuthenticatorFlags(authenticatorData) {
  if (authenticatorData.length < 37) {
    throw new Error('WebAuthn authenticatorData must be at least 37 bytes');
  }
  const flags = authenticatorData[32];
  if ((flags & 0x01) === 0) {
    throw new Error('WebAuthn authenticator User Present (UP) flag is not set');
  }
  if ((flags & 0x04) === 0) {
    throw new Error('WebAuthn authenticator User Verified (UV) flag is not set');
  }
}

async function verifyP256Signature(publicKey, webAuthn) {
  if (!globalThis.crypto?.subtle) {
    throw new Error('Web Crypto subtle API is required for fixture validation');
  }
  if (publicKey.length !== 65 || publicKey[0] !== 0x04) {
    throw new Error('WebAuthn signer keyData does not contain a 65-byte uncompressed P-256 public key');
  }
  if (webAuthn.signature.length !== 64) {
    throw new Error('WebAuthn compact P-256 signature must be 64 bytes');
  }

  requireAuthenticatorFlags(webAuthn.authenticatorData);

  const clientDataHash = Buffer.from(await crypto.subtle.digest('SHA-256', webAuthn.clientData));
  const signedData = Buffer.concat([webAuthn.authenticatorData, clientDataHash]);
  const key = await crypto.subtle.importKey(
    'raw',
    publicKey,
    { name: 'ECDSA', namedCurve: 'P-256' },
    false,
    ['verify'],
  );
  return crypto.subtle.verify(
    { name: 'ECDSA', hash: 'SHA-256' },
    key,
    webAuthn.signature,
    signedData,
  );
}

function validateFixtureIdentity(fixture) {
  if (!fixture || fixture.schema !== SMART_ACCOUNT_AUTH_FIXTURE_SCHEMA) {
    throw new TypeError(`fixture schema must be ${SMART_ACCOUNT_AUTH_FIXTURE_SCHEMA}`);
  }
  if (fixture.provider?.id !== SMART_ACCOUNT_KIT_PROVIDER_ID) {
    throw new Error('fixture provider id does not match the Fresnica smart-account provider');
  }
  if (fixture.provider?.smartAccountKitVersion !== SMART_ACCOUNT_KIT_VERSION) {
    throw new Error(`fixture smart-account-kit version must be ${SMART_ACCOUNT_KIT_VERSION}`);
  }
  for (const [field, expected] of [
    ['accountWasmHash', STELLAR_TESTNET_SMART_ACCOUNT.accountWasmHash],
    ['webauthnVerifierAddress', STELLAR_TESTNET_SMART_ACCOUNT.webauthnVerifierAddress],
  ]) {
    if (fixture.provider?.[field] !== expected) {
      throw new Error(`fixture ${field} does not match the pinned Testnet deployment`);
    }
  }
  if (fixture.network?.passphrase !== STELLAR_TESTNET_SMART_ACCOUNT.networkPassphrase) {
    throw new Error('fixture network passphrase does not match the pinned Testnet deployment');
  }
}

export async function verifySmartAccountAuthFixture(fixture) {
  validateFixtureIdentity(fixture);
  const accountAddress = requireText(fixture.account?.address, 'fixture account address');
  const credentialId = requireText(fixture.account?.credentialId, 'fixture credentialId');
  const authXdr = fixture.invocation?.authXdr;
  if (!Array.isArray(authXdr) || authXdr.length === 0) {
    throw new TypeError('fixture invocation.authXdr must contain at least one auth entry');
  }

  // Decode the host function as a guard against accidentally capturing an
  // unrelated JSON request that merely has func/auth keys.
  xdr.HostFunction.fromXDR(requireText(fixture.invocation?.funcXdr, 'fixture funcXdr'), 'base64');

  const entries = [];
  for (const [index, encoded] of authXdr.entries()) {
    const entry = xdr.SorobanAuthorizationEntry.fromXDR(requireText(encoded, `authXdr[${index}]`), 'base64');
    const credentials = addressCredentials(entry.credentials());
    const address = Address.fromScAddress(credentials.address()).toString();
    if (address !== accountAddress) {
      throw new Error(`auth entry ${index} address ${address} does not match fixture account ${accountAddress}`);
    }

    const contextRuleIds = readContextRuleIds(credentials.signature());
    const { signaturePayload, authDigest: digest } = authDigest(
      fixture.network.passphrase,
      entry,
      credentials.signatureExpirationLedger(),
      contextRuleIds,
    );

    let matchingSigner = null;
    for (const signerEntry of readSignerEntries(credentials.signature())) {
      const signer = parseExternalSigner(signerEntry.key());
      if (!signer || signer.verifier !== STELLAR_TESTNET_SMART_ACCOUNT.webauthnVerifierAddress) continue;
      if (signer.keyData.length <= 65) {
        throw new Error('WebAuthn signer keyData is missing credential ID bytes');
      }
      const signerCredentialId = encodeBase64Url(signer.keyData.subarray(65));
      if (signerCredentialId !== credentialId) continue;
      matchingSigner = { signer, webAuthn: parseWebAuthnSignature(signerEntry.val()) };
      break;
    }
    if (!matchingSigner) {
      throw new Error(`auth entry ${index} has no matching pinned WebAuthn signer`);
    }

    let clientData;
    try {
      clientData = JSON.parse(matchingSigner.webAuthn.clientData.toString('utf8'));
    } catch {
      throw new Error(`auth entry ${index} contains invalid WebAuthn clientDataJSON`);
    }
    if (clientData.type !== 'webauthn.get') {
      throw new Error(`auth entry ${index} clientData type is not webauthn.get`);
    }
    if (!equalBytes(decodeBase64Url(clientData.challenge), digest)) {
      throw new Error(`auth entry ${index} WebAuthn challenge does not match the Protocol 27 auth digest`);
    }

    const signatureValid = await verifyP256Signature(
      matchingSigner.signer.keyData.subarray(0, 65),
      matchingSigner.webAuthn,
    );
    if (!signatureValid) {
      throw new Error(`auth entry ${index} WebAuthn P-256 signature is invalid`);
    }

    entries.push(Object.freeze({
      index,
      address,
      contextRuleIds: Object.freeze([...contextRuleIds]),
      signatureExpirationLedger: credentials.signatureExpirationLedger(),
      signaturePayloadHex: Buffer.from(signaturePayload).toString('hex'),
      authDigestHex: Buffer.from(digest).toString('hex'),
      origin: requireText(clientData.origin, `auth entry ${index} WebAuthn origin`),
      credentialId,
      verifier: matchingSigner.signer.verifier,
    }));
  }

  return Object.freeze({
    schema: fixture.schema,
    transactionHash: requireText(fixture.transaction?.hash, 'fixture transaction hash'),
    ...(Number.isInteger(fixture.transaction?.ledger) ? { ledger: fixture.transaction.ledger } : {}),
    accountAddress,
    credentialId,
    entries: Object.freeze(entries),
  });
}
