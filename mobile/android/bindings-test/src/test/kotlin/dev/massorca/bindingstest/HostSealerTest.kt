package dev.massorca.bindingstest

import dev.massorca.app.keystore.storage.HostEntries
import dev.massorca.app.keystore.storage.HostSealer
import java.io.File
import java.security.GeneralSecurityException
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertThrows
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config

/** The Keystore module's cipher and files under Robolectric, with a software key where the phone has the Keystore. */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
class HostSealerTest {
  private fun softwareKey(): SecretKey =
    KeyGenerator.getInstance("AES").apply { init(256) }.generateKey()

  @Test
  fun aSealedValueUnsealsToWhatWasSealedAndIsNotThePlainBytes() {
    val key = softwareKey()
    val sealer = HostSealer { key }
    val plain = "-----BEGIN PRIVATE KEY-----".toByteArray()
    val sealed = sealer.seal(plain)
    assertFalse(String(sealed, Charsets.ISO_8859_1).contains("PRIVATE KEY"))
    assertArrayEquals(plain, sealer.unseal(sealed))
    assertFalse("a fresh IV each time", sealed.contentEquals(sealer.seal(plain)))
  }

  @Test
  fun anotherKeyAndAChangedValueAreRefused() {
    val key = softwareKey()
    val sealed = HostSealer { key }.seal("secret".toByteArray())
    assertThrows(GeneralSecurityException::class.java) { HostSealer { softwareKey() }.unseal(sealed) }
    val flipped = sealed.copyOf().also { it[it.size - 1] = (it[it.size - 1].toInt() xor 1).toByte() }
    assertThrows(GeneralSecurityException::class.java) { HostSealer { key }.unseal(flipped) }
    assertThrows(IllegalArgumentException::class.java) { HostSealer { key }.unseal(ByteArray(12)) }
  }

  @Test
  fun hostEntriesRoundTripInTheAppsPrivateFolder() {
    val dir = File(RuntimeEnvironment.getApplication().filesDir, "hosts")
    val entries = HostEntries(dir)
    assertNull(entries.read("office"))
    entries.write("office", byteArrayOf(1, 2, 3))
    entries.write("home", byteArrayOf(4))
    assertArrayEquals(byteArrayOf(1, 2, 3), entries.read("office"))
    assertEquals(listOf("home", "office"), entries.list())
    entries.write("office", byteArrayOf(9))
    assertArrayEquals(byteArrayOf(9), entries.read("office"))
    assertTrue(entries.remove("office"))
    assertFalse(entries.remove("office"))
    assertEquals(listOf("home"), entries.list())
    assertThrows(IllegalArgumentException::class.java) { entries.read("../escape") }
  }
}
