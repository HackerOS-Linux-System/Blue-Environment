package org.hackeros.blueconnect.core

import org.json.JSONObject
import java.io.File

/**
 * A device we have seen or paired with. Field names/JSON shape are identical
 * to the desktop's `DiscoveredDevice`, so a devices.json is interchangeable
 * between implementations.
 */
data class DeviceInfo(
    val id: String,
    val name: String,
    val deviceType: String = "unknown",
    val address: String = "",
    val tcpPort: Int = Protocol.PAIRING_TCP_PORT,
    val paired: Boolean = false,
    val pinnedCertSha256: String? = null,
)

/** Thread-safe, persisted registry of known devices (`devices.json`). */
class DeviceStore(dir: File) {
    private val lock = Any()
    private val file = File(dir, "devices.json")
    private val devices = LinkedHashMap<String, DeviceInfo>()

    init {
        dir.mkdirs()
        load()
    }

    private fun load() {
        try {
            if (!file.exists()) return
            val root = JSONObject(file.readText())
            for (key in root.keys()) {
                val o = root.optJSONObject(key) ?: continue
                val id = o.optString("id", key)
                devices[id] = DeviceInfo(
                    id = id,
                    name = o.optString("name", id),
                    deviceType = Protocol.normalizeDeviceType(o.optString("deviceType", "unknown")),
                    address = o.optString("address", ""),
                    tcpPort = o.optInt("tcpPort", Protocol.PAIRING_TCP_PORT),
                    paired = o.optBoolean("paired", false),
                    // JSON null (desktop writes it for never-paired devices) must become Kotlin null.
                    pinnedCertSha256 = if (o.isNull("pinnedCertSha256")) null else o.optString("pinnedCertSha256"),
                )
            }
        } catch (e: Exception) {
            devices.clear() // unreadable file: start empty rather than crash on startup
        }
    }

    private fun save() {
        val root = JSONObject()
        for ((id, d) in devices) {
            root.put(
                id,
                JSONObject()
                    .put("id", d.id).put("name", d.name).put("deviceType", d.deviceType)
                    .put("address", d.address).put("tcpPort", d.tcpPort).put("paired", d.paired)
                    .put("pinnedCertSha256", d.pinnedCertSha256 ?: JSONObject.NULL),
            )
        }
        val tmp = File(file.parentFile, "devices.json.tmp")
        tmp.writeText(root.toString(2))
        if (!tmp.renameTo(file)) { // some filesystems refuse to rename over an existing file
            file.delete()
            tmp.renameTo(file)
        }
    }

    fun get(id: String): DeviceInfo? = synchronized(lock) { devices[id] }

    fun findPairedByFingerprint(fp: String): DeviceInfo? =
        synchronized(lock) { devices.values.firstOrNull { it.paired && it.pinnedCertSha256 == fp } }

    /** Paired devices first, then by name — same order as the desktop's list. */
    fun all(): List<DeviceInfo> = synchronized(lock) {
        devices.values.sortedWith(compareByDescending<DeviceInfo> { it.paired }.thenBy { it.name })
    }

    /** Records what a discovery datagram told us; never touches pairing state. */
    fun seen(id: String, name: String, type: String, address: String, tcpPort: Int) = synchronized(lock) {
        val old = devices[id]
        devices[id] = old?.copy(name = name, deviceType = type, address = address, tcpPort = tcpPort)
            ?: DeviceInfo(id, name, type, address, tcpPort)
        save()
    }

    /** Marks a device paired and pins the certificate it presented. */
    fun markPaired(id: String, fallbackName: String, address: String, tcpPort: Int, fingerprint: String) = synchronized(lock) {
        val old = devices[id] ?: DeviceInfo(id, fallbackName, "unknown", address, tcpPort)
        devices[id] = old.copy(paired = true, pinnedCertSha256 = fingerprint)
        save()
    }

    fun updateAddress(id: String, address: String) = synchronized(lock) {
        val d = devices[id]
        if (d != null && d.address != address) {
            devices[id] = d.copy(address = address)
            save()
        }
    }

    fun forget(id: String): Boolean = synchronized(lock) {
        val removed = devices.remove(id) != null
        if (removed) save()
        removed
    }
}
