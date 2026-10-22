package org.hackeros.blueconnect.android

import android.content.ComponentName
import android.content.Context
import android.provider.Settings
import android.service.notification.NotificationListenerService
import android.service.notification.StatusBarNotification
import androidx.core.app.NotificationCompat
import org.hackeros.blueconnect.core.NotificationInfo
import java.util.concurrent.ConcurrentHashMap

/**
 * Keeps an in-memory list of this phone's current notifications so a paired computer can ask
 * for them (`kdeconnect.notification.request`). Nothing is stored on disk or sent unprompted.
 * It only works after the person enabled "Notification access" for Blue Connect in system
 * settings — the app shows a button for that.
 */
class BlueNotificationListener : NotificationListenerService() {

    companion object {
        private const val MAX = 50
        private val current = ConcurrentHashMap<String, NotificationInfo>()

        /** Newest first. */
        fun snapshot(): List<NotificationInfo> = current.values.sortedByDescending { it.timeMs }.take(MAX)

        fun isEnabled(c: Context): Boolean {
            val flat = Settings.Secure.getString(c.contentResolver, "enabled_notification_listeners") ?: return false
            val me = ComponentName(c, BlueNotificationListener::class.java)
            return flat.split(':').any { ComponentName.unflattenFromString(it) == me }
        }

        /** Our own notifications and silent ongoing ones (media players, services) are not interesting. */
        internal fun toInfo(sbn: StatusBarNotification, ownPackage: String, appLabel: String): NotificationInfo? {
            if (sbn.packageName == ownPackage) return null
            val n = sbn.notification ?: return null
            if (n.flags and android.app.Notification.FLAG_ONGOING_EVENT != 0) return null
            val extras = n.extras
            val title = extras.getCharSequence(android.app.Notification.EXTRA_TITLE)?.toString().orEmpty()
            val text = extras.getCharSequence(android.app.Notification.EXTRA_TEXT)?.toString().orEmpty()
            if (title.isEmpty() && text.isEmpty()) return null
            return NotificationInfo(sbn.key, appLabel, title, text, sbn.postTime)
        }
    }

    override fun onListenerConnected() {
        current.clear()
        try { activeNotifications?.forEach { add(it) } } catch (e: SecurityException) { /* access revoked */ }
    }

    override fun onNotificationPosted(sbn: StatusBarNotification) = add(sbn)

    override fun onNotificationRemoved(sbn: StatusBarNotification) { current.remove(sbn.key) }

    private fun add(sbn: StatusBarNotification) {
        val label = try {
            packageManager.getApplicationLabel(packageManager.getApplicationInfo(sbn.packageName, 0)).toString()
        } catch (e: Exception) { sbn.packageName }
        toInfo(sbn, packageName, label)?.let { current[sbn.key] = it }
    }
}
