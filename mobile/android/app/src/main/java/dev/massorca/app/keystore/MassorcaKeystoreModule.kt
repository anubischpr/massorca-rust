package dev.massorca.app.keystore

import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.Base64
import com.facebook.react.bridge.Arguments
import com.facebook.react.bridge.ReactApplicationContext
import com.facebook.react.bridge.WritableArray
import dev.massorca.app.binding.NativeMassorcaKeystoreSpec
import java.io.File
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

/**
 * Host entries in app-private storage, and seal/unseal with an AES-256-GCM key the Android Keystore holds (decision
 * 0080, section 3). Bytes cross the bridge as base64. The names are the ones the Rust store validates.
 */
class MassorcaKeystoreModule(context: ReactApplicationContext) : NativeMassorcaKeystoreSpec(context) {
  private val hostsDir = File(context.filesDir, "hosts")

  override fun getName(): String = NAME

  override fun readHost(name: String): String? {
    val file = entryFile(name)
    return if (file.isFile) Base64.encodeToString(file.readBytes(), Base64.NO_WRAP) else null
  }

  override fun writeHost(name: String, entry: String) {
    val target = entryFile(name)
    hostsDir.mkdirs()
    // Written beside the target and renamed, so a crash leaves the old entry or the new one, never half of each.
    val partial = File(hostsDir, name + PARTIAL)
    partial.writeBytes(Base64.decode(entry, Base64.NO_WRAP))
    if (!partial.renameTo(target)) {
      partial.delete()
      throw IllegalStateException("the host entry could not be saved")
    }
  }

  override fun removeHost(name: String): Boolean = entryFile(name).delete()

  override fun listHosts(): WritableArray {
    val names = Arguments.createArray()
    hostsDir.list().orEmpty()
      .filterNot { it.endsWith(PARTIAL) }
      .sorted()
      .forEach { names.pushString(it) }
    return names
  }

  override fun seal(plain: String): String {
    val cipher = Cipher.getInstance(TRANSFORMATION)
    cipher.init(Cipher.ENCRYPT_MODE, key())
    val sealed = cipher.iv + cipher.doFinal(Base64.decode(plain, Base64.NO_WRAP))
    return Base64.encodeToString(sealed, Base64.NO_WRAP)
  }

  override fun unseal(sealed: String): String {
    val bytes = Base64.decode(sealed, Base64.NO_WRAP)
    require(bytes.size > IV_BYTES) { "the sealed value is too short" }
    val cipher = Cipher.getInstance(TRANSFORMATION)
    cipher.init(Cipher.DECRYPT_MODE, key(), GCMParameterSpec(TAG_BITS, bytes, 0, IV_BYTES))
    val plain = cipher.doFinal(bytes, IV_BYTES, bytes.size - IV_BYTES)
    return Base64.encodeToString(plain, Base64.NO_WRAP)
  }

  private fun entryFile(name: String): File {
    require(HOST_NAME.matches(name)) { "the host name is not a valid file name" }
    return File(hostsDir, name)
  }

  private fun key(): SecretKey {
    val store = KeyStore.getInstance(ANDROID_KEYSTORE).apply { load(null) }
    (store.getKey(KEY_ALIAS, null) as? SecretKey)?.let { return it }
    val spec = KeyGenParameterSpec.Builder(
      KEY_ALIAS,
      KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT,
    )
      .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
      .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
      .setKeySize(KEY_BITS)
      .build()
    return KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, ANDROID_KEYSTORE)
      .apply { init(spec) }
      .generateKey()
  }

  companion object {
    const val NAME = "MassorcaKeystore"
    private const val ANDROID_KEYSTORE = "AndroidKeyStore"
    private const val KEY_ALIAS = "dev.massorca.app.host-store"
    private const val KEY_BITS = 256
    private const val TRANSFORMATION = "AES/GCM/NoPadding"
    private const val IV_BYTES = 12
    private const val TAG_BITS = 128
    private const val PARTIAL = ".partial"
    private val HOST_NAME = Regex("[a-z0-9][a-z0-9_-]{0,31}")
  }
}
