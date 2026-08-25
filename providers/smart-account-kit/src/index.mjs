import { SmartAccountKitProvider } from './provider.mjs';
import { smartAccountKitConfig } from './config.mjs';

export * from './config.mjs';
export * from './provider.mjs';

export async function createSmartAccountKitProvider(options = {}) {
  const { SmartAccountKit } = await import('smart-account-kit');
  const config = smartAccountKitConfig(options);
  return new SmartAccountKitProvider(new SmartAccountKit(config), {
    nativeTokenContract: options.nativeTokenContract,
  });
}
