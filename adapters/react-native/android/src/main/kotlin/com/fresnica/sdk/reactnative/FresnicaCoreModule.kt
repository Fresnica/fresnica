package com.fresnica.sdk.reactnative

import android.security.keystore.KeyPermanentlyInvalidatedException
import android.security.keystore.UserNotAuthenticatedException
import android.util.Base64
import androidx.biometric.BiometricManager
import androidx.biometric.BiometricManager.Authenticators.BIOMETRIC_STRONG
import androidx.biometric.BiometricPrompt
import androidx.core.content.ContextCompat
import androidx.fragment.app.FragmentActivity
import com.facebook.react.bridge.Arguments
import com.facebook.react.bridge.Promise
import com.facebook.react.bridge.ReactApplicationContext
import com.facebook.react.bridge.ReactContextBaseJavaModule
import com.facebook.react.bridge.ReactMethod
import com.facebook.react.bridge.WritableMap
import com.fresnica.sdk.NativeAccountKind
import com.fresnica.sdk.FresnicaSdkApi
import com.fresnica.sdk.NativeSdkException
import com.fresnica.sdk.NativeExportedSigningMaterial
import com.fresnica.sdk.NativeGeneratedMnemonic
import com.fresnica.sdk.NativeProtectedSoftwareSigner
import com.fresnica.sdk.NativeSigningMaterialKind
import com.fresnica.sdk.security.FresnicaSignerAuthorization
import com.fresnica.sdk.security.WalletUnlockKeyStore
import java.security.GeneralSecurityException
import java.util.concurrent.atomic.AtomicBoolean

/**
 * High-level React Native adapter over the Fresnica Native SDK.
 *
 * Routine protected-software signing stays native-only: WalletUnlockKey bytes, biometric Ciphers
 * and one-shot authorization sessions never cross this boundary. Secret-bearing strings cross only
 * for explicit import, one-time mnemonic generation, or explicit Reveal / Export.
 */
class FresnicaCoreModule(
    reactContext: ReactApplicationContext,
) : ReactContextBaseJavaModule(reactContext) {
    private val core = FresnicaSdkApi()
    private val authorization = FresnicaSignerAuthorization(WalletUnlockKeyStore(reactContext), core)
    private val authenticationInProgress = AtomicBoolean(false)

    override fun getName(): String = NAME

    // Wallet / signer lifecycle ---------------------------------------------------------------

    @ReactMethod
    fun parseAccount(address: String, promise: Promise) {
        if (!requireNonBlank(address, "address", promise)) return
        runCatching { core.parseAccount(address) }
            .onSuccess { identity ->
                promise.resolve(
                    Arguments.createMap().apply {
                        putString(
                            "kind",
                            when (identity.kind) {
                                NativeAccountKind.CLASSIC -> "classic"
                                NativeAccountKind.CONTRACT -> "contract"
                            },
                        )
                        putString("address", identity.address)
                        identity.publicKey?.let { putString("publicKey", it) } ?: putNull("publicKey")
                    },
                )
            }
            .onFailure { reject(promise, it) }
    }

    @ReactMethod
    fun protectSecret(
        secret: String,
        appPasscode: String,
        expectedSignerPublicKey: String?,
        promise: Promise,
    ) {
        runCatching { core.protectSecret(secret, appPasscode, expectedSignerPublicKey) }
            .onSuccess { promise.resolve(protectedSignerMap(it)) }
            .onFailure { reject(promise, it) }
    }

    @ReactMethod
    fun protectMnemonic(
        mnemonic: String,
        mnemonicPassphrase: String,
        index: Double,
        language: String?,
        appPasscode: String,
        expectedSignerPublicKey: String?,
        promise: Promise,
    ) {
        val parsedIndex = uint32(index, "index", promise) ?: return
        runCatching {
            core.protectMnemonic(
                mnemonic,
                mnemonicPassphrase,
                parsedIndex,
                language,
                appPasscode,
                expectedSignerPublicKey,
            )
        }.onSuccess { promise.resolve(protectedSignerMap(it)) }
            .onFailure { reject(promise, it) }
    }

    @ReactMethod
    fun generateMnemonic(
        language: String,
        strength: Double,
        mnemonicPassphrase: String,
        index: Double,
        appPasscode: String,
        promise: Promise,
    ) {
        if (!requireNonBlank(language, "language", promise)) return
        val parsedStrength = uint32(strength, "strength", promise) ?: return
        val parsedIndex = uint32(index, "index", promise) ?: return
        runCatching {
            core.generateMnemonic(
                language,
                parsedStrength,
                mnemonicPassphrase,
                parsedIndex,
                appPasscode,
            )
        }.onSuccess { promise.resolve(generatedMnemonicMap(it)) }
            .onFailure { reject(promise, it) }
    }

    @ReactMethod
    fun deriveMnemonicSigner(
        sourceEnvelopeJson: String,
        appPasscode: String,
        expectedSourceSignerPublicKey: String,
        index: Double,
        promise: Promise,
    ) {
        val parsedIndex = uint32(index, "index", promise) ?: return
        runCatching {
            core.deriveMnemonicSigner(
                sourceEnvelopeJson,
                appPasscode,
                expectedSourceSignerPublicKey,
                parsedIndex,
            )
        }.onSuccess { promise.resolve(protectedSignerMap(it)) }
            .onFailure { reject(promise, it) }
    }

    @ReactMethod
    fun reprotect(
        envelopeJson: String,
        currentPasscode: String,
        newPasscode: String,
        expectedSignerPublicKey: String,
        promise: Promise,
    ) {
        runCatching {
            core.reprotect(
                envelopeJson,
                currentPasscode,
                newPasscode,
                expectedSignerPublicKey,
            )
        }.onSuccess { promise.resolve(protectedSignerMap(it)) }
            .onFailure { reject(promise, it) }
    }

    @ReactMethod
    fun reveal(
        envelopeJson: String,
        freshAppPasscode: String,
        expectedSignerPublicKey: String,
        promise: Promise,
    ) {
        runCatching { core.reveal(envelopeJson, freshAppPasscode, expectedSignerPublicKey) }
            .onSuccess { promise.resolve(exportedMaterialMap(it)) }
            .onFailure { reject(promise, it) }
    }

    // External Ed25519 signer boundary --------------------------------------------------------

    @ReactMethod
    fun prepareEd25519Signing(
        transactionXdrBase64: String,
        networkPassphrase: String,
        promise: Promise,
    ) {
        if (!requireNonBlank(networkPassphrase, "networkPassphrase", promise)) return
        val transactionXdr = decodeBase64(transactionXdrBase64, "transactionXdrBase64", promise) ?: return
        try {
            val request = core.prepareEd25519Signing(transactionXdr, networkPassphrase)
            promise.resolve(
                Arguments.createMap().apply {
                    putString("transactionHashBase64", Base64.encodeToString(request.transactionHash, Base64.NO_WRAP))
                    putString("transactionXdrBase64", Base64.encodeToString(request.transactionXdr, Base64.NO_WRAP))
                    putString("networkPassphrase", request.networkPassphrase)
                },
            )
        } catch (error: Throwable) {
            reject(promise, error)
        } finally {
            transactionXdr.fill(0)
        }
    }

    @ReactMethod
    fun applyEd25519Signature(
        transactionXdrBase64: String,
        networkPassphrase: String,
        signerPublicKey: String,
        signatureBase64: String,
        promise: Promise,
    ) {
        if (!requireNonBlank(networkPassphrase, "networkPassphrase", promise)) return
        if (!requireNonBlank(signerPublicKey, "signerPublicKey", promise)) return
        val transactionXdr = decodeBase64(transactionXdrBase64, "transactionXdrBase64", promise) ?: return
        val signature = decodeBase64(signatureBase64, "signatureBase64", promise, ED25519_SIGNATURE_BYTES)
        if (signature == null) {
            transactionXdr.fill(0)
            return
        }
        try {
            val signed = core.applyEd25519Signature(
                transactionXdr,
                networkPassphrase,
                signerPublicKey,
                signature,
            )
            try {
                promise.resolve(Base64.encodeToString(signed, Base64.NO_WRAP))
            } finally {
                signed.fill(0)
            }
        } catch (error: Throwable) {
            reject(promise, error)
        } finally {
            transactionXdr.fill(0)
            signature.fill(0)
        }
    }

    // Native-only protected software signing -------------------------------------------------

    @ReactMethod
    fun canUseSystemAuth(promise: Promise) {
        val available =
            currentFragmentActivity() != null &&
                BiometricManager.from(reactApplicationContext)
                    .canAuthenticate(BIOMETRIC_STRONG) == BiometricManager.BIOMETRIC_SUCCESS
        promise.resolve(available)
    }

    @ReactMethod
    fun hasSystemAuthDomain(promise: Promise) {
        runCatching { authorization.hasSystemAuthDomain() }
            .onSuccess(promise::resolve)
            .onFailure { reject(promise, it) }
    }

    @ReactMethod
    fun initializeSystemAuth(reason: String, promise: Promise) {
        val activity = currentFragmentActivity()
        if (activity == null) {
            promise.reject(ERROR_SYSTEM_AUTH_UNAVAILABLE, "A FragmentActivity is required for system authentication")
            return
        }
        if (!requireNonBlank(reason, "reason", promise)) return
        if (!authenticationInProgress.compareAndSet(false, true)) {
            promise.reject(ERROR_AUTH_IN_PROGRESS, "Another Fresnica biometric operation is already active")
            return
        }

        val session = try {
            authorization.beginSystemAuthDomainEnrollment()
        } catch (error: Throwable) {
            authenticationInProgress.set(false)
            reject(promise, error)
            return
        }

        activity.runOnUiThread {
            val prompt = BiometricPrompt(
                activity,
                ContextCompat.getMainExecutor(activity),
                object : BiometricPrompt.AuthenticationCallback() {
                    override fun onAuthenticationSucceeded(result: BiometricPrompt.AuthenticationResult) {
                        try {
                            if (result.cryptoObject?.cipher !== session.cipher) {
                                runCatching { authorization.cancelSystemAuthDomainEnrollment(session) }
                                promise.reject(
                                    ERROR_SYSTEM_AUTH_FAILED,
                                    "BiometricPrompt did not authorize the Fresnica system-auth domain Cipher",
                                )
                                return
                            }
                            authorization.finishSystemAuthDomainEnrollment(session)
                            promise.resolve(true)
                        } catch (error: Throwable) {
                            reject(promise, error)
                        } finally {
                            authenticationInProgress.set(false)
                        }
                    }

                    override fun onAuthenticationError(errorCode: Int, errString: CharSequence) {
                        try {
                            runCatching { authorization.cancelSystemAuthDomainEnrollment(session) }
                            promise.reject(biometricErrorCode(errorCode), errString.toString())
                        } finally {
                            authenticationInProgress.set(false)
                        }
                    }
                },
            )
            prompt.authenticate(promptInfo(reason), BiometricPrompt.CryptoObject(session.cipher))
        }
    }

    @ReactMethod
    fun registerSignerSystemAuth(
        envelopeJson: String,
        appPasscode: String,
        expectedSignerPublicKey: String,
        promise: Promise,
    ) {
        runCatching {
            authorization.registerSignerSystemAuth(
                envelopeJson,
                appPasscode,
                expectedSignerPublicKey,
            )
        }.onSuccess { promise.resolve(true) }.onFailure { reject(promise, it) }
    }

    @ReactMethod
    fun hasSignerSystemAuth(expectedSignerPublicKey: String, promise: Promise) {
        runCatching { authorization.isSignerSystemAuthEnrolled(expectedSignerPublicKey) }
            .onSuccess(promise::resolve)
            .onFailure { reject(promise, it) }
    }

    @ReactMethod
    fun removeSignerSystemAuth(expectedSignerPublicKey: String, promise: Promise) {
        runCatching { authorization.removeSignerSystemAuth(expectedSignerPublicKey) }
            .onSuccess { promise.resolve(true) }
            .onFailure { reject(promise, it) }
    }

    @ReactMethod
    fun removeSystemAuthDomain(promise: Promise) {
        runCatching { authorization.removeSystemAuthDomain() }
            .onSuccess { promise.resolve(true) }
            .onFailure { reject(promise, it) }
    }

    @ReactMethod
    fun signMessageWithSystemAuth(
        envelopeJson: String,
        expectedSignerPublicKey: String,
        message: String,
        reason: String,
        promise: Promise,
    ) {
        val activity = currentFragmentActivity()
        if (activity == null) {
            promise.reject(ERROR_SYSTEM_AUTH_UNAVAILABLE, "A FragmentActivity is required for biometric signing")
            return
        }
        if (!requireNonBlank(reason, "reason", promise)) return
        val messageBytes = message.toByteArray(Charsets.UTF_8)
        if (!authenticationInProgress.compareAndSet(false, true)) {
            messageBytes.fill(0)
            promise.reject(ERROR_AUTH_IN_PROGRESS, "Another Fresnica biometric operation is already active")
            return
        }

        val session = try {
            authorization.beginSystemAuthMessageSign(
                envelopeJson,
                expectedSignerPublicKey,
                messageBytes,
            )
        } catch (error: Throwable) {
            authenticationInProgress.set(false)
            reject(promise, error)
            return
        } finally {
            messageBytes.fill(0)
        }

        activity.runOnUiThread {
            val prompt = BiometricPrompt(
                activity,
                ContextCompat.getMainExecutor(activity),
                object : BiometricPrompt.AuthenticationCallback() {
                    override fun onAuthenticationSucceeded(result: BiometricPrompt.AuthenticationResult) {
                        try {
                            if (result.cryptoObject?.cipher !== session.cipher) {
                                runCatching { authorization.cancelSystemAuthMessageSign(session) }
                                promise.reject(
                                    ERROR_SYSTEM_AUTH_FAILED,
                                    "BiometricPrompt did not authorize the Fresnica message-signing Cipher",
                                )
                                return
                            }
                            val signature = authorization.finishSystemAuthMessageSign(session)
                            try {
                                promise.resolve(Base64.encodeToString(signature, Base64.NO_WRAP))
                            } finally {
                                signature.fill(0)
                            }
                        } catch (error: Throwable) {
                            reject(promise, error)
                        } finally {
                            authenticationInProgress.set(false)
                        }
                    }

                    override fun onAuthenticationError(errorCode: Int, errString: CharSequence) {
                        try {
                            runCatching { authorization.cancelSystemAuthMessageSign(session) }
                            promise.reject(
                                biometricErrorCode(errorCode),
                                errString.toString(),
                            )
                        } finally {
                            authenticationInProgress.set(false)
                        }
                    }
                },
            )
            prompt.authenticate(
                promptInfo(reason),
                BiometricPrompt.CryptoObject(session.cipher),
            )
        }
    }

    @ReactMethod
    fun signMessageWithPasscode(
        envelopeJson: String,
        appPasscode: String,
        expectedSignerPublicKey: String,
        message: String,
        promise: Promise,
    ) {
        val messageBytes = message.toByteArray(Charsets.UTF_8)
        try {
            val signature = authorization.signMessageWithPasscode(
                envelopeJson,
                appPasscode,
                expectedSignerPublicKey,
                messageBytes,
            )
            try {
                promise.resolve(Base64.encodeToString(signature, Base64.NO_WRAP))
            } finally {
                signature.fill(0)
            }
        } catch (error: Throwable) {
            reject(promise, error)
        } finally {
            messageBytes.fill(0)
        }
    }

    @ReactMethod
    fun signWithSystemAuth(
        envelopeJson: String,
        expectedSignerPublicKey: String,
        transactionXdrBase64: String,
        networkPassphrase: String,
        reason: String,
        promise: Promise,
    ) {
        val activity = currentFragmentActivity()
        if (activity == null) {
            promise.reject(ERROR_SYSTEM_AUTH_UNAVAILABLE, "A FragmentActivity is required for biometric signing")
            return
        }
        if (!requireNonBlank(networkPassphrase, "networkPassphrase", promise)) return
        if (!requireNonBlank(reason, "reason", promise)) return
        val transactionXdr = decodeBase64(transactionXdrBase64, "transactionXdrBase64", promise) ?: return
        if (!authenticationInProgress.compareAndSet(false, true)) {
            transactionXdr.fill(0)
            promise.reject(ERROR_AUTH_IN_PROGRESS, "Another Fresnica biometric operation is already active")
            return
        }

        val session = try {
            authorization.beginSystemAuthSign(
                envelopeJson,
                expectedSignerPublicKey,
                transactionXdr,
                networkPassphrase,
            )
        } catch (error: Throwable) {
            authenticationInProgress.set(false)
            reject(promise, error)
            return
        } finally {
            transactionXdr.fill(0)
        }

        activity.runOnUiThread {
            val prompt = BiometricPrompt(
                activity,
                ContextCompat.getMainExecutor(activity),
                object : BiometricPrompt.AuthenticationCallback() {
                    override fun onAuthenticationSucceeded(result: BiometricPrompt.AuthenticationResult) {
                        try {
                            if (result.cryptoObject?.cipher !== session.cipher) {
                                runCatching { authorization.cancelSystemAuthSign(session) }
                                promise.reject(
                                    ERROR_SYSTEM_AUTH_FAILED,
                                    "BiometricPrompt did not authorize the Fresnica signing Cipher",
                                )
                                return
                            }
                            val signed = authorization.finishSystemAuthSign(session)
                            try {
                                promise.resolve(Base64.encodeToString(signed, Base64.NO_WRAP))
                            } finally {
                                signed.fill(0)
                            }
                        } catch (error: Throwable) {
                            reject(promise, error)
                        } finally {
                            authenticationInProgress.set(false)
                        }
                    }

                    override fun onAuthenticationError(errorCode: Int, errString: CharSequence) {
                        try {
                            runCatching { authorization.cancelSystemAuthSign(session) }
                            promise.reject(
                                biometricErrorCode(errorCode),
                                errString.toString(),
                            )
                        } finally {
                            authenticationInProgress.set(false)
                        }
                    }
                },
            )
            prompt.authenticate(
                promptInfo(reason),
                BiometricPrompt.CryptoObject(session.cipher),
            )
        }
    }

    @ReactMethod
    fun signWithPasscode(
        envelopeJson: String,
        appPasscode: String,
        expectedSignerPublicKey: String,
        transactionXdrBase64: String,
        networkPassphrase: String,
        promise: Promise,
    ) {
        if (!requireNonBlank(networkPassphrase, "networkPassphrase", promise)) return
        val transactionXdr = decodeBase64(transactionXdrBase64, "transactionXdrBase64", promise) ?: return
        try {
            val signed = authorization.signWithPasscode(
                envelopeJson,
                appPasscode,
                expectedSignerPublicKey,
                transactionXdr,
                networkPassphrase,
            )
            try {
                promise.resolve(Base64.encodeToString(signed, Base64.NO_WRAP))
            } finally {
                signed.fill(0)
            }
        } catch (error: Throwable) {
            reject(promise, error)
        } finally {
            transactionXdr.fill(0)
        }
    }

    private fun protectedSignerMap(value: NativeProtectedSoftwareSigner): WritableMap =
        Arguments.createMap().apply {
            putString("signerPublicKey", value.signerPublicKey)
            putString("envelopeJson", value.envelopeJson)
        }

    private fun generatedMnemonicMap(value: NativeGeneratedMnemonic): WritableMap =
        Arguments.createMap().apply {
            putMap("signer", protectedSignerMap(value.signer))
            putString("mnemonic", value.mnemonic)
            putString("language", value.language)
            putDouble("index", value.index.toDouble())
        }

    private fun exportedMaterialMap(value: NativeExportedSigningMaterial): WritableMap =
        Arguments.createMap().apply {
            putString(
                "kind",
                when (value.kind) {
                    NativeSigningMaterialKind.SECRET -> "secret"
                    NativeSigningMaterialKind.MNEMONIC -> "mnemonic"
                },
            )
            value.secret?.let { putString("secret", it) } ?: putNull("secret")
            value.mnemonic?.let { putString("mnemonic", it) } ?: putNull("mnemonic")
            value.mnemonicPassphrase?.let { putString("mnemonicPassphrase", it) } ?: putNull("mnemonicPassphrase")
            value.index?.let { putDouble("index", it.toDouble()) } ?: putNull("index")
            value.language?.let { putString("language", it) } ?: putNull("language")
        }

    private fun currentFragmentActivity(): FragmentActivity? =
        reactApplicationContext.currentActivity as? FragmentActivity

    private fun promptInfo(reason: String): BiometricPrompt.PromptInfo =
        BiometricPrompt.PromptInfo.Builder()
            .setTitle(reason.trim())
            .setAllowedAuthenticators(BIOMETRIC_STRONG)
            .setNegativeButtonText("Cancel")
            .build()

    private fun requireNonBlank(value: String, field: String, promise: Promise): Boolean {
        if (value.isBlank()) {
            promise.reject(ERROR_INVALID_INPUT, "$field must not be blank")
            return false
        }
        return true
    }

    private fun uint32(value: Double, field: String, promise: Promise): UInt? {
        if (!value.isFinite() || value < 0.0 || value > UInt.MAX_VALUE.toDouble() || value % 1.0 != 0.0) {
            promise.reject(ERROR_INVALID_INPUT, "$field must be an unsigned 32-bit integer")
            return null
        }
        return value.toLong().toUInt()
    }

    private fun decodeBase64(
        text: String,
        field: String,
        promise: Promise,
        exactLength: Int? = null,
    ): ByteArray? {
        if (text.isEmpty()) {
            promise.reject(ERROR_INVALID_INPUT, "$field must not be empty")
            return null
        }
        val decoded = try {
            java.util.Base64.getDecoder().decode(text)
        } catch (error: IllegalArgumentException) {
            promise.reject(ERROR_INVALID_INPUT, "$field is not valid base64", error)
            return null
        }
        if (decoded.isEmpty()) {
            promise.reject(ERROR_INVALID_INPUT, "$field must decode to non-empty bytes")
            return null
        }
        if (exactLength != null && decoded.size != exactLength) {
            decoded.fill(0)
            promise.reject(ERROR_INVALID_INPUT, "$field must decode to exactly $exactLength bytes")
            return null
        }
        return decoded
    }

    private fun reject(promise: Promise, error: Throwable) {
        when (error) {
            is NativeSdkException.InvalidInput -> promise.reject(ERROR_INVALID_INPUT, error.detail, error)
            is NativeSdkException.InvalidPasscode -> promise.reject(ERROR_INVALID_PASSCODE, error.detail, error)
            is NativeSdkException.InvalidUnlockKey -> promise.reject(ERROR_INVALID_UNLOCK_KEY, error.detail, error)
            is NativeSdkException.InvalidProtectedData -> promise.reject(ERROR_INVALID_PROTECTED_DATA, error.detail, error)
            is NativeSdkException.IdentityMismatch -> promise.reject(ERROR_IDENTITY_MISMATCH, error.detail, error)
            is NativeSdkException.InvalidTransaction -> promise.reject(ERROR_INVALID_TRANSACTION, error.detail, error)
            is NativeSdkException.InvalidMessageSignature -> promise.reject(ERROR_INVALID_MESSAGE_SIGNATURE, error.detail, error)
            is NativeSdkException.CoreException -> promise.reject(ERROR_CORE, error.detail, error)
            is KeyPermanentlyInvalidatedException -> promise.reject(
                ERROR_SYSTEM_AUTH_INVALIDATED,
                error.message ?: "Biometric enrollment changed",
                error,
            )
            is UserNotAuthenticatedException -> promise.reject(
                ERROR_SYSTEM_AUTH_FAILED,
                error.message ?: "Biometric authentication is required",
                error,
            )
            is GeneralSecurityException -> {
                val message = error.message ?: "Android system authentication failed"
                val code = if (
                    message.contains("system-auth domain", ignoreCase = true) ||
                    message.contains("wrapped WalletUnlockKey", ignoreCase = true)
                ) {
                    ERROR_SYSTEM_AUTH_NOT_ENROLLED
                } else {
                    ERROR_SYSTEM_AUTH_FAILED
                }
                promise.reject(code, message, error)
            }
            else -> promise.reject(ERROR_NATIVE, error.message ?: "Fresnica native operation failed", error)
        }
    }

    private fun biometricErrorCode(errorCode: Int): String = when (errorCode) {
        BiometricPrompt.ERROR_NEGATIVE_BUTTON,
        BiometricPrompt.ERROR_USER_CANCELED,
        BiometricPrompt.ERROR_CANCELED -> ERROR_USER_CANCEL
        BiometricPrompt.ERROR_HW_NOT_PRESENT,
        BiometricPrompt.ERROR_HW_UNAVAILABLE,
        BiometricPrompt.ERROR_NO_BIOMETRICS -> ERROR_SYSTEM_AUTH_UNAVAILABLE
        else -> ERROR_SYSTEM_AUTH_FAILED
    }

    companion object {
        const val NAME = "FresnicaCore"
        private const val ED25519_SIGNATURE_BYTES = 64

        private const val ERROR_INVALID_INPUT = "invalid-input"
        private const val ERROR_INVALID_PASSCODE = "invalid-passcode"
        private const val ERROR_INVALID_UNLOCK_KEY = "invalid-unlock-key"
        private const val ERROR_INVALID_PROTECTED_DATA = "invalid-protected-data"
        private const val ERROR_IDENTITY_MISMATCH = "identity-mismatch"
        private const val ERROR_INVALID_TRANSACTION = "invalid-transaction"
        private const val ERROR_INVALID_MESSAGE_SIGNATURE = "invalid-message-signature"
        private const val ERROR_CORE = "core-error"
        private const val ERROR_AUTH_IN_PROGRESS = "auth-in-progress"
        private const val ERROR_USER_CANCEL = "user-cancel"
        private const val ERROR_SYSTEM_AUTH_UNAVAILABLE = "system-auth-unavailable"
        private const val ERROR_SYSTEM_AUTH_NOT_ENROLLED = "system-auth-not-enrolled"
        private const val ERROR_SYSTEM_AUTH_INVALIDATED = "system-auth-invalidated"
        private const val ERROR_SYSTEM_AUTH_FAILED = "system-auth-failed"
        private const val ERROR_NATIVE = "native-error"
    }
}
