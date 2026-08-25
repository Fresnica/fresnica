function requireWebAuthnFunction(webAuthn, name) {
  if (!webAuthn || typeof webAuthn[name] !== 'function') {
    throw new TypeError(`webAuthn.${name} is required`);
  }
  return webAuthn[name].bind(webAuthn);
}

/**
 * Force user verification for every passkey operation.
 *
 * The reviewed Protocol 27 WebAuthn verifier rejects assertions without the UV
 * authenticator flag. smart-account-kit 0.6.2 requests `preferred`, which can
 * legally return UV=0. Fresnica tightens that provider boundary to `required`
 * for registration, discovery authentication and transaction signing alike.
 */
export function requireUserVerification(webAuthn) {
  const startRegistration = requireWebAuthnFunction(webAuthn, 'startRegistration');
  const startAuthentication = requireWebAuthnFunction(webAuthn, 'startAuthentication');

  return Object.freeze({
    async startRegistration({ optionsJSON }) {
      return startRegistration({
        optionsJSON: {
          ...optionsJSON,
          authenticatorSelection: {
            ...optionsJSON?.authenticatorSelection,
            userVerification: 'required',
          },
        },
      });
    },

    async startAuthentication({ optionsJSON }) {
      return startAuthentication({
        optionsJSON: {
          ...optionsJSON,
          userVerification: 'required',
        },
      });
    },
  });
}

export async function defaultRequiredUserVerificationWebAuthn() {
  const browser = await import('@simplewebauthn/browser');
  return requireUserVerification({
    startRegistration: browser.startRegistration,
    startAuthentication: browser.startAuthentication,
  });
}
