import assert from 'node:assert/strict';
import test from 'node:test';

import { requireUserVerification } from '../src/webauthn.mjs';

test('forces required user verification during passkey registration', async () => {
  let received;
  const wrapped = requireUserVerification({
    async startRegistration(args) {
      received = args;
      return { id: 'registered' };
    },
    async startAuthentication() {
      throw new Error('not used');
    },
  });

  const original = {
    challenge: 'challenge',
    authenticatorSelection: {
      residentKey: 'preferred',
      userVerification: 'preferred',
    },
  };
  assert.deepEqual(
    await wrapped.startRegistration({ optionsJSON: original }),
    { id: 'registered' },
  );
  assert.deepEqual(received.optionsJSON.authenticatorSelection, {
    residentKey: 'preferred',
    userVerification: 'required',
  });
  assert.equal(original.authenticatorSelection.userVerification, 'preferred');
});

test('forces required user verification during authentication and signing', async () => {
  let received;
  const wrapped = requireUserVerification({
    async startRegistration() {
      throw new Error('not used');
    },
    async startAuthentication(args) {
      received = args;
      return { id: 'authenticated' };
    },
  });

  const original = {
    challenge: 'digest',
    rpId: 'localhost',
    userVerification: 'preferred',
  };
  assert.deepEqual(
    await wrapped.startAuthentication({ optionsJSON: original }),
    { id: 'authenticated' },
  );
  assert.deepEqual(received.optionsJSON, {
    challenge: 'digest',
    rpId: 'localhost',
    userVerification: 'required',
  });
  assert.equal(original.userVerification, 'preferred');
});

test('rejects incomplete WebAuthn adapters', () => {
  assert.throws(
    () => requireUserVerification({ startRegistration() {} }),
    /webAuthn\.startAuthentication is required/,
  );
});
