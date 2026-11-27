import Foundation
import XCTest
import FresnicaSDK

final class ProtectedSorobanSigningTests: XCTestCase {
    func testReleasedNativeSdkSignsLiveBlendTransactionWithPasscode() throws {
        let fixture = try jsonFixture("protected-soroban-signing-v1")
        let signing = try jsonFixture("transaction-signing-v1")
        let signerCases = try XCTUnwrap(signing["cases"] as? [[String: Any]])
        let signer = try XCTUnwrap(signerCases.first)
        let transaction = try XCTUnwrap(fixture["transaction"] as? [String: Any])
        let publicKey = try XCTUnwrap(fixture["source_account"] as? String)
        let passcode = "test-passphrase"
        let unsigned = try XCTUnwrap(
            Data(base64Encoded: try XCTUnwrap(transaction["unsigned_xdr_base64"] as? String))
        )
        let expectedSigned = try XCTUnwrap(
            Data(base64Encoded: try XCTUnwrap(transaction["expected_signed_xdr_base64"] as? String))
        )

        let api: FresnicaSdkApiProtocol = FresnicaSdkApi()
        let protectedSigner = try api.protectSecret(
            secret: try XCTUnwrap(signer["secret"] as? String),
            passcode: passcode,
            expectedSignerPublicKey: publicKey
        )
        let authorization = FresnicaSignerAuthorization(core: api)

        let signed = try authorization.signWithPasscode(
            envelopeJson: protectedSigner.envelopeJson,
            appPasscode: passcode,
            expectedSignerPublicKey: publicKey,
            transactionXdr: unsigned,
            networkPassphrase: try XCTUnwrap(fixture["network_passphrase"] as? String)
        )
        XCTAssertEqual(signed, expectedSigned)

        let wrongNetworkSigned = try authorization.signWithPasscode(
            envelopeJson: protectedSigner.envelopeJson,
            appPasscode: passcode,
            expectedSignerPublicKey: publicKey,
            transactionXdr: unsigned,
            networkPassphrase: try XCTUnwrap(fixture["wrong_network_passphrase"] as? String)
        )
        XCTAssertNotEqual(wrongNetworkSigned, expectedSigned)

        XCTAssertThrowsError(
            try authorization.signWithPasscode(
                envelopeJson: protectedSigner.envelopeJson,
                appPasscode: passcode,
                expectedSignerPublicKey: try XCTUnwrap(fixture["wrong_signer_public_key"] as? String),
                transactionXdr: unsigned,
                networkPassphrase: try XCTUnwrap(fixture["network_passphrase"] as? String)
            )
        )
    }

    private func jsonFixture(_ name: String) throws -> [String: Any] {
        let url = try XCTUnwrap(
            Bundle.module.url(
                forResource: name,
                withExtension: "json",
                subdirectory: "Fixtures"
            )
        )
        let data = try Data(contentsOf: url)
        return try XCTUnwrap(
            JSONSerialization.jsonObject(with: data) as? [String: Any]
        )
    }
}
