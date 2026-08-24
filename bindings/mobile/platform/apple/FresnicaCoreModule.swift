import Foundation
import Security

public typealias FresnicaPromiseResolveBlock = @convention(block) (Any?) -> Void
public typealias FresnicaPromiseRejectBlock = @convention(block) (String?, String?, NSError?) -> Void

/// React Native-facing Apple module. It deliberately exposes only high-level signing actions.
/// WalletUnlockKey bytes and Keychain data never cross this boundary.
@objc(FresnicaCoreModule)
public final class FresnicaCoreModule: NSObject {
    private let authorization: FresnicaSignerAuthorization

    public override init() {
        authorization = FresnicaSignerAuthorization()
        super.init()
    }

    @objc public static func requiresMainQueueSetup() -> Bool {
        false
    }

    @objc(canEnrollSystemAuth:rejecter:)
    public func canEnrollSystemAuth(
        _ resolve: @escaping FresnicaPromiseResolveBlock,
        rejecter reject: @escaping FresnicaPromiseRejectBlock
    ) {
        resolve(NSNumber(value: authorization.canEnrollSystemAuth()))
    }

    @objc(hasSystemAuth:resolver:rejecter:)
    public func hasSystemAuth(
        _ expectedSignerPublicKey: String,
        resolver resolve: @escaping FresnicaPromiseResolveBlock,
        rejecter reject: @escaping FresnicaPromiseRejectBlock
    ) {
        do {
            resolve(NSNumber(value: try authorization.isSystemAuthEnrolled(
                expectedSignerPublicKey: expectedSignerPublicKey
            )))
        } catch {
            rejectNativeError(error, with: reject)
        }
    }

    @objc(removeSystemAuth:resolver:rejecter:)
    public func removeSystemAuth(
        _ expectedSignerPublicKey: String,
        resolver resolve: @escaping FresnicaPromiseResolveBlock,
        rejecter reject: @escaping FresnicaPromiseRejectBlock
    ) {
        do {
            try authorization.removeSystemAuth(expectedSignerPublicKey: expectedSignerPublicKey)
            resolve(true)
        } catch {
            rejectNativeError(error, with: reject)
        }
    }

    @objc(enrollSystemAuth:appPasscode:expectedSignerPublicKey:resolver:rejecter:)
    public func enrollSystemAuth(
        _ envelopeJson: String,
        appPasscode: String,
        expectedSignerPublicKey: String,
        resolver resolve: @escaping FresnicaPromiseResolveBlock,
        rejecter reject: @escaping FresnicaPromiseRejectBlock
    ) {
        do {
            try authorization.enrollSystemAuth(
                envelopeJson: envelopeJson,
                appPasscode: appPasscode,
                expectedSignerPublicKey: expectedSignerPublicKey
            )
            resolve(true)
        } catch {
            rejectNativeError(error, with: reject)
        }
    }

    @objc(signWithSystemAuth:expectedSignerPublicKey:transactionXdrBase64:networkPassphrase:reason:resolver:rejecter:)
    public func signWithSystemAuth(
        _ envelopeJson: String,
        expectedSignerPublicKey: String,
        transactionXdrBase64: String,
        networkPassphrase: String,
        reason: String,
        resolver resolve: @escaping FresnicaPromiseResolveBlock,
        rejecter reject: @escaping FresnicaPromiseRejectBlock
    ) {
        guard let transactionXdr = Data(base64Encoded: transactionXdrBase64) else {
            rejectInput("transactionXdrBase64 is not valid base64", reject)
            return
        }
        do {
            var signed = try authorization.signWithSystemAuth(
                envelopeJson: envelopeJson,
                expectedSignerPublicKey: expectedSignerPublicKey,
                transactionXdr: transactionXdr,
                networkPassphrase: networkPassphrase,
                reason: reason
            )
            defer { wipe(&signed) }
            resolve(signed.base64EncodedString())
        } catch {
            rejectNativeError(error, with: reject)
        }
    }

    @objc(signWithPasscode:appPasscode:expectedSignerPublicKey:transactionXdrBase64:networkPassphrase:resolver:rejecter:)
    public func signWithPasscode(
        _ envelopeJson: String,
        appPasscode: String,
        expectedSignerPublicKey: String,
        transactionXdrBase64: String,
        networkPassphrase: String,
        resolver resolve: @escaping FresnicaPromiseResolveBlock,
        rejecter reject: @escaping FresnicaPromiseRejectBlock
    ) {
        guard let transactionXdr = Data(base64Encoded: transactionXdrBase64) else {
            rejectInput("transactionXdrBase64 is not valid base64", reject)
            return
        }
        do {
            var signed = try authorization.signWithPasscode(
                envelopeJson: envelopeJson,
                appPasscode: appPasscode,
                expectedSignerPublicKey: expectedSignerPublicKey,
                transactionXdr: transactionXdr,
                networkPassphrase: networkPassphrase
            )
            defer { wipe(&signed) }
            resolve(signed.base64EncodedString())
        } catch {
            rejectNativeError(error, with: reject)
        }
    }

    private func rejectInput(
        _ message: String,
        _ reject: FresnicaPromiseRejectBlock
    ) {
        let error = NSError(
            domain: "FresnicaCoreModule",
            code: 1,
            userInfo: [NSLocalizedDescriptionKey: message]
        )
        reject("invalid-input", message, error)
    }

    private func rejectNativeError(
        _ error: Swift.Error,
        with block: FresnicaPromiseRejectBlock
    ) {
        let mapped = map(error)
        block(mapped.code, mapped.message, error as NSError)
    }

    private func map(_ error: Swift.Error) -> (code: String, message: String) {
        if let core = error as? MobileCoreError {
            switch core {
            case let .InvalidInput(detail): return ("invalid-input", detail)
            case let .InvalidPasscode(detail): return ("invalid-passcode", detail)
            case let .InvalidUnlockKey(detail): return ("invalid-unlock-key", detail)
            case let .InvalidProtectedData(detail): return ("invalid-protected-data", detail)
            case let .IdentityMismatch(detail): return ("identity-mismatch", detail)
            case let .InvalidTransaction(detail): return ("invalid-transaction", detail)
            case let .CoreError(detail): return ("core-error", detail)
            }
        }

        if let store = error as? FresnicaWalletUnlockKeyStore.StoreError {
            switch store {
            case .emptySignerId:
                return ("invalid-input", "signer public key must not be empty")
            case .invalidUnlockKeyLength, .invalidStoredUnlockKeyLength:
                return ("invalid-unlock-key", "WalletUnlockKey has an invalid length")
            case .emptyAuthenticationReason:
                return ("invalid-input", "authentication reason must not be empty")
            case let .biometryUnavailable(detail):
                return ("system-auth-unavailable", detail ?? "Biometric authentication is unavailable")
            case let .accessControlCreationFailed(detail):
                return ("system-auth-unavailable", detail ?? "Unable to create biometric Keychain access control")
            case let .keychain(status):
                switch status {
                case errSecUserCanceled:
                    return ("user-cancel", "Biometric authentication was cancelled")
                case errSecAuthFailed:
                    return ("system-auth-failed", "Biometric authentication failed")
                case errSecItemNotFound:
                    return ("system-auth-not-enrolled", "No system-auth WalletUnlockKey enrollment exists")
                case errSecInteractionNotAllowed:
                    return ("system-auth-unavailable", "Keychain authentication interaction is unavailable")
                default:
                    return ("system-auth-error", "Keychain operation failed (\(status))")
                }
            case .invalidStoredValue:
                return ("invalid-protected-data", "Stored WalletUnlockKey record is invalid")
            }
        }

        if error is FresnicaSignerAuthorization.AuthorizationError {
            return ("invalid-unlock-key", "WalletUnlockKey has an invalid length")
        }

        let nsError = error as NSError
        return ("native-error", nsError.localizedDescription)
    }

    private func wipe(_ data: inout Data) {
        guard !data.isEmpty else { return }
        data.resetBytes(in: 0..<data.count)
        data.removeAll(keepingCapacity: false)
    }
}
