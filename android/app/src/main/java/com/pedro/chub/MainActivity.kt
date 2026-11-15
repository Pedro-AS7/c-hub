package com.pedro.chub

import android.app.Application
import android.os.Bundle
import androidx.activity.compose.BackHandler
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.viewModels
import androidx.biometric.BiometricManager.Authenticators.BIOMETRIC_STRONG
import androidx.biometric.BiometricManager.Authenticators.DEVICE_CREDENTIAL
import androidx.biometric.BiometricPrompt
import androidx.core.content.ContextCompat
import androidx.fragment.app.FragmentActivity
import com.pedro.chub.ui.App
import kotlinx.coroutines.suspendCancellableCoroutine
import java.security.Security
import javax.crypto.Cipher
import kotlin.coroutines.resume

class ChubApp : Application() {
    override fun onCreate() {
        super.onCreate()
        // O BouncyCastle do Android é reduzido (sem Ed25519 para o sshj): troca pelo completo.
        Security.removeProvider("BC")
        Security.insertProviderAt(org.bouncycastle.jce.provider.BouncyCastleProvider(), 1)
    }
}

class MainActivity : FragmentActivity() {
    private val model: MainViewModel by viewModels()

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        model.authenticator = ::authenticate
        setContent {
            // Ícones da barra de status escuros num tema claro (Catppuccin Latte, personalizado claro).
            val light = com.pedro.chub.ui.C.theme.light
            androidx.compose.runtime.LaunchedEffect(light) {
                val transparent = android.graphics.Color.TRANSPARENT
                val style = if (light) androidx.activity.SystemBarStyle.light(transparent, transparent)
                else androidx.activity.SystemBarStyle.dark(transparent)
                enableEdgeToEdge(style, style)
            }
            App(model)
            BackHandler(enabled = model.screen != Screen.Home) { model.screen = Screen.Home }
        }
    }

    override fun onResume() {
        super.onResume()
        // Ao voltar ao app, reconecta o que caiu (uma biometria para todos).
        if (model.hasKey && model.conn.values.none { it == Conn.Connecting }) {
            val down = model.hosts.any { model.conn[it.id] !is Conn.On }
            if (down && model.hostKeyAsk == null) model.connect()
        }
    }

    override fun onStop() {
        // Em segundo plano, o celular sai dos tmux (veja MainViewModel.release); a tela do chat
        // reconecta ao voltar.
        model.releaseAll()
        super.onStop()
    }

    override fun onDestroy() {
        if (model.authenticator == ::authenticate) model.authenticator = null
        super.onDestroy()
    }

    /** Pede a digital, o rosto ou o PIN e devolve a cifra liberada (ou null se cancelado). */
    private suspend fun authenticate(cipher: Cipher, title: String): Cipher? = suspendCancellableCoroutine { continuation ->
        val prompt = BiometricPrompt(this, ContextCompat.getMainExecutor(this), object : BiometricPrompt.AuthenticationCallback() {
            override fun onAuthenticationSucceeded(result: BiometricPrompt.AuthenticationResult) {
                if (continuation.isActive) continuation.resume(result.cryptoObject?.cipher)
            }

            override fun onAuthenticationError(errorCode: Int, errString: CharSequence) {
                if (continuation.isActive) continuation.resume(null)
            }
        })
        val info = BiometricPrompt.PromptInfo.Builder()
            .setTitle(title)
            .setSubtitle("chave SSH do C-hub")
            .setAllowedAuthenticators(BIOMETRIC_STRONG or DEVICE_CREDENTIAL)
            .build()
        prompt.authenticate(info, BiometricPrompt.CryptoObject(cipher))
        continuation.invokeOnCancellation { prompt.cancelAuthentication() }
    }
}
