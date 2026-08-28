#import <React/RCTBridgeModule.h>

// Swift implementation lives in FresnicaCoreModule.swift. RCT_EXTERN_REMAP_MODULE keeps the
// Xaman-compatible Objective-C React Native bridge thin, exports the shared FresnicaCore JS name,
// and does not expose native key material.
@interface RCT_EXTERN_REMAP_MODULE(FresnicaCore, FresnicaCoreModule, NSObject)

RCT_EXTERN_METHOD(parseAccount:(NSString *)address
                  resolver:(RCTPromiseResolveBlock)resolve
                  rejecter:(RCTPromiseRejectBlock)reject)

RCT_EXTERN_METHOD(protectSecret:(NSString *)secret
                  appPasscode:(NSString *)appPasscode
                  expectedSignerPublicKey:(NSString *)expectedSignerPublicKey
                  resolver:(RCTPromiseResolveBlock)resolve
                  rejecter:(RCTPromiseRejectBlock)reject)

RCT_EXTERN_METHOD(protectMnemonic:(NSString *)mnemonic
                  mnemonicPassphrase:(NSString *)mnemonicPassphrase
                  index:(nonnull NSNumber *)index
                  language:(NSString *)language
                  appPasscode:(NSString *)appPasscode
                  expectedSignerPublicKey:(NSString *)expectedSignerPublicKey
                  resolver:(RCTPromiseResolveBlock)resolve
                  rejecter:(RCTPromiseRejectBlock)reject)

RCT_EXTERN_METHOD(generateMnemonic:(NSString *)language
                  strength:(nonnull NSNumber *)strength
                  mnemonicPassphrase:(NSString *)mnemonicPassphrase
                  index:(nonnull NSNumber *)index
                  appPasscode:(NSString *)appPasscode
                  resolver:(RCTPromiseResolveBlock)resolve
                  rejecter:(RCTPromiseRejectBlock)reject)

RCT_EXTERN_METHOD(deriveMnemonicSigner:(NSString *)sourceEnvelopeJson
                  appPasscode:(NSString *)appPasscode
                  expectedSourceSignerPublicKey:(NSString *)expectedSourceSignerPublicKey
                  index:(nonnull NSNumber *)index
                  resolver:(RCTPromiseResolveBlock)resolve
                  rejecter:(RCTPromiseRejectBlock)reject)

RCT_EXTERN_METHOD(reprotect:(NSString *)envelopeJson
                  currentPasscode:(NSString *)currentPasscode
                  newPasscode:(NSString *)newPasscode
                  expectedSignerPublicKey:(NSString *)expectedSignerPublicKey
                  resolver:(RCTPromiseResolveBlock)resolve
                  rejecter:(RCTPromiseRejectBlock)reject)

RCT_EXTERN_METHOD(reveal:(NSString *)envelopeJson
                  freshAppPasscode:(NSString *)freshAppPasscode
                  expectedSignerPublicKey:(NSString *)expectedSignerPublicKey
                  resolver:(RCTPromiseResolveBlock)resolve
                  rejecter:(RCTPromiseRejectBlock)reject)

RCT_EXTERN_METHOD(prepareEd25519Signing:(NSString *)transactionXdrBase64
                  networkPassphrase:(NSString *)networkPassphrase
                  resolver:(RCTPromiseResolveBlock)resolve
                  rejecter:(RCTPromiseRejectBlock)reject)

RCT_EXTERN_METHOD(applyEd25519Signature:(NSString *)transactionXdrBase64
                  networkPassphrase:(NSString *)networkPassphrase
                  signerPublicKey:(NSString *)signerPublicKey
                  signatureBase64:(NSString *)signatureBase64
                  resolver:(RCTPromiseResolveBlock)resolve
                  rejecter:(RCTPromiseRejectBlock)reject)

RCT_EXTERN_METHOD(canUseSystemAuth:(RCTPromiseResolveBlock)resolve
                  rejecter:(RCTPromiseRejectBlock)reject)

RCT_EXTERN_METHOD(hasSystemAuthDomain:(RCTPromiseResolveBlock)resolve
                  rejecter:(RCTPromiseRejectBlock)reject)

RCT_EXTERN_METHOD(initializeSystemAuth:(NSString *)reason
                  resolver:(RCTPromiseResolveBlock)resolve
                  rejecter:(RCTPromiseRejectBlock)reject)

RCT_EXTERN_METHOD(registerSignerSystemAuth:(NSString *)envelopeJson
                  appPasscode:(NSString *)appPasscode
                  expectedSignerPublicKey:(NSString *)expectedSignerPublicKey
                  resolver:(RCTPromiseResolveBlock)resolve
                  rejecter:(RCTPromiseRejectBlock)reject)

RCT_EXTERN_METHOD(hasSignerSystemAuth:(NSString *)expectedSignerPublicKey
                  resolver:(RCTPromiseResolveBlock)resolve
                  rejecter:(RCTPromiseRejectBlock)reject)

RCT_EXTERN_METHOD(removeSignerSystemAuth:(NSString *)expectedSignerPublicKey
                  resolver:(RCTPromiseResolveBlock)resolve
                  rejecter:(RCTPromiseRejectBlock)reject)

RCT_EXTERN_METHOD(removeSystemAuthDomain:(RCTPromiseResolveBlock)resolve
                  rejecter:(RCTPromiseRejectBlock)reject)

RCT_EXTERN_METHOD(signWithSystemAuth:(NSString *)envelopeJson
                  expectedSignerPublicKey:(NSString *)expectedSignerPublicKey
                  transactionXdrBase64:(NSString *)transactionXdrBase64
                  networkPassphrase:(NSString *)networkPassphrase
                  reason:(NSString *)reason
                  resolver:(RCTPromiseResolveBlock)resolve
                  rejecter:(RCTPromiseRejectBlock)reject)

RCT_EXTERN_METHOD(signWithPasscode:(NSString *)envelopeJson
                  appPasscode:(NSString *)appPasscode
                  expectedSignerPublicKey:(NSString *)expectedSignerPublicKey
                  transactionXdrBase64:(NSString *)transactionXdrBase64
                  networkPassphrase:(NSString *)networkPassphrase
                  resolver:(RCTPromiseResolveBlock)resolve
                  rejecter:(RCTPromiseRejectBlock)reject)

@end
