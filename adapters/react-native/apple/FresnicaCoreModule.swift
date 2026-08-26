import Foundation
import Security
import FresnicaSDK

public typealias FresnicaPromiseResolveBlock = @convention(block) (Any?) -> Void
public typealias FresnicaPromiseRejectBlock = @convention(block) (String?, String?, NSError?) -> Void

/// High-level React Native adapter over the Fresnica Native SDK.
///
/// Routine protected-software signing remains native-only. WalletUnlockKey bytes, biometric state
/// and one-shot authorization objects never cross this boundary. Secret-bearing strings cross only
/// for explicit import, one-time mnemonic generation, or explicit Reveal / Export.
@objc(FresnicaCoreModule)
public final class FresnicaCoreModule: NSObject {
    private let core: FresnicaSdkApiProtocol
    private let authorization: FresnicaSignerAuthorization

    public override init() {
        let nativeSdk = FresnicaSdkApi()
        core = nativeSdk
        authorization = FresnicaSignerAuthorization(core: nativeSdk)
        super.init()
    }

    @objc public static func requiresMainQueueSetup() -> Bool {
        false
    }

    // MARK: - Wallet / signer lifecycle

    @objc(parseAccount:resolver:rejecter:)
    public func parseAccount(
        _ address: String,
        resolver resolve: @escaping FresnicaPromiseResolveBlock,
        rejecter reject: @escaping FresnicaPromiseRejectBlock
    ) {
        guard requireNonBlank(address, field: "address", reject: reject) else { return }
        do {
            resolve(accountIdentityDictionary(try core.parseAccount(address: address)))
        } catch {
            rejectNativeError(error, with: reject)
        }
    }

    @objc(protectSecret:appPasscode:expectedSignerPublicKey:resolver:rejecter:)
    public func protectSecret(
        _ secret: String,
        appPasscode: String,
        expectedSignerPublicKey: String?,
        resolver resolve: @escaping FresnicaPromiseResolveBlock,
        rejecter reject: @escaping FresnicaPromiseRejectBlock
    ) {
        do {
            resolve(protectedSignerDictionary(try core.protectSecret(
                secret: secret,
                passcode: appPasscode,
                expectedSignerPublicKey: expectedSignerPublicKey
            )))
        } catch {
            rejectNativeError(error, with: reject)
        }
    }

    @objc(protectMnemonic:mnemonicPassphrase:index:language:appPasscode:expectedSignerPublicKey:resolver:rejecter:)
    public func protectMnemonic(
        _ mnemonic: String,
        mnemonicPassphrase: String,
        index: NSNumber,
        language: String?,
        appPasscode: String,
        expectedSignerPublicKey: String?,
        resolver resolve: @escaping FresnicaPromiseResolveBlock,
        rejecter reject: @escaping FresnicaPromiseRejectBlock
    ) {
        guard let parsedIndex = uint32(index, field: "index", reject: reject) else { return }
        do {
            resolve(protectedSignerDictionary(try core.protectMnemonic(
                mnemonic: mnemonic,
                mnemonicPassphrase: mnemonicPassphrase,
                index: parsedIndex,
                language: language,
                passcode: appPasscode,
                expectedSignerPublicKey: expectedSignerPublicKey
            )))
        } catch {
            rejectNativeError(error, with: reject)
        }
    }

    @objc(generateMnemonic:strength:mnemonicPassphrase:index:appPasscode:resolver:rejecter:)
    public func generateMnemonic(
        _ language: String,
        strength: NSNumber,
        mnemonicPassphrase: String,
        index: NSNumber,
        appPasscode: String,
        resolver resolve: @escaping FresnicaPromiseResolveBlock,
        rejecter reject: @escaping FresnicaPromiseRejectBlock
    ) {
        guard requireNonBlank(language, field: "language", reject: reject) else { return }
        guard let parsedStrength = uint32(strength, field: "strength", reject: reject) else { return }
        guard let parsedIndex = uint32(index, field: "index", reject: reject) else { return }
        do {
            resolve(generatedMnemonicDictionary(try core.generateMnemonic(
                language: language,
                strength: parsedStrength,
                mnemonicPassphrase: mnemonicPassphrase,
                index: parsedIndex,
                passcode: appPasscode
            )))
        } catch {
            rejectNativeError(error, with: reject)
        }
    }

    @objc(deriveMnemonicSigner:appPasscode:expectedSourceSignerPublicKey:index:resolver:rejecter:)
    public func deriveMnemonicSigner(
        _ sourceEnvelopeJson: String,
        appPasscode: String,
        expectedSourceSignerPublicKey: String,
        index: NSNumber,
        resolver resolve: @escaping FresnicaPromiseResolveBlock,
        rejecter reject: @escaping FresnicaPromiseRejectBlock
    ) {
        guard let parsedIndex = uint32(index, field: "index", reject: reject) else { return }
        do {
            resolve(protectedSignerDictionary(try core.deriveMnemonicSigner(
                sourceEnvelopeJson: sourceEnvelopeJson,
                appPasscode: appPasscode,
                expectedSourceSignerPublicKey: expectedSourceSignerPublicKey,
                index: parsedIndex
            )))
        } catch {
            rejectNativeError(error, with: reject)
        }
    }

    @objc(reprotect:currentPasscode:newPasscode:expectedSignerPublicKey:resolver:rejecter:)
    public func reprotect(
        _ envelopeJson: String,
        currentPasscode: String,
        newPasscode: String,
        expectedSignerPublicKey: String,
        resolver resolve: @escaping FresnicaPromiseResolveBlock,
        rejecter reject: @escaping FresnicaPromiseRejectBlock
    ) {
        do {
            resolve(protectedSignerDictionary(try core.reprotect(
                envelopeJson: envelopeJson,
                currentPasscode: currentPasscode,
                newPasscode: newPasscode,
                expectedSignerPublicKey: expectedSignerPublicKey
            )))
        } catch {
            rejectNativeError(error, with: reject)
        }
    }

    @objc(reveal:freshAppPasscode:expectedSignerPublicKey:resolver:rejecter:)
    public func reveal(
        _ envelopeJson: String,
        freshAppPasscode: String,
        expectedSignerPublicKey: String,
        resolver resolve: @escaping FresnicaPromiseResolveBlock,
        rejecter reject: @escaping FresnicaPromiseRejectBlock
    ) {
        do {
            resolve(exportedMaterialDictionary(try core.reveal(
                envelopeJson: envelopeJson,
                freshPasscode: freshAppPasscode,
                expectedSignerPublicKey: expectedSignerPublicKey
            )))
        } catch {
            rejectNativeError(error, with: reject)
        }
    }

    // MARK: - External Ed25519 signer boundary

    @objc(prepareEd25519Signing:networkPassphrase:resolver:rejecter:)
    public func prepareEd25519Signing(
        _ transactionXdrBase64: String,
        networkPassphrase: String,
        resolver resolve: @escaping FresnicaPromiseResolveBlock,
        rejecter reject: @escaping FresnicaPromiseRejectBlock
    ) {
        guard requireNonBlank(networkPassphrase, field: "networkPassphrase", reject: reject) else { return }
        guard var transactionXdr = decodeBase64(
            transactionXdrBase64,
            field: "transactionXdrBase64",
            reject: reject
        ) else { return }
        defer { wipe(&transactionXdr) }

        do {
            let request = try core.prepareEd25519Signing(
                transactionXdr: transactionXdr,
                networkPassphrase: networkPassphrase
            )
            resolve([
                "transactionHashBase64": request.transactionHash.base64EncodedString(),
                "transactionXdrBase64": request.transactionXdr.base64EncodedString(),
                "networkPassphrase": request.networkPassphrase,
            ])
        } catch {
            rejectNativeError(error, with: reject)
        }
    }

    @objc(applyEd25519Signature:networkPassphrase:signerPublicKey:signatureBase64:resolver:rejecter:)
    public func applyEd25519Signature(
        _ transactionXdrBase64: String,
        networkPassphrase: String,
        signerPublicKey: String,
        signatureBase64: String,
        resolver resolve: @escaping FresnicaPromiseResolveBlock,
        rejecter reject: @escaping FresnicaPromiseRejectBlock
    ) {
        guard requireNonBlank(networkPassphrase, field: "networkPassphrase", reject: reject) else { return }
        guard requireNonBlank(signerPublicKey, field: "signerPublicKey", reject: reject) else { return }
        guard var transactionXdr = decodeBase64(
            transactionXdrBase64,
            field: "transactionXdrBase64",
            reject: reject
        ) else { return }
        guard var signature = decodeBase64(
            signatureBase64,
            field: "signatureBase64",
            exactLength: Self.ed25519SignatureBytes,
            reject: reject
        ) else {
            wipe(&transactionXdr)
            return
        }
        defer {
            wipe(&transactionXdr)
            wipe(&signature)
        }

        do {
            var signed = try core.applyEd25519Signature(
                transactionXdr: transactionXdr,
                networkPassphrase: networkPassphrase,
                signerPublicKey: signerPublicKey,
                signature: signature
            )
            defer { wipe(&signed) }
            resolve(signed.base64EncodedString())
        } catch {
            rejectNativeError(error, with: reject)
        }
    }

    // MARK: - Native-only protected software signing

    @objc(canUseSystemAuth:rejecter:)
    public func canUseSystemAuth(
        _ resolve: @escaping FresnicaPromiseResolveBlock,
        rejecter reject: @escaping FresnicaPromiseRejectBlock
    ) {
        resolve(NSNumber(value: authorization.canEnrollSystemAuth()))
    }

    @objc(hasSystemAuthDomain:rejecter:)
    public func hasSystemAuthDomain(
        _ resolve: @escaping FresnicaPromiseResolveBlock,
        rejecter reject: @escaping FresnicaPromiseRejectBlock
    ) {
        do {
            resolve(NSNumber(value: try authorization.hasSystemAuthDomain()))
        } catch {
            rejectNativeError(error, with: reject)
        }
    }

    @objc(initializeSystemAuth:resolver:rejecter:)
    public func initializeSystemAuth(
        _ reason: String,
        resolver resolve: @escaping FresnicaPromiseResolveBlock,
        rejecter reject: @escaping FresnicaPromiseRejectBlock
    ) {
        do {
            try authorization.initializeSystemAuth(reason: reason)
            resolve(true)
        } catch {
            rejectNativeError(error, with: reject)
        }
    }

    @objc(registerSignerSystemAuth:appPasscode:expectedSignerPublicKey:resolver:rejecter:)
    public func registerSignerSystemAuth(
        _ envelopeJson: String,
        appPasscode: String,
        expectedSignerPublicKey: String,
        resolver resolve: @escaping FresnicaPromiseResolveBlock,
        rejecter reject: @escaping FresnicaPromiseRejectBlock
    ) {
        do {
            try authorization.registerSignerSystemAuth(
                envelopeJson: envelopeJson,
                appPasscode: appPasscode,
                expectedSignerPublicKey: expectedSignerPublicKey
            )
            resolve(true)
        } catch {
            rejectNativeError(error, with: reject)
        }
    }

    @objc(hasSignerSystemAuth:resolver:rejecter:)
    public func hasSignerSystemAuth(
        _ expectedSignerPublicKey: String,
        resolver resolve: @escaping FresnicaPromiseResolveBlock,
        rejecter reject: @escaping FresnicaPromiseRejectBlock
    ) {
        do {
            resolve(NSNumber(value: try authorization.isSignerSystemAuthEnrolled(
                expectedSignerPublicKey: expectedSignerPublicKey
            )))
        } catch {
            rejectNativeError(error, with: reject)
        }
    }

    @objc(removeSignerSystemAuth:resolver:rejecter:)
    public func removeSignerSystemAuth(
        _ expectedSignerPublicKey: String,
        resolver resolve: @escaping FresnicaPromiseResolveBlock,
        rejecter reject: @escaping FresnicaPromiseRejectBlock
    ) {
        do {
            try authorization.removeSignerSystemAuth(expectedSignerPublicKey: expectedSignerPublicKey)
            resolve(true)
        } catch {
            rejectNativeError(error, with: reject)
        }
    }

    @objc(removeSystemAuthDomain:rejecter:)
    public func removeSystemAuthDomain(
        _ resolve: @escaping FresnicaPromiseResolveBlock,
        rejecter reject: @escaping FresnicaPromiseRejectBlock
    ) {
        do {
            try authorization.removeSystemAuthDomain()
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
        guard requireNonBlank(networkPassphrase, field: "networkPassphrase", reject: reject) else { return }
        guard requireNonBlank(reason, field: "reason", reject: reject) else { return }
        guard var transactionXdr = decodeBase64(
            transactionXdrBase64,
            field: "transactionXdrBase64",
            reject: reject
        ) else { return }
        defer { wipe(&transactionXdr) }

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
        guard requireNonBlank(networkPassphrase, field: "networkPassphrase", reject: reject) else { return }
        guard var transactionXdr = decodeBase64(
            transactionXdrBase64,
            field: "transactionXdrBase64",
            reject: reject
        ) else { return }
        defer { wipe(&transactionXdr) }

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

    // MARK: - JS DTO conversion

    private func accountIdentityDictionary(_ value: NativeAccountIdentity) -> [String: Any] {
        [
            "kind": value.kind == .classic ? "classic" : "contract",
            "address": value.address,
            "publicKey": value.publicKey as Any? ?? NSNull(),
        ]
    }

    private func protectedSignerDictionary(_ value: NativeProtectedSoftwareSigner) -> [String: Any] {
        [
            "signerPublicKey": value.signerPublicKey,
            "envelopeJson": value.envelopeJson,
        ]
    }

    private func generatedMnemonicDictionary(_ value: NativeGeneratedMnemonic) -> [String: Any] {
        [
            "signer": protectedSignerDictionary(value.signer),
            "mnemonic": value.mnemonic,
            "language": value.language,
            "index": NSNumber(value: value.index),
        ]
    }

    private func exportedMaterialDictionary(_ value: NativeExportedSigningMaterial) -> [String: Any] {
        [
            "kind": value.kind == .secret ? "secret" : "mnemonic",
            "secret": value.secret as Any? ?? NSNull(),
            "mnemonic": value.mnemonic as Any? ?? NSNull(),
            "mnemonicPassphrase": value.mnemonicPassphrase as Any? ?? NSNull(),
            "index": value.index.map { NSNumber(value: $0) } as Any? ?? NSNull(),
            "language": value.language as Any? ?? NSNull(),
        ]
    }

    // MARK: - Input validation / errors

    private func requireNonBlank(
        _ value: String,
        field: String,
        reject: FresnicaPromiseRejectBlock
    ) -> Bool {
        guard !value.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            rejectInput("\(field) must not be blank", reject)
            return false
        }
        return true
    }

    private func uint32(
        _ number: NSNumber,
        field: String,
        reject: FresnicaPromiseRejectBlock
    ) -> UInt32? {
        let value = number.doubleValue
        guard value.isFinite,
              value >= 0,
              value <= Double(UInt32.max),
              value.rounded(.towardZero) == value else {
            rejectInput("\(field) must be an unsigned 32-bit integer", reject)
            return nil
        }
        return UInt32(value)
    }

    private func decodeBase64(
        _ text: String,
        field: String,
        exactLength: Int? = nil,
        reject: FresnicaPromiseRejectBlock
    ) -> Data? {
        guard !text.isEmpty else {
            rejectInput("\(field) must not be empty", reject)
            return nil
        }
        guard let data = Data(base64Encoded: text, options: []), !data.isEmpty else {
            rejectInput("\(field) is not valid non-empty base64", reject)
            return nil
        }
        if let exactLength, data.count != exactLength {
            rejectInput("\(field) must decode to exactly \(exactLength) bytes", reject)
            return nil
        }
        return data
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
        if let core = error as? NativeSdkError {
            switch core {
            case let .InvalidInput(detail): return ("invalid-input", detail)
            case let .InvalidPasscode(detail): return ("invalid-passcode", detail)
            case let .InvalidUnlockKey(detail): return ("invalid-unlock-key", detail)
            case let .InvalidProtectedData(detail): return ("invalid-protected-data", detail)
            case let .IdentityMismatch(detail): return ("identity-mismatch", detail)
            case let .InvalidTransaction(detail): return ("invalid-transaction", detail)
            case let .CoreError(detail): return ("core-error", detail)
            @unknown default:
                return ("core-error", "Unknown Native SDK error")
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
            case .systemAuthDomainMissing:
                return ("system-auth-not-enrolled", "No Fresnica system-auth domain exists")
            case .staleSignerRecord:
                return ("system-auth-not-enrolled", "Signer system-auth registration belongs to a stale domain")
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
            case let .crypto(detail):
                return ("system-auth-error", detail ?? "System-auth cryptographic operation failed")
            case .invalidStoredValue:
                return ("invalid-protected-data", "Stored system-auth record is invalid")
            @unknown default:
                return ("system-auth-error", "Unknown system-auth Keychain error")
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

    private static let ed25519SignatureBytes = 64
}
