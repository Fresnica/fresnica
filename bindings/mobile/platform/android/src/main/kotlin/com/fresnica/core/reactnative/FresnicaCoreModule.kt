package com.fresnica.core.reactnative

import android.security.keystore.KeyPermanentlyInvalidatedException
import android.security.keystore.UserNotAuthenticatedException
import android.util.Base64
import androidx.biometric.BiometricManager.Authenticators.BIOMETRIC_STRONG
import androidx.biometric.BiometricPrompt
import androidx.core.content.ContextCompat
import androidx.fragment.app.FragmentActivity
import com.facebook.react.bridge.Promise
import com.facebook.react.bridge.ReactApplicationContext
import com.facebook.react.bridge.ReactContextBaseJavaModule
import com.facebook.react.bridge.ReactMethod
import com.fresnica.core.MobileCoreException
import com.fresnica.core.security.FresnicaSignerAuthorization
import com.fresnica.core.security.WalletUnlockKeyStore
import java.security.GeneralSecurityException
import java.util.concurrent.atomic.AtomicBoolean

/**
 * Thin React Native surface for Fresnica software-signer authorization.
 *
 * JavaScript may pass public/reviewed transaction material, opaque Core envelopes, signer IDs,
 * and an app passcode for explicit enrollment/fallback flows. WalletUnlockKey bytes, Ciphers and
 * one-shot authorization sessions remain native-only.
 */
class FresnicaCoreModule(
    reactContext: ReactApplicationContext,
) : ReactContextBaseJavaModule(reactContext) {
    private val authorization = FresnicaSignerAuthorization(WalletUnlockKeyStore(reactContext))
    private val authenticationInProgress = AtomicBoolean(false)

    override fun getName(): String = NAME

    @ReactMethod
    fun canEnrollSystemAuth(promise: Promise) {
        val activity = currentFragmentActivity()
        promise.resolve(activity != null)
    }

    @ReactMethod
    fun hasSystemAuth(expectedSignerPublicKey: String, promise: Promise) {
        runCatching {
            authorization.isSystemAuthEnrolled(expectedSignerPublicKey)
        }.onSuccess(promise::resolve).onFailure { reject(promise, it) }
    }

    @ReactMethod
    fun removeSystemAuth(expectedSignerPublicKey: String, promise: Promise) {
        runCatching {
            authorization.removeSystemAuth(expectedSignerPublicKey)
        }.onSuccess { promise.resolve(true) }.onFailure { reject(promise, it) }
    }

    @ReactMethod
    fun enrollSystemAuth(
        envelopeJson: String,
        appPasscode: String,
        expectedSignerPublicKey: String,
        promise: Promise,
    ) {
        val activity = currentFragmentActivity()
        if (activity == null) {
            promise.reject(ERROR_SYSTEM_AUTH_UNAVAILABLE, "A FragmentActivity is required for biometric enrollment")
            return
        }
        if (!authenticationInProgress.compareAndSet(false, true)) {
            promise.reject(ERROR_AUTH_IN_PROGRESS, "Another Fresnica biometric operation is already active")
            return
        }

        val session = try {
            authorization.beginSystemAuthEnrollment(
                envelopeJson,
                appPasscode,
                expectedSignerPublicKey,
            )
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
                                runCatching { authorization.cancelSystemAuthEnrollment(session) }
                                promise.reject(
                                    ERROR_SYSTEM_AUTH_FAILED,
                                    "BiometricPrompt did not authorize the Fresnica enrollment Cipher",
                                )
                                return
                            }
                            authorization.finishSystemAuthEnrollment(session)
                            promise.resolve(true)
                        } catch (error: Throwable) {
                            reject(promise, error)
                        } finally {
                            authenticationInProgress.set(false)
                        }
                    }

                    override fun onAuthenticationError(errorCode: Int, errString: CharSequence) {
                        try {
                            runCatching { authorization.cancelSystemAuthEnrollment(session) }
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
                promptInfo("Enable biometric signing"),
                BiometricPrompt.CryptoObject(session.cipher),
            )
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
        val transactionXdr = decodeTransaction(transactionXdrBase64, promise) ?: return
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
            transactionXdr.fill(0)
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
        val transactionXdr = decodeTransaction(transactionXdrBase64, promise) ?: return
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

    private fun currentFragmentActivity(): FragmentActivity? =
        reactApplicationContext.currentActivity as? FragmentActivity

    private fun promptInfo(reason: String): BiometricPrompt.PromptInfo =
        BiometricPrompt.PromptInfo.Builder()
            .setTitle(reason.trim().ifEmpty { "Authorize Fresnica" })
            .setAllowedAuthenticators(BIOMETRIC_STRONG)
            .setNegativeButtonText("Cancel")
            .build()

    private fun decodeTransaction(text: String, promise: Promise): ByteArray? =
        try {
            Base64.decode(text, Base64.DEFAULT)
        } catch (error: IllegalArgumentException) {
            promise.reject(ERROR_INVALID_INPUT, "transactionXdrBase64 is not valid base64", error)
            null
        }

    private fun reject(promise: Promise, error: Throwable) {
        when (error) {
            is MobileCoreException.InvalidInput -> promise.reject(ERROR_INVALID_INPUT, error.detail, error)
            is MobileCoreException.InvalidPasscode -> promise.reject(ERROR_INVALID_PASSCODE, error.detail, error)
            is MobileCoreException.InvalidUnlockKey -> promise.reject(ERROR_INVALID_UNLOCK_KEY, error.detail, error)
            is MobileCoreException.InvalidProtectedData -> promise.reject(ERROR_INVALID_PROTECTED_DATA, error.detail, error)
            is MobileCoreException.IdentityMismatch -> promise.reject(ERROR_IDENTITY_MISMATCH, error.detail, error)
            is MobileCoreException.InvalidTransaction -> promise.reject(ERROR_INVALID_TRANSACTION, error.detail, error)
            is MobileCoreException.CoreException -> promise.reject(ERROR_CORE, error.detail, error)
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
                val code = if (message.contains("No system-auth", ignoreCase = true)) {
                    ERROR_SYSTEM_AUTH_NOT_ENROLLED
                } else {
                    ERROR_SYSTEM_AUTH_FAILED
                }
                promise.reject(code, message, error)
            }
            else -> promise.reject(ERROR_NATIVE, error.message ?: "Fresnica native signing failed", error)
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

        private const val ERROR_INVALID_INPUT = "invalid-input"
        private const val ERROR_INVALID_PASSCODE = "invalid-passcode"
        private const val ERROR_INVALID_UNLOCK_KEY = "invalid-unlock-key"
        private const val ERROR_INVALID_PROTECTED_DATA = "invalid-protected-data"
        private const val ERROR_IDENTITY_MISMATCH = "identity-mismatch"
        private const val ERROR_INVALID_TRANSACTION = "invalid-transaction"
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
