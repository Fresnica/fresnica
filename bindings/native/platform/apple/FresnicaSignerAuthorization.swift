import Foundation

/// Native-only protected-software signer authorization for Fresnica on Apple platforms.
public final class FresnicaSignerAuthorization {
    private let core: FresnicaSdkApiProtocol
    private let keyStore: FresnicaWalletUnlockKeyStore

    public init(
        core: FresnicaSdkApiProtocol = FresnicaSdkApi(),
        keyStore: FresnicaWalletUnlockKeyStore = FresnicaWalletUnlockKeyStore()
    ) {
        self.core = core
        self.keyStore = keyStore
    }

    public func canEnrollSystemAuth() -> Bool { keyStore.canEnrollBiometry() }

    public func hasSystemAuthDomain() throws -> Bool { try keyStore.hasDomain() }

    /// One device-level biometric enrollment. No signer secret is involved in this operation.
    public func initializeSystemAuth(reason: String) throws {
        try keyStore.initializeDomain(reason: reason)
    }

    /// Passcode-authenticated signer registration. This wraps the verified key with the domain
    /// public key and therefore does not trigger Face ID / Touch ID.
    public func registerSignerSystemAuth(
        envelopeJson: String,
        appPasscode: String,
        expectedSignerPublicKey: String
    ) throws {
        var unlockKey = try core.deriveUnlockKey(
            envelopeJson: envelopeJson,
            passcode: appPasscode,
            expectedSignerPublicKey: expectedSignerPublicKey
        )
        defer { wipe(&unlockKey) }
        guard unlockKey.count == FresnicaWalletUnlockKeyStore.unlockKeyLength else {
            throw AuthorizationError.invalidUnlockKeyLength
        }
        try keyStore.enroll(signerId: expectedSignerPublicKey, unlockKey: unlockKey)
    }

    public func isSignerSystemAuthEnrolled(expectedSignerPublicKey: String) throws -> Bool {
        try keyStore.isEnrolled(signerId: expectedSignerPublicKey)
    }

    public func removeSignerSystemAuth(expectedSignerPublicKey: String) throws {
        try keyStore.delete(signerId: expectedSignerPublicKey)
    }

    public func removeSystemAuthDomain() throws { try keyStore.deleteDomain() }

    public func signWithSystemAuth(
        envelopeJson: String,
        expectedSignerPublicKey: String,
        transactionXdr: Data,
        networkPassphrase: String,
        reason: String
    ) throws -> Data {
        var unlockKey = try keyStore.load(signerId: expectedSignerPublicKey, reason: reason)
        defer { wipe(&unlockKey) }
        guard unlockKey.count == FresnicaWalletUnlockKeyStore.unlockKeyLength else {
            throw AuthorizationError.invalidUnlockKeyLength
        }
        return try core.signTransactionXdr(
            envelopeJson: envelopeJson,
            unlockKey: unlockKey,
            expectedSignerPublicKey: expectedSignerPublicKey,
            transactionXdr: transactionXdr,
            networkPassphrase: networkPassphrase
        )
    }

    public func signMessageWithSystemAuth(
        envelopeJson: String,
        expectedSignerPublicKey: String,
        message: Data,
        reason: String
    ) throws -> Data {
        var unlockKey = try keyStore.load(signerId: expectedSignerPublicKey, reason: reason)
        defer { wipe(&unlockKey) }
        guard unlockKey.count == FresnicaWalletUnlockKeyStore.unlockKeyLength else {
            throw AuthorizationError.invalidUnlockKeyLength
        }
        return try core.signMessage(
            envelopeJson: envelopeJson,
            unlockKey: unlockKey,
            expectedSignerPublicKey: expectedSignerPublicKey,
            message: message
        )
    }

    public func signMessageWithPasscode(
        envelopeJson: String,
        appPasscode: String,
        expectedSignerPublicKey: String,
        message: Data
    ) throws -> Data {
        try core.signMessageWithPasscode(
            envelopeJson: envelopeJson,
            appPasscode: appPasscode,
            expectedSignerPublicKey: expectedSignerPublicKey,
            message: message
        )
    }

    public func signWithPasscode(
        envelopeJson: String,
        appPasscode: String,
        expectedSignerPublicKey: String,
        transactionXdr: Data,
        networkPassphrase: String
    ) throws -> Data {
        var unlockKey = try core.deriveUnlockKey(
            envelopeJson: envelopeJson,
            passcode: appPasscode,
            expectedSignerPublicKey: expectedSignerPublicKey
        )
        defer { wipe(&unlockKey) }
        guard unlockKey.count == FresnicaWalletUnlockKeyStore.unlockKeyLength else {
            throw AuthorizationError.invalidUnlockKeyLength
        }
        return try core.signTransactionXdr(
            envelopeJson: envelopeJson,
            unlockKey: unlockKey,
            expectedSignerPublicKey: expectedSignerPublicKey,
            transactionXdr: transactionXdr,
            networkPassphrase: networkPassphrase
        )
    }

    private func wipe(_ data: inout Data) {
        guard !data.isEmpty else { return }
        data.resetBytes(in: 0..<data.count)
        data.removeAll(keepingCapacity: false)
    }

    public enum AuthorizationError: Swift.Error, Equatable {
        case invalidUnlockKeyLength
    }
}
