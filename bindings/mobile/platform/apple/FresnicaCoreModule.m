#import <React/RCTBridgeModule.h>

// Swift implementation lives in FresnicaCoreModule.swift. RCT_EXTERN_MODULE keeps the
// Xaman-compatible Objective-C React Native bridge thin and does not expose native key material.
@interface RCT_EXTERN_MODULE(FresnicaCoreModule, NSObject)

RCT_EXTERN_METHOD(canEnrollSystemAuth:(RCTPromiseResolveBlock)resolve
                  rejecter:(RCTPromiseRejectBlock)reject)

RCT_EXTERN_METHOD(hasSystemAuth:(NSString *)expectedSignerPublicKey
                  resolver:(RCTPromiseResolveBlock)resolve
                  rejecter:(RCTPromiseRejectBlock)reject)

RCT_EXTERN_METHOD(removeSystemAuth:(NSString *)expectedSignerPublicKey
                  resolver:(RCTPromiseResolveBlock)resolve
                  rejecter:(RCTPromiseRejectBlock)reject)

RCT_EXTERN_METHOD(enrollSystemAuth:(NSString *)envelopeJson
                  appPasscode:(NSString *)appPasscode
                  expectedSignerPublicKey:(NSString *)expectedSignerPublicKey
                  resolver:(RCTPromiseResolveBlock)resolve
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
