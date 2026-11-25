package dev.massorca.app.keystore

import android.util.Base64
import com.facebook.react.bridge.Arguments
import com.facebook.react.bridge.ReactApplicationContext
import com.facebook.react.bridge.WritableArray
import dev.massorca.app.binding.NativeMassorcaKeystoreSpec
import dev.massorca.app.keystore.storage.AndroidKeystoreKey
import dev.massorca.app.keystore.storage.HostEntries
import dev.massorca.app.keystore.storage.HostSealer
import java.io.File

/**
 * Host entries in app-private storage, and seal/unseal with an AES-256-GCM key the Android Keystore holds (decision
 * 0080, section 3). Bytes cross the bridge as base64. The cipher and the files live in `storage`, which the JVM tests
 * run under Robolectric.
 */
class MassorcaKeystoreModule(context: ReactApplicationContext) : NativeMassorcaKeystoreSpec(context) {
  private val entries = HostEntries(File(context.filesDir, "hosts"))
  private val sealer = HostSealer(AndroidKeystoreKey::get)

  override fun getName(): String = NAME

  override fun readHost(name: String): String? = entries.read(name)?.let(::encode)

  override fun writeHost(name: String, entry: String) = entries.write(name, decode(entry))

  override fun removeHost(name: String): Boolean = entries.remove(name)

  override fun listHosts(): WritableArray {
    val names = Arguments.createArray()
    entries.list().forEach { names.pushString(it) }
    return names
  }

  override fun seal(plain: String): String = encode(sealer.seal(decode(plain)))

  override fun unseal(sealed: String): String = encode(sealer.unseal(decode(sealed)))

  private fun encode(bytes: ByteArray): String = Base64.encodeToString(bytes, Base64.NO_WRAP)

  private fun decode(text: String): ByteArray = Base64.decode(text, Base64.NO_WRAP)

  companion object {
    const val NAME = "MassorcaKeystore"
  }
}
