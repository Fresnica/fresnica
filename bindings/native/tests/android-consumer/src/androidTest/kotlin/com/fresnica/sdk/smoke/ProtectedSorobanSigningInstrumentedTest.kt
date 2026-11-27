package com.fresnica.sdk.smoke

import android.content.Context
import android.util.Base64
import androidx.test.core.app.ApplicationProvider
import androidx.test.ext.junit.runners.AndroidJUnit4
import com.fresnica.sdk.FresnicaSdkApi
import com.fresnica.sdk.security.FresnicaSignerAuthorization
import com.fresnica.sdk.security.WalletUnlockKeyStore
import org.json.JSONObject
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import java.util.Arrays

@RunWith(AndroidJUnit4::class)
class ProtectedSorobanSigningInstrumentedTest {
    private val context: Context = ApplicationProvider.getApplicationContext()

    @Test
    fun releasedNativeSdkSignsLiveBlendTransactionWithPasscode() {
        val fixture = assetJson("protected-soroban-signing-v1.json")
        val signing = assetJson("transaction-signing-v1.json")
        val signer = signing.getJSONArray("cases").getJSONObject(0)
        val transaction = fixture.getJSONObject("transaction")
        val publicKey = fixture.getString("source_account")
        val passcode = "test-passphrase"
        val unsigned = Base64.decode(transaction.getString("unsigned_xdr_base64"), Base64.DEFAULT)
        val original = unsigned.copyOf()
        val expectedSigned =
            Base64.decode(transaction.getString("expected_signed_xdr_base64"), Base64.DEFAULT)

        val api = FresnicaSdkApi()
        val protectedSigner =
            api.protectSecret(signer.getString("secret"), passcode, publicKey)
        val authorization =
            FresnicaSignerAuthorization(WalletUnlockKeyStore(context), api)

        val signed =
            authorization.signWithPasscode(
                protectedSigner.envelopeJson,
                passcode,
                publicKey,
                unsigned,
                fixture.getString("network_passphrase"),
            )

        assertArrayEquals(expectedSigned, signed)
        assertArrayEquals(original, unsigned)

        val wrongNetworkSigned =
            authorization.signWithPasscode(
                protectedSigner.envelopeJson,
                passcode,
                publicKey,
                unsigned,
                fixture.getString("wrong_network_passphrase"),
            )
        assertFalse(Arrays.equals(expectedSigned, wrongNetworkSigned))
        assertArrayEquals(original, unsigned)

        val wrongIdentity =
            runCatching {
                authorization.signWithPasscode(
                    protectedSigner.envelopeJson,
                    passcode,
                    fixture.getString("wrong_signer_public_key"),
                    unsigned,
                    fixture.getString("network_passphrase"),
                )
            }
        assertTrue(wrongIdentity.isFailure)
        assertArrayEquals(original, unsigned)
    }

    private fun assetJson(name: String): JSONObject =
        context.assets.open(name).bufferedReader().use { JSONObject(it.readText()) }
}
