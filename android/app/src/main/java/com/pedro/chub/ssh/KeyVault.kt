package com.pedro.chub.ssh

import android.content.Context
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.security.keystore.StrongBoxUnavailableException
import net.schmizz.sshj.common.Buffer
import net.schmizz.sshj.common.KeyType
import net.schmizz.sshj.userauth.keyprovider.KeyProvider
import net.schmizz.sshj.userauth.password.PasswordFinder
import net.schmizz.sshj.userauth.password.Resource
import org.json.JSONObject
import java.io.File
import java.security.KeyStore
import java.security.PublicKey
import java.util.Base64
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

/** Chave privada SSH com a senha dela (se tiver), já aberta. Só existe durante uma conexão. */
class Secret(val privateKey: String, val passphrase: String?) {
    fun keyProvider(): KeyProvider {
        val finder = object : PasswordFinder {
            override fun reqPassword(resource: Resource<*>?): CharArray = (passphrase ?: "").toCharArray()
            override fun shouldRetry(resource: Resource<*>?) = false
        }
        return net.schmizz.sshj.SSHClient().loadKeys(privateKey, null, finder)
    }
}

/**
 * Guarda a chave SSH importada (do Termux) cifrada com AES-GCM por uma chave do Keystore que só
 * funciona depois da digital, do rosto ou do PIN, a cada uso (`timeout 0`): abrir a chave pede a
 * biometria toda vez que o app conecta. A chave do Keystore não sai do chip (StrongBox quando há).
 */
class KeyVault(context: Context) {
    private val file = File(context.filesDir, "chave-ssh.bin")
    private val prefs = context.getSharedPreferences("chave", Context.MODE_PRIVATE)

    fun hasKey(): Boolean = file.exists()

    /** Chave pública no formato do `authorized_keys`, para copiar. */
    fun publicKey(): String? = prefs.getString("publica", null)

    private fun keystore(): KeyStore = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }

    private fun secretKey(create: Boolean): SecretKey {
        (keystore().getKey(ALIAS, null) as? SecretKey)?.let { return it }
        check(create) { "a chave do Keystore sumiu: importe a chave SSH de novo" }
        fun spec(strongBox: Boolean) = KeyGenParameterSpec.Builder(ALIAS, KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT)
            .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
            .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
            .setKeySize(256)
            .setUserAuthenticationRequired(true)
            .setUserAuthenticationParameters(0, KeyProperties.AUTH_BIOMETRIC_STRONG or KeyProperties.AUTH_DEVICE_CREDENTIAL)
            .setInvalidatedByBiometricEnrollment(false)
            .setIsStrongBoxBacked(strongBox)
            .build()
        val generator = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, "AndroidKeyStore")
        return try {
            generator.init(spec(true))
            generator.generateKey()
        } catch (_: StrongBoxUnavailableException) {
            generator.init(spec(false))
            generator.generateKey()
        }
    }

    /** Cifra para gravar uma chave nova; ainda precisa da biometria. */
    fun encryptCipher(): Cipher {
        // Chave nova a cada importação: a anterior deixa de abrir qualquer coisa.
        keystore().deleteEntry(ALIAS)
        return Cipher.getInstance(TRANSFORMATION).apply { init(Cipher.ENCRYPT_MODE, secretKey(true)) }
    }

    /** Cifra para abrir a chave gravada; ainda precisa da biometria. */
    fun decryptCipher(): Cipher {
        val data = file.readBytes()
        val ivLength = data[0].toInt()
        val iv = data.copyOfRange(1, 1 + ivLength)
        return Cipher.getInstance(TRANSFORMATION).apply {
            init(Cipher.DECRYPT_MODE, secretKey(false), GCMParameterSpec(128, iv))
        }
    }

    /** Grava a chave com a cifra já liberada pela biometria. */
    fun save(cipher: Cipher, secret: Secret, publicKey: String) {
        val json = JSONObject().put("chave", secret.privateKey).put("senha", secret.passphrase ?: "")
        val encrypted = cipher.doFinal(json.toString().toByteArray())
        val iv = cipher.iv
        val tmp = File(file.path + ".tmp")
        tmp.writeBytes(byteArrayOf(iv.size.toByte()) + iv + encrypted)
        tmp.renameTo(file)
        prefs.edit().putString("publica", publicKey).apply()
    }

    /** Abre a chave com a cifra já liberada pela biometria. */
    fun open(cipher: Cipher): Secret {
        val data = file.readBytes()
        val ivLength = data[0].toInt()
        val plain = cipher.doFinal(data, 1 + ivLength, data.size - 1 - ivLength)
        val json = JSONObject(String(plain))
        plain.fill(0)
        return Secret(json.getString("chave"), json.optString("senha").ifEmpty { null })
    }

    fun delete() {
        file.delete()
        prefs.edit().clear().apply()
        keystore().deleteEntry(ALIAS)
    }

    companion object {
        private const val ALIAS = "chub-chave-ssh"
        private const val TRANSFORMATION = "AES/GCM/NoPadding"

        /** Confere a chave importada (e a senha) e devolve a pública no formato do `authorized_keys`. */
        fun check(secret: Secret): String {
            val public = secret.keyProvider().public
            return "${KeyType.fromKey(public)} ${encode(public)} c-hub-android"
        }

        fun encode(key: PublicKey): String =
            Base64.getEncoder().encodeToString(Buffer.PlainBuffer().putPublicKey(key).compactData)

        /** Impressão digital como a do `ssh-keygen -l`: `SHA256:<base64 sem =>`. */
        fun fingerprint(key: PublicKey): String {
            val blob = Buffer.PlainBuffer().putPublicKey(key).compactData
            val digest = java.security.MessageDigest.getInstance("SHA-256").digest(blob)
            return "SHA256:" + Base64.getEncoder().withoutPadding().encodeToString(digest)
        }
    }
}
