package com.fresnica.sdk.security;

import android.content.Context;
import android.content.SharedPreferences;
import android.os.Build;
import android.security.keystore.KeyGenParameterSpec;
import android.security.keystore.KeyProperties;
import android.util.Base64;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.security.GeneralSecurityException;
import java.security.KeyStore;
import java.security.MessageDigest;
import java.security.ProviderException;
import java.util.Arrays;
import java.util.UUID;

import javax.crypto.Cipher;
import javax.crypto.KeyGenerator;
import javax.crypto.SecretKey;
import javax.crypto.spec.GCMParameterSpec;

/**
 * Native-only storage for one Fresnica WalletUnlockKey per software signer.
 *
 * <p>The 32-byte unlock key is encrypted with a per-enrollment AndroidKeyStore AES-GCM key whose
 * use requires strong biometric authentication for every cryptographic operation. The encrypted
 * bytes and IV are stored in private SharedPreferences; the Keystore key is non-exportable.
 *
 * <p>This class deliberately does not invoke BiometricPrompt itself. Callers wrap the Cipher from
 * {@link EnrollmentSession#getCipher()} or {@link UnlockSession#getCipher()} in a
 * BiometricPrompt.CryptoObject, authenticate, and only then call the corresponding finish method.
 * This keeps the unlock key in native memory and prevents a successful biometric probe from being
 * confused with authorization of the actual key-decryption operation.
 */
public final class WalletUnlockKeyStore {
    public static final int UNLOCK_KEY_BYTES = 32;

    private static final String KEYSTORE = "AndroidKeyStore";
    private static final String PREFS = "fresnica_wallet_unlock_keys_v1";
    private static final String ALIAS_PREFIX = "fresnica.unlock.";
    private static final String TRANSFORMATION = "AES/GCM/NoPadding";
    private static final int AES_KEY_BITS = 256;
    private static final int GCM_TAG_BITS = 128;

    private final SharedPreferences preferences;

    public WalletUnlockKeyStore(Context context) {
        Context applicationContext = context.getApplicationContext();
        preferences = applicationContext.getSharedPreferences(PREFS, Context.MODE_PRIVATE);
    }

    /** Begins a recoverable enrollment without deleting any existing enrollment. */
    public EnrollmentSession beginEnrollment(String signerId)
            throws GeneralSecurityException, IOException {
        String digest = signerDigest(signerId);
        String alias = ALIAS_PREFIX + digest + "." + UUID.randomUUID();
        SecretKey key = generateUserAuthKey(alias);
        try {
            Cipher cipher = Cipher.getInstance(TRANSFORMATION);
            cipher.init(Cipher.ENCRYPT_MODE, key);
            return new EnrollmentSession(signerId, alias, cipher);
        } catch (GeneralSecurityException error) {
            deleteAlias(alias);
            throw error;
        }
    }

    /**
     * Completes enrollment after BiometricPrompt authenticated the session Cipher.
     *
     * <p>The previous enrollment, if any, is deleted only after the new encrypted record is
     * durably committed. If persistence fails, the new pending Keystore key is removed and the
     * previous enrollment remains usable.
     */
    public void finishEnrollment(EnrollmentSession session, byte[] unlockKey)
            throws GeneralSecurityException, IOException {
        if (unlockKey == null || unlockKey.length != UNLOCK_KEY_BYTES) {
            throw new GeneralSecurityException("WalletUnlockKey must be exactly 32 bytes");
        }

        byte[] ciphertext = session.cipher.doFinal(unlockKey);
        byte[] iv = session.cipher.getIV();
        if (iv == null || iv.length == 0) {
            deleteAlias(session.alias);
            throw new GeneralSecurityException("AndroidKeyStore did not provide an AES-GCM IV");
        }

        String recordKey = recordKey(session.signerId);
        StoredRecord previous = decodeRecord(preferences.getString(recordKey, null));
        StoredRecord replacement = new StoredRecord(session.alias, iv, ciphertext);

        if (!preferences.edit().putString(recordKey, replacement.encode()).commit()) {
            deleteAlias(session.alias);
            throw new IOException("Unable to persist Fresnica WalletUnlockKey metadata");
        }

        if (previous != null && !previous.alias.equals(session.alias)) {
            deleteAlias(previous.alias);
        }
    }

    /** Deletes a pending enrollment after cancellation or a failed biometric flow. */
    public void cancelEnrollment(EnrollmentSession session) throws GeneralSecurityException, IOException {
        deleteAlias(session.alias);
    }

    /** Begins one authenticated decrypt operation for an enrolled signer. */
    public UnlockSession beginUnlock(String signerId)
            throws GeneralSecurityException, IOException {
        StoredRecord record = requireRecord(signerId);
        SecretKey key = loadKey(record.alias);
        if (key == null) {
            throw new GeneralSecurityException("WalletUnlockKey Keystore key is missing");
        }

        Cipher cipher = Cipher.getInstance(TRANSFORMATION);
        cipher.init(Cipher.DECRYPT_MODE, key, new GCMParameterSpec(GCM_TAG_BITS, record.iv));
        return new UnlockSession(signerId, record.alias, cipher, record.ciphertext);
    }

    /**
     * Returns the 32-byte unlock key only after BiometricPrompt authenticated the session Cipher.
     * The caller must keep the result native-only and zero it immediately after the SDK call.
     */
    public byte[] finishUnlock(UnlockSession session) throws GeneralSecurityException {
        byte[] clear = session.cipher.doFinal(session.ciphertext);
        if (clear.length != UNLOCK_KEY_BYTES) {
            Arrays.fill(clear, (byte) 0);
            throw new GeneralSecurityException("Stored WalletUnlockKey has an invalid length");
        }
        return clear;
    }

    /** Best-effort enrollment status for UI metadata; actual use is verified by beginUnlock. */
    public boolean isEnrolled(String signerId) {
        try {
            StoredRecord record = decodeRecord(preferences.getString(recordKey(signerId), null));
            return record != null && keyStore().containsAlias(record.alias);
        } catch (Exception ignored) {
            return false;
        }
    }

    /** Removes encrypted metadata and the corresponding AndroidKeyStore key. */
    public void delete(String signerId) throws GeneralSecurityException, IOException {
        String key = recordKey(signerId);
        StoredRecord record = decodeRecord(preferences.getString(key, null));
        if (!preferences.edit().remove(key).commit()) {
            throw new IOException("Unable to remove Fresnica WalletUnlockKey metadata");
        }
        if (record != null) {
            deleteAlias(record.alias);
        }
    }

    private SecretKey generateUserAuthKey(String alias) throws GeneralSecurityException, IOException {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
            try {
                return generateKey(alias, true);
            } catch (ProviderException strongBoxUnavailable) {
                // StrongBox is an optimization, not a requirement. AndroidKeyStore still keeps
                // the fallback key non-exportable and may back it with the device TEE.
            }
        }
        return generateKey(alias, false);
    }

    private SecretKey generateKey(String alias, boolean strongBox)
            throws GeneralSecurityException {
        KeyGenerator generator = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, KEYSTORE);
        KeyGenParameterSpec.Builder builder = new KeyGenParameterSpec.Builder(
                alias,
                KeyProperties.PURPOSE_ENCRYPT | KeyProperties.PURPOSE_DECRYPT)
                .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                .setRandomizedEncryptionRequired(true)
                .setKeySize(AES_KEY_BITS)
                .setUserAuthenticationRequired(true)
                .setInvalidatedByBiometricEnrollment(true);

        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            builder.setUserAuthenticationParameters(0, KeyProperties.AUTH_BIOMETRIC_STRONG);
        } else {
            // On API 23-29, -1 means biometric authentication is required for every use.
            builder.setUserAuthenticationValidityDurationSeconds(-1);
        }

        if (strongBox && Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
            builder.setIsStrongBoxBacked(true);
        }

        generator.init(builder.build());
        return generator.generateKey();
    }

    private StoredRecord requireRecord(String signerId) throws GeneralSecurityException {
        StoredRecord record = decodeRecord(preferences.getString(recordKey(signerId), null));
        if (record == null) {
            throw new GeneralSecurityException("No system-auth WalletUnlockKey enrollment exists");
        }
        return record;
    }

    private SecretKey loadKey(String alias) throws GeneralSecurityException, IOException {
        return (SecretKey) keyStore().getKey(alias, null);
    }

    private void deleteAlias(String alias) throws GeneralSecurityException, IOException {
        KeyStore store = keyStore();
        if (store.containsAlias(alias)) {
            store.deleteEntry(alias);
        }
    }

    private static KeyStore keyStore() throws GeneralSecurityException, IOException {
        KeyStore store = KeyStore.getInstance(KEYSTORE);
        store.load(null);
        return store;
    }

    private static String recordKey(String signerId) throws GeneralSecurityException {
        return "signer." + signerDigest(signerId);
    }

    private static String signerDigest(String signerId) throws GeneralSecurityException {
        if (signerId == null || signerId.trim().isEmpty()) {
            throw new GeneralSecurityException("signerId must not be empty");
        }
        MessageDigest digest = MessageDigest.getInstance("SHA-256");
        byte[] hash = digest.digest(signerId.getBytes(StandardCharsets.UTF_8));
        StringBuilder text = new StringBuilder(hash.length * 2);
        for (byte value : hash) {
            text.append(String.format("%02x", value & 0xff));
        }
        Arrays.fill(hash, (byte) 0);
        return text.toString();
    }

    private static StoredRecord decodeRecord(String encoded) {
        if (encoded == null) {
            return null;
        }
        String[] fields = encoded.split("\\|", -1);
        if (fields.length != 3 || fields[0].isEmpty()) {
            return null;
        }
        try {
            return new StoredRecord(
                    fields[0],
                    Base64.decode(fields[1], Base64.NO_WRAP),
                    Base64.decode(fields[2], Base64.NO_WRAP));
        } catch (IllegalArgumentException ignored) {
            return null;
        }
    }

    public static final class EnrollmentSession {
        private final String signerId;
        private final String alias;
        private final Cipher cipher;

        private EnrollmentSession(String signerId, String alias, Cipher cipher) {
            this.signerId = signerId;
            this.alias = alias;
            this.cipher = cipher;
        }

        public Cipher getCipher() {
            return cipher;
        }
    }

    public static final class UnlockSession {
        private final String signerId;
        private final String alias;
        private final Cipher cipher;
        private final byte[] ciphertext;

        private UnlockSession(String signerId, String alias, Cipher cipher, byte[] ciphertext) {
            this.signerId = signerId;
            this.alias = alias;
            this.cipher = cipher;
            this.ciphertext = ciphertext;
        }

        public Cipher getCipher() {
            return cipher;
        }

        public String getSignerId() {
            return signerId;
        }
    }

    private static final class StoredRecord {
        private final String alias;
        private final byte[] iv;
        private final byte[] ciphertext;

        private StoredRecord(String alias, byte[] iv, byte[] ciphertext) {
            this.alias = alias;
            this.iv = iv;
            this.ciphertext = ciphertext;
        }

        private String encode() {
            return alias
                    + "|"
                    + Base64.encodeToString(iv, Base64.NO_WRAP)
                    + "|"
                    + Base64.encodeToString(ciphertext, Base64.NO_WRAP);
        }
    }
}
