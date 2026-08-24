package com.fresnica.core.security

import com.fresnica.core.MobileCoreApi
import java.util.concurrent.atomic.AtomicBoolean
import javax.crypto.Cipher

/**
 * Native-only orchestration for Fresnica protected-software signing on Android.
 *
 * React Native should call this service through a thin native module. It may pass reviewed XDR,
 * encrypted signer envelopes, signer public keys, and an app passcode when explicitly requested,
 * but it never receives WalletUnlockKey bytes.
 */
class FresnicaSignerAuthorization(
    private val keyStore: WalletUnlockKeyStore,
    private val core: MobileCoreApi = MobileCoreApi(),
) : AutoCloseable {

    /**
     * Derives a verified WalletUnlockKey inside native code and prepares the exact Cipher that
     * BiometricPrompt must authenticate before the key can be enrolled.
     */
    fun beginSystemAuthEnrollment(
        envelopeJson: String,
        appPasscode: String,
        expectedSignerPublicKey: String,
    ): EnrollmentSession {
        val unlockKey = core.deriveUnlockKey(
            envelopeJson,
            appPasscode,
            expectedSignerPublicKey,
        )
        require(unlockKey.size == WalletUnlockKeyStore.UNLOCK_KEY_BYTES) {
            "Core returned an invalid WalletUnlockKey length"
        }

        return try {
            EnrollmentSession(
                keyStore.beginEnrollment(expectedSignerPublicKey),
                unlockKey,
            )
        } catch (error: Throwable) {
            unlockKey.fill(0)
            throw error
        }
    }

    /** Completes enrollment only after BiometricPrompt authenticated session.cipher. */
    fun finishSystemAuthEnrollment(session: EnrollmentSession) {
        val unlockKey = session.takeUnlockKey()
        try {
            keyStore.finishEnrollment(session.storeSession, unlockKey)
        } catch (error: Throwable) {
            runCatching { keyStore.cancelEnrollment(session.storeSession) }
            throw error
        } finally {
            unlockKey.fill(0)
        }
    }

    /** Cancels a pending enrollment and removes its temporary AndroidKeyStore alias. */
    fun cancelSystemAuthEnrollment(session: EnrollmentSession) {
        session.clearUnlockKey()
        keyStore.cancelEnrollment(session.storeSession)
    }

    /**
     * Freezes the reviewed signing inputs and returns the exact decrypt Cipher to authenticate.
     * The transaction bytes are copied so JavaScript cannot mutate the pending reviewed payload.
     */
    fun beginSystemAuthSign(
        envelopeJson: String,
        expectedSignerPublicKey: String,
        transactionXdr: ByteArray,
        networkPassphrase: String,
    ): SigningSession {
        val storeSession = keyStore.beginUnlock(expectedSignerPublicKey)
        return SigningSession(
            storeSession = storeSession,
            envelopeJson = envelopeJson,
            expectedSignerPublicKey = expectedSignerPublicKey,
            transactionXdr = transactionXdr.copyOf(),
            networkPassphrase = networkPassphrase,
        )
    }

    /**
     * Decrypts the native-only WalletUnlockKey after biometric success, signs through Rust Core,
     * and zeros the temporary key bytes before returning signed XDR.
     */
    fun finishSystemAuthSign(session: SigningSession): ByteArray {
        session.consume()
        val unlockKey = keyStore.finishUnlock(session.storeSession)
        try {
            require(unlockKey.size == WalletUnlockKeyStore.UNLOCK_KEY_BYTES) {
                "Stored WalletUnlockKey has an invalid length"
            }
            return core.signTransactionXdr(
                session.envelopeJson,
                unlockKey,
                session.expectedSignerPublicKey,
                session.transactionXdr,
                session.networkPassphrase,
            )
        } finally {
            unlockKey.fill(0)
            session.clearTransaction()
        }
    }

    /**
     * App-passcode recovery/fallback path. The derived unlock key never leaves this native method.
     */
    fun signWithPasscode(
        envelopeJson: String,
        appPasscode: String,
        expectedSignerPublicKey: String,
        transactionXdr: ByteArray,
        networkPassphrase: String,
    ): ByteArray {
        val unlockKey = core.deriveUnlockKey(
            envelopeJson,
            appPasscode,
            expectedSignerPublicKey,
        )
        try {
            return core.signTransactionXdr(
                envelopeJson,
                unlockKey,
                expectedSignerPublicKey,
                transactionXdr,
                networkPassphrase,
            )
        } finally {
            unlockKey.fill(0)
        }
    }

    fun isSystemAuthEnrolled(expectedSignerPublicKey: String): Boolean =
        keyStore.isEnrolled(expectedSignerPublicKey)

    fun removeSystemAuth(expectedSignerPublicKey: String) {
        keyStore.delete(expectedSignerPublicKey)
    }

    override fun close() {
        core.close()
    }

    class EnrollmentSession internal constructor(
        internal val storeSession: WalletUnlockKeyStore.EnrollmentSession,
        unlockKey: ByteArray,
    ) {
        private var pendingUnlockKey: ByteArray? = unlockKey

        val cipher: Cipher
            get() = storeSession.cipher

        @Synchronized
        internal fun takeUnlockKey(): ByteArray {
            val key = pendingUnlockKey
                ?: throw IllegalStateException("Enrollment session has already been completed")
            pendingUnlockKey = null
            return key
        }

        @Synchronized
        internal fun clearUnlockKey() {
            pendingUnlockKey?.fill(0)
            pendingUnlockKey = null
        }
    }

    class SigningSession internal constructor(
        internal val storeSession: WalletUnlockKeyStore.UnlockSession,
        internal val envelopeJson: String,
        internal val expectedSignerPublicKey: String,
        transactionXdr: ByteArray,
        internal val networkPassphrase: String,
    ) {
        private val consumed = AtomicBoolean(false)
        internal var transactionXdr: ByteArray = transactionXdr
            private set

        val cipher: Cipher
            get() = storeSession.cipher

        internal fun consume() {
            check(consumed.compareAndSet(false, true)) {
                "Signing session has already been consumed"
            }
        }

        internal fun clearTransaction() {
            transactionXdr.fill(0)
            transactionXdr = ByteArray(0)
        }
    }
}
