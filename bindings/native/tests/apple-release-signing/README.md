This package is an iOS Simulator test harness for the published Fresnica Native SDK.

The validation workflow populates `Vendor/` from the released Apple ZIP and copies the canonical JSON vectors into `Tests/ProtectedSorobanSigningTests/Fixtures/` before running `xcodebuild test`. The release binaries, not locally rebuilt Native SDK source, are under test.
