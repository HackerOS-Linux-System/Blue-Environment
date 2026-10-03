package org.hackeros.blueconnect.android

import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.net.wifi.WifiManager
import android.os.Build
import android.os.IBinder
import android.provider.Settings
import android.util.Log
import androidx.core.app.ServiceCompat
import androidx.core.content.ContextCompat
import org.legendaryos.blueconnect.core.ConnectEngine
import org.legendaryos.blueconnect.core.ConnectListener
import org.legendaryos.blueconnect.core.ConnectOptions
import org.legendaryos.blueconnect.core.DeviceInfo
import org.legendaryos.blueconnect.core.IncomingPairingRequest
import org.legendaryos.blueconnect.core.OutgoingPairingSas
import org.legendaryos.blueconnect.core.PairingDecision
import java.io.File

/**
 * Foreground service that owns the [ConnectEngine] — keeps the UDP/TLS
 * listeners alive while the app is in the background so the desktop can find
 * and pair with this phone at any time (and ask it for SMS).
 */
class ConnectService : Service() {

    companion object {
        const val ACTION_STOP = "org.legendaryos.blueconnect.STOP"
        private const val TAG = "BlueConnect"

        fun start(context: Context) {
            ContextCompat.startForegroundService(context, Intent(context, ConnectService::class.java))
        }

        fun stop(context: Context) {
            context.stopService(Intent(context, ConnectService::class.java))
        }
    }

    private var engine: ConnectEngine? = null
    private var multicastLock: WifiManager.MulticastLock? = null

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onCreate() {
        super.onCreate()
        Notifications.createChannels(this)
        val type = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) ServiceInfo.FOREGROUND_SERVICE_TYPE_CONNECTED_DEVICE else 0
        ServiceCompat.startForeground(this, Notifications.ID_SERVICE, Notifications.serviceNotification(this), type)
        Thread({ boot() }, "blue-connect-boot").start()
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        if (intent?.action == ACTION_STOP) {
            stopSelf()
            return START_NOT_STICKY
        }
        return START_STICKY
    }

    private fun deviceName(): String =
        (Settings.Global.getString(contentResolver, "device_name")?.takeIf { it.isNotBlank() } ?: Build.MODEL ?: "Android")

    private fun deviceType(): String =
        if (resources.configuration.smallestScreenWidthDp >= 600) "tablet" else "phone"

    private fun boot() {
        try {
            // Some Wi-Fi drivers drop incoming broadcast datagrams unless a multicast lock is held.
            val wifi = applicationContext.getSystemService(Context.WIFI_SERVICE) as? WifiManager
            multicastLock = wifi?.createMulticastLock("blue-connect")?.apply { setReferenceCounted(false); acquire() }

            val identity = AndroidKeystoreIdentity.loadOrCreate()
            val options = ConnectOptions(dataDir = File(filesDir, "blue-connect"), deviceName = deviceName(), deviceType = deviceType())
            val e = ConnectEngine(options, identity, UiListener())
            e.packetHandler = SmsPlugin(applicationContext) { msg -> Log.i(TAG, msg); BlueConnectRuntime.postMessage(msg) }
            e.start()
            engine = e
            BlueConnectRuntime.attach(e)
        } catch (ex: Exception) {
            Log.e(TAG, "Could not start Blue Connect", ex)
            BlueConnectRuntime.postMessage(
                "Could not start Blue Connect: ${ex.message}. Is another app (e.g. KDE Connect) using TCP/UDP 1716–1717?",
            )
            stopSelf()
        }
    }

    override fun onDestroy() {
        engine?.stop()
        engine = null
        try { multicastLock?.takeIf { it.isHeld }?.release() } catch (e: Exception) { /* ignore */ }
        BlueConnectRuntime.detach()
        super.onDestroy()
    }

    private inner class UiListener : ConnectListener {
        override fun onDevicesChanged() = BlueConnectRuntime.refreshDevices()

        override fun onIncomingPairing(request: IncomingPairingRequest, decision: PairingDecision) {
            BlueConnectRuntime.showIncoming(request, decision)
            Notifications.pairingRequest(this@ConnectService, request.deviceName, request.sas)
        }

        override fun onOutgoingPairingSas(info: OutgoingPairingSas) = BlueConnectRuntime.showOutgoing(info)

        override fun onPing(device: DeviceInfo, message: String?) = Notifications.ping(this@ConnectService, device.name, message)

        override fun onLog(message: String) {
            Log.i(TAG, message)
            BlueConnectRuntime.postMessage(message)
        }
    }
}
