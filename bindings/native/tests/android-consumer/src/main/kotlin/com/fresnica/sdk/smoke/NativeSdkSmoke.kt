package com.fresnica.sdk.smoke

import android.content.Context
import com.fresnica.sdk.FresnicaSdkApi
import com.fresnica.sdk.security.FresnicaSignerAuthorization
import com.fresnica.sdk.security.WalletUnlockKeyStore

/** Compile-only proof that a standalone Android host can consume the raw Native SDK AAR. */
class NativeSdkSmoke(context: Context) {
    private val api = FresnicaSdkApi()
    private val keyStore = WalletUnlockKeyStore(context)
    private val authorization = FresnicaSignerAuthorization(keyStore, api)

    fun sdkApi(): FresnicaSdkApi = api

    fun signerAuthorization(): FresnicaSignerAuthorization = authorization
}
