package org.hackeros.blueconnect.core

import java.io.File
import java.net.InetAddress

class ConnectOptions(
    /** Folder for device_id and devices.json. */
    val dataDir: File,
    val deviceName: String,
    /** phone | tablet | desktop | laptop | tv */
    val deviceType: String = "phone",
    /** UDP discovery port (KDE Connect's real port by default). */
    val discoveryPort: Int = Protocol.DISCOVERY_PORT,
    /** TCP port of the mutual-TLS listener; advertised as tcpPort in our identity. */
    val tcpPort: Int = Protocol.PAIRING_TCP_PORT,
    /** UDP port scans send to. Null = [discoveryPort]. Only tests need to differ. */
    val scanTargetPort: Int? = null,
    /** Where scans send the identity datagram. Null = broadcast + every interface's directed broadcast. */
    val discoveryTargets: List<InetAddress>? = null,
    val bindAddress: InetAddress = InetAddress.getByName("0.0.0.0"),
)
