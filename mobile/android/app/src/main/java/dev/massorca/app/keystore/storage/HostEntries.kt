package dev.massorca.app.keystore.storage

import java.io.File

/** Host entries in one folder, one file per host. The names are the ones the Rust store validates. */
class HostEntries(private val dir: File) {
  fun read(name: String): ByteArray? {
    val file = entryFile(name)
    return if (file.isFile) file.readBytes() else null
  }

  fun write(name: String, entry: ByteArray) {
    val target = entryFile(name)
    dir.mkdirs()
    // Written beside the target and renamed, so a crash leaves the old entry or the new one, never half of each.
    val partial = File(dir, name + PARTIAL)
    partial.writeBytes(entry)
    if (!partial.renameTo(target)) {
      partial.delete()
      throw IllegalStateException("the host entry could not be saved")
    }
  }

  fun remove(name: String): Boolean = entryFile(name).delete()

  fun list(): List<String> = dir.list().orEmpty().filterNot { it.endsWith(PARTIAL) }.sorted()

  private fun entryFile(name: String): File {
    require(HOST_NAME.matches(name)) { "the host name is not a valid file name" }
    return File(dir, name)
  }

  private companion object {
    const val PARTIAL = ".partial"
    val HOST_NAME = Regex("[a-z0-9][a-z0-9_-]{0,31}")
  }
}
