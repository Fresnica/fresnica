import { Buffer } from 'buffer';
import {
  createSmartAccountKitProvider,
  SMART_ACCOUNT_KIT_VERSION,
  STELLAR_TESTNET_SMART_ACCOUNT,
} from '../src/index.mjs';

globalThis.Buffer = Buffer;

const logElement = document.querySelector('#log');
const userElement = document.querySelector('#user');
const recipientElement = document.querySelector('#recipient');
const amountElement = document.querySelector('#amount');

function log(label, value) {
  const text = typeof value === 'string' ? value : JSON.stringify(value, null, 2);
  logElement.textContent = `${label}\n${text}\n\n${logElement.textContent}`;
}

function reportError(error) {
  console.error(error);
  log('ERROR', { name: error?.name, message: error?.message ?? String(error) });
}

if (!window.isSecureContext || typeof PublicKeyCredential === 'undefined') {
  throw new Error('WebAuthn requires a secure context and PublicKeyCredential support; use localhost or HTTPS');
}

const provider = await createSmartAccountKitProvider({
  rpName: 'Fresnica Testnet Smoke',
});

log('Provider', {
  provider: provider.providerId,
  smartAccountKit: SMART_ACCOUNT_KIT_VERSION,
  network: STELLAR_TESTNET_SMART_ACCOUNT.network,
  deploymentDate: STELLAR_TESTNET_SMART_ACCOUNT.deploymentDate,
});

async function run(button, action) {
  button.disabled = true;
  try {
    await action();
  } catch (error) {
    reportError(error);
  } finally {
    button.disabled = false;
  }
}

document.querySelector('#create').addEventListener('click', (event) => run(event.currentTarget, async () => {
  const result = await provider.createAccount({
    appName: 'Fresnica Testnet Smoke',
    userName: userElement.value.trim(),
    submit: true,
    autoFund: true,
    nickname: 'Fresnica passkey',
  });
  log('Created', result);
}));

document.querySelector('#restore').addEventListener('click', (event) => run(event.currentTarget, async () => {
  log('Restored', await provider.restore());
}));

document.querySelector('#discover').addEventListener('click', (event) => run(event.currentTarget, async () => {
  const discovered = await provider.authenticateAndDiscover();
  log('Discovered', discovered);
  if (!provider.activeAccount() && discovered.accounts.length === 1) {
    const connected = await provider.connect({
      accountAddress: discovered.accounts[0].accountAddress,
      credentialId: discovered.credentialId,
    });
    log('Connected', connected);
  }
}));

document.querySelector('#transfer').addEventListener('click', (event) => run(event.currentTarget, async () => {
  if (!provider.activeAccount()) throw new Error('Connect or create a smart account first');
  const amount = Number(amountElement.value);
  const result = await provider.transfer({
    recipient: recipientElement.value.trim(),
    amount,
  });
  log('Transfer', result);
}));

document.querySelector('#disconnect').addEventListener('click', (event) => run(event.currentTarget, async () => {
  await provider.disconnect();
  log('Disconnected', 'OK');
}));
