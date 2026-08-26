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
import java.security.KeyFactory;
import java.security.KeyPairGenerator;
import java.security.KeyStore;
import java.security.MessageDigest;
import java.security.PrivateKey;
import java.security.ProviderException;
import java.security.PublicKey;
import java.security.SecureRandom;
import java.security.spec.MGF1ParameterSpec;
import java.security.spec.X509EncodedKeySpec;
import java.util.Arrays;
import java.util.Map;
import java.util.UUID;

import javax.crypto.Cipher;
import javax.crypto.spec.OAEPParameterSpec;
import javax.crypto.spec.PSource;

/**
 * Device-level system-auth protection domain for Fresnica WalletUnlockKeys.
 *
 * <p>One auth-bound AndroidKeyStore RSA private key protects all local software signers on this
 * installation. The public key can wrap a newly derived per-signer WalletUnlockKey without user
 * authentication. The private key requires strong biometric authentication for every unwrap.
 * Each signer still has an independent Core envelope and WalletUnlockKey; only the OS protection
 * domain is shared.
 */
public final class WalletUnlockKeyStore {
    public static final int UNLOCK_KEY_BYTES = 32;

    private static final String KEYSTORE = "AndroidKeyStore";
    private static final String PREFS = "fresnica_system_auth_domain_v2";
    private static final String ACTIVE_ALIAS = "active_alias";
    private static final String SIGNER_PREFIX = "signer.";
    private static final String ALIAS_PREFIX = "fresnica.systemauth.";
    private static final String TRANSFORMATION = "RSA/ECB/OAEPWithSHA-256AndMGF1Padding";
    private static final int RSA_KEY_BITS = 2048;
    private static final int CHALLENGE_BYTES = 32;
    private static final OAEPParameterSpec OAEP_PARAMETERS = new OAEPParameterSpec(
            "SHA-256",
            "MGF1",
            MGF1ParameterSpec.SHA1,
            PSource.PSpecified.DEFAULT);

    private final SharedPreferences preferences;

    public WalletUnlockKeyStore(Context context) {
        Context applicationContext = context.getApplicationContext();
        preferences = applicationContext.getSharedPreferences(PREFS, Context.MODE_PRIVATE);
    }

    /** Creates a pending device protection domain and an authenticated proof operation. */
    public DomainEnrollmentSession beginDomainEnrollment()
            throws GeneralSecurityException, IOException {
        String alias = ALIAS_PREFIX + UUID.randomUUID();
        generateUserAuthKeyPair(alias);
        byte[] challenge = new byte[CHALLENGE_BYTES];
        new SecureRandom().nextBytes(challenge);
        try {
            byte[] ciphertext = encryptWithPublicKey(alias, challenge);
            Cipher cipher = decryptCipher(alias);
            return new DomainEnrollmentSession(alias, cipher, ciphertext, challenge);
        } catch (GeneralSecurityException | IOException error) {
            Arrays.fill(challenge, (byte) 0);
            deleteAlias(alias);
            throw error;
        }
    }

    /** Commits the pending domain only after BiometricPrompt authorized the private-key Cipher. */
    public void finishDomainEnrollment(DomainEnrollmentSession session)
            throws GeneralSecurityException, IOException {
        byte[] clear = null;
        boolean committed = false;
        try {
            clear = session.cipher.doFinal(session.ciphertext);
            if (!MessageDigest.isEqual(clear, session.challenge)) {
                throw new GeneralSecurityException("System-auth domain challenge verification failed");
            }

            String previousAlias = activeAlias();
            SharedPreferences.Editor editor = preferences.edit().putString(ACTIVE_ALIAS, session.alias);
            for (String key : preferences.getAll().keySet()) {
                if (key.startsWith(SIGNER_PREFIX)) {
                    editor.remove(key);
                }
            }
            if (!editor.commit()) {
                throw new IOException("Unable to persist Fresnica system-auth domain");
            }
            committed = true;
            if (previousAlias != null && !previousAlias.equals(session.alias)) {
                deleteAliasQuietly(previousAlias);
            }
        } finally {
            if (!committed) deleteAliasQuietly(session.alias);
            if (clear != null) Arrays.fill(clear, (byte) 0);
            session.clear();
        }
    }

    public void cancelDomainEnrollment(DomainEnrollmentSession session)
            throws GeneralSecurityException, IOException {
        try {
            deleteAlias(session.alias);
        } finally {
            session.clear();
        }
    }

    /** Returns whether the device-level protection domain is currently usable. */
    public boolean hasDomain() {
        try {
            String alias = activeAlias();
            return alias != null && keyStore().containsAlias(alias);
        } catch (Exception ignored) {
            return false;
        }
    }

    /** Wraps/replaces one signer unlock key without requiring biometric authentication. */
    public void enrollSigner(String signerId, byte[] unlockKey)
            throws GeneralSecurityException, IOException {
        if (unlockKey == null || unlockKey.length != UNLOCK_KEY_BYTES) {
            throw new GeneralSecurityException("WalletUnlockKey must be exactly 32 bytes");
        }
        String alias = requireActiveAlias();
        byte[] ciphertext = encryptWithPublicKey(alias, unlockKey);
        StoredRecord record = new StoredRecord(alias, ciphertext);
        if (!preferences.edit().putString(recordKey(signerId), record.encode()).commit()) {
            Arrays.fill(ciphertext, (byte) 0);
            throw new IOException("Unable to persist wrapped Fresnica WalletUnlockKey");
        }
        Arrays.fill(ciphertext, (byte) 0);
    }

    /** Begins one biometric-gated private-key unwrap for an enrolled signer. */
    public UnlockSession beginUnlock(String signerId)
            throws GeneralSecurityException, IOException {
        String activeAlias = requireActiveAlias();
        StoredRecord record = requireRecord(signerId);
        if (!activeAlias.equals(record.alias)) {
            throw new GeneralSecurityException("Stored WalletUnlockKey belongs to a stale system-auth domain");
        }
        return new UnlockSession(signerId, decryptCipher(activeAlias), record.ciphertext);
    }

    /** Returns the 32-byte key after the authenticated private-key operation succeeds. */
    public byte[] finishUnlock(UnlockSession session) throws GeneralSecurityException {
        byte[] clear = session.cipher.doFinal(session.ciphertext);
        if (clear.length != UNLOCK_KEY_BYTES) {
            Arrays.fill(clear, (byte) 0);
            throw new GeneralSecurityException("Stored WalletUnlockKey has an invalid length");
        }
        return clear;
    }

    public boolean isEnrolled(String signerId) {
        try {
            String alias = activeAlias();
            if (alias == null || !keyStore().containsAlias(alias)) return false;
            StoredRecord record = decodeRecord(preferences.getString(recordKey(signerId), null));
            return record != null && alias.equals(record.alias);
        } catch (Exception ignored) {
            return false;
        }
    }

    public void deleteSigner(String signerId) throws GeneralSecurityException, IOException {
        if (!preferences.edit().remove(recordKey(signerId)).commit()) {
            throw new IOException("Unable to remove wrapped Fresnica WalletUnlockKey");
        }
    }

    /** Deletes the device domain and all per-signer wrapped keys. */
    public void deleteDomain() throws GeneralSecurityException, IOException {
        String alias = activeAlias();
        if (!preferences.edit().clear().commit()) {
            throw new IOException("Unable to remove Fresnica system-auth domain metadata");
        }
        if (alias != null) deleteAlias(alias);
    }

    private void generateUserAuthKeyPair(String alias) throws GeneralSecurityException {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
            try {
                generateKeyPair(alias, true);
                return;
            } catch (ProviderException strongBoxUnavailable) {
                deleteAliasQuietly(alias);
            }
        }
        generateKeyPair(alias, false);
    }

    private void generateKeyPair(String alias, boolean strongBox) throws GeneralSecurityException {
        KeyPairGenerator generator = KeyPairGenerator.getInstance(KeyProperties.KEY_ALGORITHM_RSA, KEYSTORE);
        KeyGenParameterSpec.Builder builder = new KeyGenParameterSpec.Builder(
                alias,
                KeyProperties.PURPOSE_DECRYPT)
                .setKeySize(RSA_KEY_BITS)
                .setDigests(KeyProperties.DIGEST_SHA256, KeyProperties.DIGEST_SHA1)
                .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_RSA_OAEP)
                .setUserAuthenticationRequired(true)
                .setInvalidatedByBiometricEnrollment(true);

        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            builder.setUserAuthenticationParameters(0, KeyProperties.AUTH_BIOMETRIC_STRONG);
        } else {
            builder.setUserAuthenticationValidityDurationSeconds(-1);
        }
        if (strongBox && Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
            builder.setIsStrongBoxBacked(true);
        }
        generator.initialize(builder.build());
        generator.generateKeyPair();
    }

    private byte[] encryptWithPublicKey(String alias, byte[] clear)
            throws GeneralSecurityException, IOException {
        KeyStore store = keyStore();
        java.security.cert.Certificate certificate = store.getCertificate(alias);
        if (certificate == null) {
            throw new GeneralSecurityException("System-auth domain public key is missing");
        }
        byte[] encoded = certificate.getPublicKey().getEncoded();
        PublicKey publicKey = KeyFactory.getInstance("RSA")
                .generatePublic(new X509EncodedKeySpec(encoded));
        Cipher cipher = Cipher.getInstance(TRANSFORMATION);
        cipher.init(Cipher.ENCRYPT_MODE, publicKey, OAEP_PARAMETERS);
        return cipher.doFinal(clear);
    }

    private Cipher decryptCipher(String alias) throws GeneralSecurityException, IOException {
        PrivateKey privateKey = (PrivateKey) keyStore().getKey(alias, null);
        if (privateKey == null) {
            throw new GeneralSecurityException("System-auth domain private key is missing");
        }
        Cipher cipher = Cipher.getInstance(TRANSFORMATION);
        cipher.init(Cipher.DECRYPT_MODE, privateKey, OAEP_PARAMETERS);
        return cipher;
    }

    private String requireActiveAlias() throws GeneralSecurityException, IOException {
        String alias = activeAlias();
        if (alias == null || !keyStore().containsAlias(alias)) {
            throw new GeneralSecurityException("No Fresnica system-auth domain exists");
        }
        return alias;
    }

    private String activeAlias() {
        String alias = preferences.getString(ACTIVE_ALIAS, null);
        return alias == null || alias.trim().isEmpty() ? null : alias;
    }

    private StoredRecord requireRecord(String signerId) throws GeneralSecurityException {
        StoredRecord record = decodeRecord(preferences.getString(recordKey(signerId), null));
        if (record == null) {
            throw new GeneralSecurityException("No wrapped WalletUnlockKey exists for this signer");
        }
        return record;
    }

    private void deleteAliasQuietly(String alias) {
        try {
            deleteAlias(alias);
        } catch (Exception ignored) {
        }
    }

    private void deleteAlias(String alias) throws GeneralSecurityException, IOException {
        KeyStore store = keyStore();
        if (store.containsAlias(alias)) store.deleteEntry(alias);
    }

    private static KeyStore keyStore() throws GeneralSecurityException, IOException {
        KeyStore store = KeyStore.getInstance(KEYSTORE);
        store.load(null);
        return store;
    }

    private static String recordKey(String signerId) throws GeneralSecurityException {
        return SIGNER_PREFIX + signerDigest(signerId);
    }

    private static String signerDigest(String signerId) throws GeneralSecurityException {
        if (signerId == null || signerId.trim().isEmpty()) {
            throw new GeneralSecurityException("signerId must not be empty");
        }
        byte[] hash = MessageDigest.getInstance("SHA-256")
                .digest(signerId.getBytes(StandardCharsets.UTF_8));
        StringBuilder text = new StringBuilder(hash.length * 2);
        for (byte value : hash) text.append(String.format("%02x", value & 0xff));
        Arrays.fill(hash, (byte) 0);
        return text.toString();
    }

    private static StoredRecord decodeRecord(String encoded) {
        if (encoded == null) return null;
        String[] fields = encoded.split("\\|", -1);
        if (fields.length != 2 || fields[0].isEmpty()) return null;
        try {
            return new StoredRecord(fields[0], Base64.decode(fields[1], Base64.NO_WRAP));
        } catch (IllegalArgumentException ignored) {
            return null;
        }
    }

    public static final class DomainEnrollmentSession {
        private final String alias;
        private final Cipher cipher;
        private byte[] ciphertext;
        private byte[] challenge;

        private DomainEnrollmentSession(String alias, Cipher cipher, byte[] ciphertext, byte[] challenge) {
            this.alias = alias;
            this.cipher = cipher;
            this.ciphertext = ciphertext;
            this.challenge = challenge;
        }

        public Cipher getCipher() { return cipher; }

        private void clear() {
            Arrays.fill(ciphertext, (byte) 0);
            Arrays.fill(challenge, (byte) 0);
            ciphertext = new byte[0];
            challenge = new byte[0];
        }
    }

    public static final class UnlockSession {
        private final String signerId;
        private final Cipher cipher;
        private final byte[] ciphertext;

        private UnlockSession(String signerId, Cipher cipher, byte[] ciphertext) {
            this.signerId = signerId;
            this.cipher = cipher;
            this.ciphertext = ciphertext;
        }

        public Cipher getCipher() { return cipher; }
        public String getSignerId() { return signerId; }
    }

    private static final class StoredRecord {
        private final String alias;
        private final byte[] ciphertext;

        private StoredRecord(String alias, byte[] ciphertext) {
            this.alias = alias;
            this.ciphertext = ciphertext;
        }

        private String encode() {
            return alias + "|" + Base64.encodeToString(ciphertext, Base64.NO_WRAP);
        }
    }
}
