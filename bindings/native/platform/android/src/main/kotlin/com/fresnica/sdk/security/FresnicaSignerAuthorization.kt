package com.fresnica.sdk.security

import com.fresnica.sdk.FresnicaSdkApi
import java.util.concurrent.atomic.AtomicBoolean
import javax.crypto.Cipher

/** Native-only authorization for Fresnica protected software signers on Android. */
class FresnicaSignerAuthorization(
    private val keyStore: WalletUnlockKeyStore,
    private val core: FresnicaSdkApi = FresnicaSdkApi(),
) : AutoCloseable {

    fun beginSystemAuthDomainEnrollment(): DomainEnrollmentSession =
        DomainEnrollmentSession(keyStore.beginDomainEnrollment())

    fun finishSystemAuthDomainEnrollment(session: DomainEnrollmentSession) {
        session.consume()
        keyStore.finishDomainEnrollment(session.storeSession)
    }

    fun cancelSystemAuthDomainEnrollment(session: DomainEnrollmentSession) {
        if (session.tryConsume()) keyStore.cancelDomainEnrollment(session.storeSession)
    }

    fun hasSystemAuthDomain(): Boolean = keyStore.hasDomain()

    /** Passcode-authenticated signer registration; no biometric operation occurs here. */
    fun registerSignerSystemAuth(
        envelopeJson: String,
        appPasscode: String,
        expectedSignerPublicKey: String,
    ) {
        val unlockKey = core.deriveUnlockKey(envelopeJson, appPasscode, expectedSignerPublicKey)
        try {
            require(unlockKey.size == WalletUnlockKeyStore.UNLOCK_KEY_BYTES) {
                "SDK returned an invalid WalletUnlockKey length"
            }
            keyStore.enrollSigner(expectedSignerPublicKey, unlockKey)
        } finally {
            unlockKey.fill(0)
        }
    }

    fun isSignerSystemAuthEnrolled(expectedSignerPublicKey: String): Boolean =
        keyStore.isEnrolled(expectedSignerPublicKey)

    fun removeSignerSystemAuth(expectedSignerPublicKey: String) {
        keyStore.deleteSigner(expectedSignerPublicKey)
    }

    fun removeSystemAuthDomain() {
        keyStore.deleteDomain()
    }

    fun beginSystemAuthSign(
        envelopeJson: String,
        expectedSignerPublicKey: String,
        transactionXdr: ByteArray,
        networkPassphrase: String,
    ): SigningSession = SigningSession(
        storeSession = keyStore.beginUnlock(expectedSignerPublicKey),
        envelopeJson = envelopeJson,
        expectedSignerPublicKey = expectedSignerPublicKey,
        transactionXdr = transactionXdr.copyOf(),
        networkPassphrase = networkPassphrase,
    )

    fun finishSystemAuthSign(session: SigningSession): ByteArray {
        session.consume()
        try {
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
            }
        } finally {
            session.clearTransaction()
        }
    }

    fun cancelSystemAuthSign(session: SigningSession) {
        session.consume()
        session.clearTransaction()
    }

    fun signWithPasscode(
        envelopeJson: String,
        appPasscode: String,
        expectedSignerPublicKey: String,
        transactionXdr: ByteArray,
        networkPassphrase: String,
    ): ByteArray {
        val unlockKey = core.deriveUnlockKey(envelopeJson, appPasscode, expectedSignerPublicKey)
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

    override fun close() { core.close() }

    class DomainEnrollmentSession internal constructor(
        internal val storeSession: WalletUnlockKeyStore.DomainEnrollmentSession,
    ) {
        private val consumed = AtomicBoolean(false)
        val cipher: Cipher get() = storeSession.cipher
        internal fun consume() { check(consumed.compareAndSet(false, true)) { "Domain enrollment session has already been consumed" } }
        internal fun tryConsume(): Boolean = consumed.compareAndSet(false, true)
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
        val cipher: Cipher get() = storeSession.cipher
        internal fun consume() { check(consumed.compareAndSet(false, true)) { "Signing session has already been consumed" } }
        internal fun clearTransaction() { transactionXdr.fill(0); transactionXdr = ByteArray(0) }
    }
}
