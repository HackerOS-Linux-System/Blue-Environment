package org.hackeros.blueconnect.android

import android.Manifest
import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import androidx.core.content.ContextCompat
import org.legendaryos.blueconnect.R

/** Notification channels and the few notifications Blue Connect posts. */
object Notifications {
    const val CHANNEL_SERVICE = "service"
    const val CHANNEL_PAIRING = "pairing"
    const val CHANNEL_PING = "ping"

    const val ID_SERVICE = 1
    const val ID_PAIRING = 2
    private var nextPingId = 100

    fun createChannels(c: Context) {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) return
        val nm = c.getSystemService(NotificationManager::class.java)
        nm.createNotificationChannel(NotificationChannel(CHANNEL_SERVICE, c.getString(R.string.channel_service), NotificationManager.IMPORTANCE_LOW))
        nm.createNotificationChannel(NotificationChannel(CHANNEL_PAIRING, c.getString(R.string.channel_pairing), NotificationManager.IMPORTANCE_HIGH))
        nm.createNotificationChannel(NotificationChannel(CHANNEL_PING, c.getString(R.string.channel_ping), NotificationManager.IMPORTANCE_DEFAULT))
    }

    private fun openApp(c: Context): PendingIntent =
        PendingIntent.getActivity(
            c, 0, Intent(c, MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP),
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )

    fun serviceNotification(c: Context): Notification {
        val stop = PendingIntent.getService(
            c, 1, Intent(c, ConnectService::class.java).setAction(ConnectService.ACTION_STOP),
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )
        return NotificationCompat.Builder(c, CHANNEL_SERVICE)
            .setSmallIcon(R.drawable.ic_stat_blue)
            .setContentTitle(c.getString(R.string.notif_service_title))
            .setContentText(c.getString(R.string.notif_service_text))
            .setContentIntent(openApp(c))
            .addAction(0, "Stop", stop)
            .setOngoing(true)
            .setOnlyAlertOnce(true)
            .build()
    }

    private fun canPost(c: Context): Boolean =
        Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU ||
            ContextCompat.checkSelfPermission(c, Manifest.permission.POST_NOTIFICATIONS) == PackageManager.PERMISSION_GRANTED

    private fun post(c: Context, id: Int, n: Notification) {
        if (!canPost(c)) return
        try { NotificationManagerCompat.from(c).notify(id, n) } catch (e: SecurityException) { /* permission revoked meanwhile */ }
    }

    fun pairingRequest(c: Context, deviceName: String, sas: String) {
        post(
            c, ID_PAIRING,
            NotificationCompat.Builder(c, CHANNEL_PAIRING)
                .setSmallIcon(R.drawable.ic_stat_blue)
                .setContentTitle("Pairing request from $deviceName")
                .setContentText("Code ${sas.take(3)} ${sas.drop(3)} — open Blue Connect to compare and accept")
                .setContentIntent(openApp(c))
                .setAutoCancel(true)
                .setCategory(NotificationCompat.CATEGORY_MESSAGE)
                .build(),
        )
    }

    fun clearPairingRequest(c: Context) {
        try { NotificationManagerCompat.from(c).cancel(ID_PAIRING) } catch (e: SecurityException) { /* ignore */ }
    }

    fun ping(c: Context, deviceName: String, message: String?) {
        post(
            c, nextPingId++,
            NotificationCompat.Builder(c, CHANNEL_PING)
                .setSmallIcon(R.drawable.ic_stat_blue)
                .setContentTitle("Ping from $deviceName")
                .setContentText(message?.takeIf { it.isNotBlank() } ?: "Ping!")
                .setContentIntent(openApp(c))
                .setAutoCancel(true)
                .build(),
        )
    }
}
