import Darwin
import Foundation
import Security

private let exitPassphraseRequired: Int32 = 10
private let exitCancelled: Int32 = 11
private let exitFailure: Int32 = 20
private let store = FresnicaWalletUnlockKeyStore(
    service: "com.fresnica.terminal.system-auth.v1"
)

private func finish(_ code: Int32, _ message: String? = nil) -> Never {
    if let message {
        FileHandle.standardError.write(Data((message + "\n").utf8))
    }
    exit(code)
}

private func requireSlot(_ arguments: [String]) -> String {
    guard arguments.count == 3, !arguments[2].isEmpty else {
        finish(exitFailure, "system-auth provider requires an exact slot id")
    }
    return arguments[2]
}
private func mapStoreError(_ error: FresnicaWalletUnlockKeyStore.StoreError) -> Never {
    switch error {
    case .biometryUnavailable, .systemAuthDomainMissing:
        finish(exitPassphraseRequired)
    case .keychain(let status):
        switch status {
        case errSecUserCanceled:
            finish(exitCancelled)
        case errSecNotAvailable, errSecAuthFailed,
             errSecItemNotFound, errSecInteractionNotAllowed:
            finish(exitPassphraseRequired)
        default:
            finish(exitFailure, "macOS Keychain error \(status)")
        }
    case .staleSignerRecord, .invalidStoredValue,
         .invalidStoredUnlockKeyLength, .invalidUnlockKeyLength:
        finish(exitFailure, "system-auth stored state is invalid")
    default:
        finish(exitFailure, "system-auth provider failed: \(error)")
    }
}

let arguments = CommandLine.arguments
guard arguments.count >= 2 else {
    finish(exitFailure, "usage: fresnica-system-auth-provider COMMAND [SLOT]")
}
do {
    switch arguments[1] {
    case "probe":
        finish(store.canEnrollSystemAuth() ? 0 : exitPassphraseRequired)
    case "has":
        let slot = requireSlot(arguments)
        finish(try store.isEnrolled(signerId: slot) ? 0 : exitPassphraseRequired)
    case "enroll":
        let slot = requireSlot(arguments)
        let unlockKey = FileHandle.standardInput.readDataToEndOfFile()
        guard unlockKey.count == FresnicaWalletUnlockKeyStore.unlockKeyLength else {
            finish(exitFailure, "system-auth enrollment requires exactly 32 key bytes")
        }
        if try !store.hasDomain() {
            try store.initializeDomain(reason: "Enable Fresnica system authentication")
        }
        try store.enroll(signerId: slot, unlockKey: unlockKey)
        finish(0)
    case "release":
        let slot = requireSlot(arguments)
        let key = try store.load(
            signerId: slot,
            reason: "Authorize Fresnica wallet use"
        )
        FileHandle.standardOutput.write(key)
        finish(0)
    case "delete":
        let slot = requireSlot(arguments)
        try store.delete(signerId: slot)
        finish(0)
    default:
        finish(exitFailure, "unknown system-auth provider command")
    }
} catch let error as FresnicaWalletUnlockKeyStore.StoreError {
    mapStoreError(error)
} catch {
    finish(exitFailure, "system-auth provider failed: \(error)")
}
