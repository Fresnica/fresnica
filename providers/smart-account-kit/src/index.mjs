import { SmartAccountKitProvider } from './provider.mjs';
import { smartAccountKitConfig } from './config.mjs';
import {
  defaultRequiredUserVerificationWebAuthn,
  requireUserVerification,
} from './webauthn.mjs';

export * from './config.mjs';
export * from './provider.mjs';
export * from './webauthn.mjs';

export async function createSmartAccountKitProvider(options = {}) {
  const { SmartAccountKit } = await import('smart-account-kit');
  const webAuthn = options.webAuthn
    ? requireUserVerification(options.webAuthn)
    : await defaultRequiredUserVerificationWebAuthn();
  const config = smartAccountKitConfig({ ...options, webAuthn });
  return new SmartAccountKitProvider(new SmartAccountKit(config), {
    nativeTokenContract: options.nativeTokenContract,
  });
}
