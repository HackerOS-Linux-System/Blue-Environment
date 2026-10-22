package org.hackeros.blueconnect.android

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.media.AudioAttributes
import android.media.AudioManager
import android.media.RingtoneManager
import android.net.Uri
import android.os.BatteryManager
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.os.VibrationEffect
import android.os.Vibrator
import org.hackeros.blueconnect.core.BatteryStatus
import org.hackeros.blueconnect.core.DeviceInfo
import org.hackeros.blueconnect.core.NotificationInfo
import org.hackeros.blueconnect.core.PluginHost
import java.util.concurrent.atomic.AtomicBoolean

/**
 * Android implementation of [PluginHost].
 *
 *  * battery — from the sticky ACTION_BATTERY_CHANGED broadcast;
 *  * ring — default alarm sound on the ALARM stream (works in Do-Not-Disturb) at full volume, plus
 *    vibration, for 20 s; the previous alarm volume is restored afterwards;
 *  * clipboard — writing always works; reading is blocked by Android 10+ while the app is in the
 *    background, in which case the desktop just receives an empty clipboard;
 *  * links / shared text — shown as a notification the person taps (Android forbids starting
 *    activities from a background service, so a direct "open" is not possible);
 *  * notifications — from [BlueNotificationListener] (needs "Notification access").
 */
class AndroidPluginHost(private val context: Context) : PluginHost {

    private val main = Handler(Looper.getMainLooper())
    private val ringing = AtomicBoolean(false)

    override fun getBattery(): BatteryStatus? {
        val i = context.registerReceiver(null, android.content.IntentFilter(Intent.ACTION_BATTERY_CHANGED)) ?: return null
        val level = i.getIntExtra(BatteryManager.EXTRA_LEVEL, -1)
        val scale = i.getIntExtra(BatteryManager.EXTRA_SCALE, -1)
        if (level < 0 || scale <= 0) return null
        val status = i.getIntExtra(BatteryManager.EXTRA_STATUS, -1)
        val charging = status == BatteryManager.BATTERY_STATUS_CHARGING || status == BatteryManager.BATTERY_STATUS_FULL
        return BatteryStatus(level * 100 / scale, charging)
    }

    override fun ring() {
        if (!ringing.compareAndSet(false, true)) return
        main.post {
            val audio = context.getSystemService(Context.AUDIO_SERVICE) as AudioManager
            val oldVolume = audio.getStreamVolume(AudioManager.STREAM_ALARM)
            try {
                audio.setStreamVolume(AudioManager.STREAM_ALARM, audio.getStreamMaxVolume(AudioManager.STREAM_ALARM), 0)
            } catch (e: SecurityException) { /* DND policy access — ring at the current volume */ }

            val uri = RingtoneManager.getDefaultUri(RingtoneManager.TYPE_ALARM) ?: RingtoneManager.getDefaultUri(RingtoneManager.TYPE_RINGTONE)
            val tone = RingtoneManager.getRingtone(context, uri)?.apply {
                audioAttributes = AudioAttributes.Builder().setUsage(AudioAttributes.USAGE_ALARM).setContentType(AudioAttributes.CONTENT_TYPE_SONIFICATION).build()
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) isLooping = true
                play()
            }
            val vib = context.getSystemService(Context.VIBRATOR_SERVICE) as? Vibrator
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) vib?.vibrate(VibrationEffect.createWaveform(longArrayOf(0, 600, 400), 0))
            else @Suppress("DEPRECATION") vib?.vibrate(longArrayOf(0, 600, 400), 0)

            main.postDelayed({
                tone?.stop()
                vib?.cancel()
                try { audio.setStreamVolume(AudioManager.STREAM_ALARM, oldVolume, 0) } catch (e: SecurityException) { /* ignore */ }
                ringing.set(false)
            }, RING_MS)
        }
    }

    override fun setClipboard(text: String) {
        main.post {
            val cm = context.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
            cm.setPrimaryClip(ClipData.newPlainText("Blue Connect", text))
        }
    }

    override fun getClipboard(): String? = try {
        val cm = context.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
        cm.primaryClip?.takeIf { it.itemCount > 0 }?.getItemAt(0)?.coerceToText(context)?.toString()
    } catch (e: Exception) { null }

    override fun openUrl(url: String, from: DeviceInfo) {
        val intent = Intent(Intent.ACTION_VIEW, Uri.parse(url)).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        Notifications.shared(context, "Link from ${from.name}", url, intent)
    }

    override fun receiveText(text: String, from: DeviceInfo) {
        setClipboard(text)
        Notifications.shared(context, "Text from ${from.name}", text.take(200) + "\n(copied to the clipboard)", null)
    }

    override fun getNotifications(): List<NotificationInfo> =
        if (BlueNotificationListener.isEnabled(context)) BlueNotificationListener.snapshot() else emptyList()

    private companion object {
        const val RING_MS = 20_000L
    }
}
