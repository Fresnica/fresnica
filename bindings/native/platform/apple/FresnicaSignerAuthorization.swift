import Foundation

/// Native-only protected-software signer orchestration for Fresnica on Apple platforms.
///
/// A native host or reviewed framework adapter may provide reviewed XDR, an encrypted signer
/// envelope, signer public key, and an app passcode for an explicit fallback/enrollment action.
/// WalletUnlockKey bytes remain inside this native service and are zeroed on a best-effort basis
/// after the Rust SDK call.
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

    public func canEnrollSystemAuth() -> Bool {
        keyStore.canEnrollBiometry()
    }

    public func isSystemAuthEnrolled(expectedSignerPublicKey: String) throws -> Bool {
        try keyStore.isEnrolled(signerId: expectedSignerPublicKey)
    }

    /// Derives a verified key through the Fresnica SDK and immediately moves it into biometric Keychain.
    public func enrollSystemAuth(
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
        try keyStore.enroll(
            signerId: expectedSignerPublicKey,
            unlockKey: unlockKey
        )
    }

    /// Keychain access performs the real Face ID / Touch ID operation that releases the key.
    /// Only signed XDR leaves this method.
    public func signWithSystemAuth(
        envelopeJson: String,
        expectedSignerPublicKey: String,
        transactionXdr: Data,
        networkPassphrase: String,
        reason: String
    ) throws -> Data {
        var unlockKey = try keyStore.load(
            signerId: expectedSignerPublicKey,
            reason: reason
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

    /// App-passcode fallback. The derived WalletUnlockKey stays native and is never returned.
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

    public func removeSystemAuth(expectedSignerPublicKey: String) throws {
        try keyStore.delete(signerId: expectedSignerPublicKey)
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
