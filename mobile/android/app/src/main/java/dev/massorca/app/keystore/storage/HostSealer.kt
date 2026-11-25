package dev.massorca.app.keystore.storage

import javax.crypto.Cipher
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

/**
 * AES-256-GCM sealing of the device key (decision 0080, section 3). The key comes from [key], the Android Keystore on a
 * phone and a software key in the JVM tests, since Robolectric has no AndroidKeyStore provider. A sealed value is the
 * 12-byte IV followed by the ciphertext and tag.
 */
class HostSealer(private val key: () -> SecretKey) {
  fun seal(plain: ByteArray): ByteArray {
    val cipher = Cipher.getInstance(TRANSFORMATION)
    cipher.init(Cipher.ENCRYPT_MODE, key())
    return cipher.iv + cipher.doFinal(plain)
  }

  fun unseal(sealed: ByteArray): ByteArray {
    require(sealed.size > IV_BYTES) { "the sealed value is too short" }
    val cipher = Cipher.getInstance(TRANSFORMATION)
    cipher.init(Cipher.DECRYPT_MODE, key(), GCMParameterSpec(TAG_BITS, sealed, 0, IV_BYTES))
    return cipher.doFinal(sealed, IV_BYTES, sealed.size - IV_BYTES)
  }

  private companion object {
    const val TRANSFORMATION = "AES/GCM/NoPadding"
    const val IV_BYTES = 12
    const val TAG_BITS = 128
  }
}
