import Foundation
import LocalAuthentication
import Security

/// Native-only Keychain storage for one Fresnica WalletUnlockKey per software signer.
///
/// The 32-byte value is stored as a ThisDeviceOnly Keychain item protected by the
/// currently enrolled biometric set. Reading the item performs the actual LocalAuthentication
/// operation that releases the bytes; a separate successful biometric probe is not sufficient.
///
/// The returned Data must remain native-only and be cleared/dropped immediately after calling
/// Fresnica Core. Routine signing must never return WalletUnlockKey bytes to React Native.
public final class FresnicaWalletUnlockKeyStore {
    public static let unlockKeyLength = 32

    private let service: String

    public init(service: String = "com.fresnica.wallet-unlock-key.v1") {
        self.service = service
    }

    public func canEnrollBiometry() -> Bool {
        let context = LAContext()
        var error: NSError?
        return context.canEvaluatePolicy(.deviceOwnerAuthenticationWithBiometrics, error: &error)
    }

    /// Best-effort enrollment status without releasing the protected WalletUnlockKey bytes.
    public func isEnrolled(signerId: String) throws -> Bool {
        try validateSignerId(signerId)
        let query: [CFString: Any] = [
            kSecClass: kSecClassGenericPassword,
            kSecAttrService: service,
            kSecAttrAccount: signerId,
            kSecMatchLimit: kSecMatchLimitOne
        ]
        let status = SecItemCopyMatching(query as CFDictionary, nil)
        switch status {
        case errSecSuccess:
            return true
        case errSecItemNotFound:
            return false
        default:
            throw StoreError.keychain(status)
        }
    }

    /// Stores a new per-signer unlock key.
    ///
    /// Re-enrollment replaces the previous convenience credential. If replacement fails, the
    /// caller can always recover by asking for the Fresnica app passcode and deriving a new key
    /// through Rust Core.
    public func enroll(signerId: String, unlockKey: Data) throws {
        try validateSignerId(signerId)
        guard unlockKey.count == Self.unlockKeyLength else {
            throw StoreError.invalidUnlockKeyLength
        }

        let context = LAContext()
        var biometricError: NSError?
        guard context.canEvaluatePolicy(.deviceOwnerAuthenticationWithBiometrics,
                                        error: &biometricError) else {
            throw StoreError.biometryUnavailable(biometricError?.localizedDescription)
        }

        var accessError: Unmanaged<CFError>?
        guard let accessControl = SecAccessControlCreateWithFlags(
            nil,
            kSecAttrAccessibleWhenPasscodeSetThisDeviceOnly,
            .biometryCurrentSet,
            &accessError
        ) else {
            let message = accessError?.takeRetainedValue().localizedDescription
            throw StoreError.accessControlCreationFailed(message)
        }

        try delete(signerId: signerId)

        let addQuery: [CFString: Any] = [
            kSecClass: kSecClassGenericPassword,
            kSecAttrService: service,
            kSecAttrAccount: signerId,
            kSecAttrAccessControl: accessControl,
            kSecValueData: unlockKey,
            kSecAttrSynchronizable: false
        ]

        let status = SecItemAdd(addQuery as CFDictionary, nil)
        guard status == errSecSuccess else {
            throw StoreError.keychain(status)
        }
    }

    /// Authenticates against the exact Keychain access control and returns the stored 32 bytes.
    public func load(signerId: String, reason: String) throws -> Data {
        try validateSignerId(signerId)
        guard !reason.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            throw StoreError.emptyAuthenticationReason
        }

        let context = LAContext()
        context.localizedReason = reason
        context.localizedFallbackTitle = ""

        let query: [CFString: Any] = [
            kSecClass: kSecClassGenericPassword,
            kSecAttrService: service,
            kSecAttrAccount: signerId,
            kSecReturnData: true,
            kSecMatchLimit: kSecMatchLimitOne,
            kSecUseAuthenticationContext: context
        ]

        var item: CFTypeRef?
        let status = SecItemCopyMatching(query as CFDictionary, &item)
        guard status == errSecSuccess else {
            throw StoreError.keychain(status)
        }
        guard let data = item as? Data else {
            throw StoreError.invalidStoredValue
        }
        guard data.count == Self.unlockKeyLength else {
            throw StoreError.invalidStoredUnlockKeyLength
        }
        return data
    }

    /// Removes the Keychain item. Missing enrollment is treated as success.
    public func delete(signerId: String) throws {
        try validateSignerId(signerId)
        let query: [CFString: Any] = [
            kSecClass: kSecClassGenericPassword,
            kSecAttrService: service,
            kSecAttrAccount: signerId
        ]
        let status = SecItemDelete(query as CFDictionary)
        guard status == errSecSuccess || status == errSecItemNotFound else {
            throw StoreError.keychain(status)
        }
    }

    private func validateSignerId(_ signerId: String) throws {
        guard !signerId.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            throw StoreError.emptySignerId
        }
    }

    public enum StoreError: Swift.Error, Equatable {
        case emptySignerId
        case invalidUnlockKeyLength
        case emptyAuthenticationReason
        case biometryUnavailable(String?)
        case accessControlCreationFailed(String?)
        case keychain(OSStatus)
        case invalidStoredValue
        case invalidStoredUnlockKeyLength
    }
}
