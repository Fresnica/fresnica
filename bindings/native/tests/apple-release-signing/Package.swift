// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "FresnicaProtectedSorobanSigningProof",
    platforms: [.iOS(.v13)],
    targets: [
        .binaryTarget(
            name: "FresnicaSDK",
            path: "Vendor/FresnicaSDK.xcframework"
        ),
        .binaryTarget(
            name: "FresnicaSDKFFI",
            path: "Vendor/FresnicaSDKFFI.xcframework"
        ),
        .testTarget(
            name: "ProtectedSorobanSigningTests",
            dependencies: ["FresnicaSDK", "FresnicaSDKFFI"],
            resources: [.copy("Fixtures")]
        ),
    ]
)
