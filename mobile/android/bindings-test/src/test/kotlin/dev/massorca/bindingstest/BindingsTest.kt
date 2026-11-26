package dev.massorca.bindingstest

import dev.massorca.app.keystore.storage.HostEntries
import dev.massorca.app.keystore.storage.HostSealer
import java.io.File
import java.nio.file.Files
import java.util.concurrent.TimeUnit
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Assert.fail
import org.junit.Before
import org.junit.Test
import uniffi.massorca_client_ffi.FfiException
import uniffi.massorca_client_ffi.HostStore
import uniffi.massorca_client_ffi.MassOrca
import uniffi.massorca_client_ffi.StorageException
import uniffi.massorca_client_ffi.parseOffer

/**
 * The generated Kotlin API against the host's own build of the Rust library and a real server that
 * `scripts/ci/kotlin-bindings-test.sh` starts on this machine (decision 0080, task A6). The app's storage is the
 * Keystore module's plain classes with a software key.
 */
class BindingsTest {
  private lateinit var dir: File

  @Before
  fun makeFolder() {
    dir = Files.createTempDirectory("massorca-kt").toFile()
  }

  @After
  fun removeFolder() {
    dir.deleteRecursively()
  }

  private fun softwareKey(): SecretKey =
    KeyGenerator.getInstance("AES").apply { init(256) }.generateKey()

  /** The app's store as the Kotlin side implements it: failures reach Rust as `StorageException`. */
  private class SealedFileStore(private val entries: HostEntries, private val sealer: HostSealer) : HostStore {
    private fun <T> guarded(block: () -> T): T =
      try {
        block()
      } catch (error: Exception) {
        throw StorageException.Failed(error.message ?: error.javaClass.simpleName)
      }

    override fun readHost(name: String): ByteArray? = guarded { entries.read(name) }

    override fun writeHost(name: String, entry: ByteArray) = guarded { entries.write(name, entry) }

    override fun removeHost(name: String): Boolean = guarded { entries.remove(name) }

    override fun listHosts(): List<String> = guarded { entries.list() }

    override fun seal(plain: ByteArray): ByteArray = guarded { sealer.seal(plain) }

    override fun unseal(sealed: ByteArray): ByteArray = guarded { sealer.unseal(sealed) }
  }

  private fun app(key: SecretKey): MassOrca =
    MassOrca(SealedFileStore(HostEntries(File(dir, "hosts")), HostSealer { key }), "phone", "0.0.0-test")

  /** A pairing offer of the server under test, made by its owner with the CLI. It works once. */
  private fun newOffer(): String {
    val bin = System.getenv("MASSORCA_BIN")
    assertNotNull("MASSORCA_BIN names the massorca program", bin)
    val process = ProcessBuilder(bin, "pair", "offer").redirectErrorStream(true).start()
    val output = process.inputStream.bufferedReader().readText()
    assertTrue("pair offer timed out", process.waitFor(30, TimeUnit.SECONDS))
    assertEquals("pair offer failed", 0, process.exitValue())
    return output.lineSequence().first().trim()
  }

  @Test
  fun thePhonePairsConnectsAndReadsWhoItIsThroughTheGeneratedApi() {
    val offer = newOffer()
    val summary = parseOffer(offer)
    assertFalse(summary.server.isEmpty())
    val key = softwareKey()
    val massorca = app(key)
    val paired = runBlocking { withTimeout(60_000) { massorca.pair(offer, "office") } }
    assertEquals("office", paired.name)
    assertEquals(summary.server, paired.server)
    assertEquals("phone", paired.deviceName)
    assertEquals(listOf("office"), massorca.hosts().map { it.name })

    val host = runBlocking { withTimeout(60_000) { massorca.connect("office") } }
    val me = runBlocking { withTimeout(60_000) { host.whoami() } }
    assertEquals("phone", me.deviceName)
    assertNotNull(me.deviceId)
    assertTrue(host.protocol().protocol > 0u)
    host.close()
    massorca.close()

    // The key is stored only sealed.
    val stored = File(dir, "hosts/office").readBytes().toString(Charsets.ISO_8859_1)
    assertFalse("the device key is stored in the clear", stored.contains("PRIVATE KEY"))

    // A sealed key round-trips: a new binding over the same store and Keystore key connects as the same device.
    val again = app(key)
    val reconnected = runBlocking { withTimeout(60_000) { again.connect("office") } }
    assertEquals(me.deviceId, runBlocking { withTimeout(60_000) { reconnected.whoami() } }.deviceId)
    reconnected.close()
    again.close()
  }

  @Test
  fun aWrongKeyFailsWithStorageAndTheRightKeyStillWorks() {
    val offer = newOffer()
    val key = softwareKey()
    val massorca = app(key)
    runBlocking { withTimeout(60_000) { massorca.pair(offer, "office") } }
    massorca.close()

    val wrong = app(softwareKey())
    assertEquals("listing needs no key", listOf("office"), wrong.hosts().map { it.name })
    try {
      runBlocking { withTimeout(60_000) { wrong.connect("office") } }
      fail("connecting with another Keystore key must fail")
    } catch (error: FfiException) {
      assertTrue("expected Storage, got $error", error is FfiException.Storage)
    }
    wrong.close()

    val right = app(key)
    runBlocking { withTimeout(60_000) { right.connect("office") } }.close()
    right.close()
  }
}
