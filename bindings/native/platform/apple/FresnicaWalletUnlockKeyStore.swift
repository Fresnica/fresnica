import Foundation
import LocalAuthentication
import Security

/// Device-level system-auth protection domain for Fresnica WalletUnlockKeys.
///
/// One Keychain-protected EC private key authorizes use of every local software signer on this
/// installation. Its public key wraps each signer's independent WalletUnlockKey without prompting.
/// The private key is gated by the current biometric set and is used only to unwrap during signing.
public final class FresnicaWalletUnlockKeyStore {
    public static let unlockKeyLength = 32

    private let domainService: String
    private let signerService: String
    private let activeDomainAccount = "active-domain"
    private let algorithm: SecKeyAlgorithm = .eciesEncryptionCofactorX963SHA256AESGCM

    public init(service: String = "com.fresnica.system-auth.v2") {
        self.domainService = service + ".domain"
        self.signerService = service + ".signers"
    }

    public func canEnrollBiometry() -> Bool {
        let context = LAContext()
        var error: NSError?
        return context.canEvaluatePolicy(.deviceOwnerAuthenticationWithBiometrics, error: &error)
    }

    public func hasDomain() throws -> Bool {
        guard let record = try activeDomainRecord() else { return false }
        return try privateKeyExists(tag: record.tag)
    }

    /// Creates/replaces the device system-auth domain and proves private-key use with one biometric prompt.
    /// Existing signer wraps are discarded only after the new domain has been authenticated and committed.
    public func initializeDomain(reason: String) throws {
        guard !reason.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            throw StoreError.emptyAuthenticationReason
        }

        let context = LAContext()
        var biometricError: NSError?
        guard context.canEvaluatePolicy(.deviceOwnerAuthenticationWithBiometrics, error: &biometricError) else {
            throw StoreError.biometryUnavailable(biometricError?.localizedDescription)
        }

        let previous = try activeDomainRecord()
        let tag = Data("com.fresnica.system-auth.\(UUID().uuidString)".utf8)
        let privateKey = try createPrivateKey(tag: tag)
        var committed = false
        defer {
            if !committed { try? deletePrivateKey(tag: tag) }
        }

        guard let publicKey = SecKeyCopyPublicKey(privateKey) else {
            throw StoreError.crypto("Unable to obtain system-auth public key")
        }
        guard SecKeyIsAlgorithmSupported(publicKey, .encrypt, algorithm),
              SecKeyIsAlgorithmSupported(privateKey, .decrypt, algorithm) else {
            throw StoreError.crypto("System-auth ECIES algorithm is unavailable")
        }

        var challenge = Data(count: 32)
        let randomStatus = challenge.withUnsafeMutableBytes { buffer in
            SecRandomCopyBytes(kSecRandomDefault, buffer.count, buffer.baseAddress!)
        }
        guard randomStatus == errSecSuccess else { throw StoreError.keychain(randomStatus) }
        defer { challenge.resetBytes(in: 0..<challenge.count) }

        var cryptoError: Unmanaged<CFError>?
        guard let ciphertext = SecKeyCreateEncryptedData(
            publicKey,
            algorithm,
            challenge as CFData,
            &cryptoError
        ) as Data? else {
            throw StoreError.crypto(cryptoError?.takeRetainedValue().localizedDescription)
        }

        context.localizedReason = reason
        context.localizedFallbackTitle = ""
        let authenticatedKey = try loadPrivateKey(tag: tag, context: context)
        cryptoError = nil
        guard var clear = SecKeyCreateDecryptedData(
            authenticatedKey,
            algorithm,
            ciphertext as CFData,
            &cryptoError
        ) as Data? else {
            throw StoreError.crypto(cryptoError?.takeRetainedValue().localizedDescription)
        }
        defer { clear.resetBytes(in: 0..<clear.count) }
        guard clear == challenge else {
            throw StoreError.crypto("System-auth domain challenge verification failed")
        }

        cryptoError = nil
        guard let publicData = SecKeyCopyExternalRepresentation(publicKey, &cryptoError) as Data? else {
            throw StoreError.crypto(cryptoError?.takeRetainedValue().localizedDescription)
        }
        try storeActiveDomain(DomainRecord(tag: tag, publicKey: publicData))
        committed = true

        // Signer records are domain-tagged and therefore fail closed after the active domain changes.
        // Cleanup is best effort: failure must not invalidate the newly committed domain.
        try? deleteAllSignerRecords()
        if let previous, previous.tag != tag {
            try? deletePrivateKey(tag: previous.tag)
        }
    }

    /// Wraps/replaces one verified per-signer WalletUnlockKey without biometric authentication.
    public func enroll(signerId: String, unlockKey: Data) throws {
        try validateSignerId(signerId)
        guard unlockKey.count == Self.unlockKeyLength else {
            throw StoreError.invalidUnlockKeyLength
        }
        guard let domain = try activeDomainRecord() else {
            throw StoreError.systemAuthDomainMissing
        }
        let publicKey = try publicKey(from: domain.publicKey)
        guard SecKeyIsAlgorithmSupported(publicKey, .encrypt, algorithm) else {
            throw StoreError.crypto("System-auth public key cannot wrap WalletUnlockKey")
        }
        var cryptoError: Unmanaged<CFError>?
        guard let ciphertext = SecKeyCreateEncryptedData(
            publicKey,
            algorithm,
            unlockKey as CFData,
            &cryptoError
        ) as Data? else {
            throw StoreError.crypto(cryptoError?.takeRetainedValue().localizedDescription)
        }
        try upsertGenericPassword(
            service: signerService,
            account: signerId,
            value: try JSONEncoder().encode(SignerRecord(domainTag: domain.tag, ciphertext: ciphertext))
        )
    }

    public func isEnrolled(signerId: String) throws -> Bool {
        try validateSignerId(signerId)
        guard let domain = try activeDomainRecord(), try privateKeyExists(tag: domain.tag) else {
            return false
        }
        guard let data = try genericPassword(service: signerService, account: signerId) else {
            return false
        }
        guard let record = try? JSONDecoder().decode(SignerRecord.self, from: data) else {
            return false
        }
        return record.domainTag == domain.tag
    }

    /// Biometric-gated unwrap of one signer's WalletUnlockKey.
    public func load(signerId: String, reason: String) throws -> Data {
        try validateSignerId(signerId)
        guard !reason.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            throw StoreError.emptyAuthenticationReason
        }
        guard let domain = try activeDomainRecord() else {
            throw StoreError.systemAuthDomainMissing
        }
        guard let data = try genericPassword(service: signerService, account: signerId),
              let record = try? JSONDecoder().decode(SignerRecord.self, from: data) else {
            throw StoreError.keychain(errSecItemNotFound)
        }
        guard record.domainTag == domain.tag else { throw StoreError.staleSignerRecord }

        let context = LAContext()
        context.localizedReason = reason
        context.localizedFallbackTitle = ""
        let privateKey = try loadPrivateKey(tag: domain.tag, context: context)
        guard SecKeyIsAlgorithmSupported(privateKey, .decrypt, algorithm) else {
            throw StoreError.crypto("System-auth private key cannot unwrap WalletUnlockKey")
        }
        var cryptoError: Unmanaged<CFError>?
        guard let clear = SecKeyCreateDecryptedData(
            privateKey,
            algorithm,
            record.ciphertext as CFData,
            &cryptoError
        ) as Data? else {
            throw StoreError.crypto(cryptoError?.takeRetainedValue().localizedDescription)
        }
        guard clear.count == Self.unlockKeyLength else {
            throw StoreError.invalidStoredUnlockKeyLength
        }
        return clear
    }

    public func delete(signerId: String) throws {
        try validateSignerId(signerId)
        try deleteGenericPassword(service: signerService, account: signerId)
    }

    public func deleteDomain() throws {
        let active = try activeDomainRecord()
        try deleteAllSignerRecords()
        try deleteGenericPassword(service: domainService, account: activeDomainAccount)
        if let active { try deletePrivateKey(tag: active.tag) }
    }

    private func createPrivateKey(tag: Data) throws -> SecKey {
        var accessError: Unmanaged<CFError>?
        guard let accessControl = SecAccessControlCreateWithFlags(
            nil,
            kSecAttrAccessibleWhenPasscodeSetThisDeviceOnly,
            [.biometryCurrentSet, .privateKeyUsage],
            &accessError
        ) else {
            throw StoreError.accessControlCreationFailed(
                accessError?.takeRetainedValue().localizedDescription
            )
        }

        let attributes: [CFString: Any] = [
            kSecAttrKeyType: kSecAttrKeyTypeECSECPrimeRandom,
            kSecAttrKeySizeInBits: 256,
            kSecPrivateKeyAttrs: [
                kSecAttrIsPermanent: true,
                kSecAttrApplicationTag: tag,
                kSecAttrAccessControl: accessControl,
            ],
        ]
        var keyError: Unmanaged<CFError>?
        guard let key = SecKeyCreateRandomKey(attributes as CFDictionary, &keyError) else {
            throw StoreError.crypto(keyError?.takeRetainedValue().localizedDescription)
        }
        return key
    }

    private func publicKey(from data: Data) throws -> SecKey {
        let attributes: [CFString: Any] = [
            kSecAttrKeyType: kSecAttrKeyTypeECSECPrimeRandom,
            kSecAttrKeyClass: kSecAttrKeyClassPublic,
            kSecAttrKeySizeInBits: 256,
        ]
        var error: Unmanaged<CFError>?
        guard let key = SecKeyCreateWithData(data as CFData, attributes as CFDictionary, &error) else {
            throw StoreError.crypto(error?.takeRetainedValue().localizedDescription)
        }
        return key
    }

    private func loadPrivateKey(tag: Data, context: LAContext) throws -> SecKey {
        var query: [CFString: Any] = [
            kSecClass: kSecClassKey,
            kSecAttrKeyClass: kSecAttrKeyClassPrivate,
            kSecAttrApplicationTag: tag,
            kSecReturnRef: true,
            kSecMatchLimit: kSecMatchLimitOne,
            kSecUseAuthenticationContext: context,
        ]
        useDataProtectionKeychainIfNeeded(&query)
        var item: CFTypeRef?
        let status = SecItemCopyMatching(query as CFDictionary, &item)
        guard status == errSecSuccess, let key = item as! SecKey? else {
            throw StoreError.keychain(status)
        }
        return key
    }

    private func privateKeyExists(tag: Data) throws -> Bool {
        var query: [CFString: Any] = [
            kSecClass: kSecClassKey,
            kSecAttrKeyClass: kSecAttrKeyClassPrivate,
            kSecAttrApplicationTag: tag,
            kSecReturnAttributes: true,
            kSecMatchLimit: kSecMatchLimitOne,
        ]
        useDataProtectionKeychainIfNeeded(&query)
        let status = SecItemCopyMatching(query as CFDictionary, nil)
        if status == errSecSuccess { return true }
        if status == errSecItemNotFound { return false }
        throw StoreError.keychain(status)
    }

    private func deletePrivateKey(tag: Data) throws {
        var query: [CFString: Any] = [
            kSecClass: kSecClassKey,
            kSecAttrKeyClass: kSecAttrKeyClassPrivate,
            kSecAttrApplicationTag: tag,
        ]
        useDataProtectionKeychainIfNeeded(&query)
        let status = SecItemDelete(query as CFDictionary)
        guard status == errSecSuccess || status == errSecItemNotFound else {
            throw StoreError.keychain(status)
        }
    }

    private func activeDomainRecord() throws -> DomainRecord? {
        guard let data = try genericPassword(service: domainService, account: activeDomainAccount) else {
            return nil
        }
        guard let record = try? JSONDecoder().decode(DomainRecord.self, from: data) else {
            throw StoreError.invalidStoredValue
        }
        return record
    }

    private func storeActiveDomain(_ record: DomainRecord) throws {
        try upsertGenericPassword(
            service: domainService,
            account: activeDomainAccount,
            value: try JSONEncoder().encode(record)
        )
    }

    private func genericPassword(service: String, account: String) throws -> Data? {
        var query: [CFString: Any] = [
            kSecClass: kSecClassGenericPassword,
            kSecAttrService: service,
            kSecAttrAccount: account,
            kSecReturnData: true,
            kSecMatchLimit: kSecMatchLimitOne,
        ]
        useDataProtectionKeychainIfNeeded(&query)
        var item: CFTypeRef?
        let status = SecItemCopyMatching(query as CFDictionary, &item)
        if status == errSecItemNotFound { return nil }
        guard status == errSecSuccess, let data = item as? Data else {
            throw StoreError.keychain(status)
        }
        return data
    }

    private func upsertGenericPassword(service: String, account: String, value: Data) throws {
        var query: [CFString: Any] = [
            kSecClass: kSecClassGenericPassword,
            kSecAttrService: service,
            kSecAttrAccount: account,
        ]
        useDataProtectionKeychainIfNeeded(&query)
        let updateStatus = SecItemUpdate(query as CFDictionary, [kSecValueData: value] as CFDictionary)
        if updateStatus == errSecSuccess { return }
        guard updateStatus == errSecItemNotFound else { throw StoreError.keychain(updateStatus) }

        query[kSecValueData] = value
        query[kSecAttrAccessible] = kSecAttrAccessibleWhenPasscodeSetThisDeviceOnly
        query[kSecAttrSynchronizable] = false
        let addStatus = SecItemAdd(query as CFDictionary, nil)
        guard addStatus == errSecSuccess else { throw StoreError.keychain(addStatus) }
    }

    private func deleteGenericPassword(service: String, account: String) throws {
        var query: [CFString: Any] = [
            kSecClass: kSecClassGenericPassword,
            kSecAttrService: service,
            kSecAttrAccount: account,
        ]
        useDataProtectionKeychainIfNeeded(&query)
        let status = SecItemDelete(query as CFDictionary)
        guard status == errSecSuccess || status == errSecItemNotFound else {
            throw StoreError.keychain(status)
        }
    }

    private func deleteAllSignerRecords() throws {
        var query: [CFString: Any] = [
            kSecClass: kSecClassGenericPassword,
            kSecAttrService: signerService,
        ]
        useDataProtectionKeychainIfNeeded(&query)
        let status = SecItemDelete(query as CFDictionary)
        guard status == errSecSuccess || status == errSecItemNotFound else {
            throw StoreError.keychain(status)
        }
    }

    private func useDataProtectionKeychainIfNeeded(_ query: inout [CFString: Any]) {
#if os(macOS)
        query[kSecUseDataProtectionKeychain] = true
#endif
    }

    private func validateSignerId(_ signerId: String) throws {
        guard !signerId.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            throw StoreError.emptySignerId
        }
    }

    private struct DomainRecord: Codable, Equatable {
        let tag: Data
        let publicKey: Data
    }

    private struct SignerRecord: Codable, Equatable {
        let domainTag: Data
        let ciphertext: Data
    }

    public enum StoreError: Swift.Error, Equatable {
        case emptySignerId
        case invalidUnlockKeyLength
        case emptyAuthenticationReason
        case biometryUnavailable(String?)
        case accessControlCreationFailed(String?)
        case systemAuthDomainMissing
        case staleSignerRecord
        case keychain(OSStatus)
        case crypto(String?)
        case invalidStoredValue
        case invalidStoredUnlockKeyLength
    }
}
