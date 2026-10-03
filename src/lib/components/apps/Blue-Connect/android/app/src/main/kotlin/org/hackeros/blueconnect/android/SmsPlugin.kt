package org.hackeros.blueconnect.android

import android.Manifest
import android.content.Context
import android.content.ContentResolver
import android.content.pm.PackageManager
import android.os.Build
import android.os.Bundle
import android.provider.Telephony
import android.telephony.SmsManager
import androidx.core.content.ContextCompat
import org.json.JSONArray
import org.json.JSONObject
import org.legendaryos.blueconnect.core.DeviceInfo
import org.legendaryos.blueconnect.core.PacketHandler
import org.legendaryos.blueconnect.core.Protocol

/**
 * The phone side of Blue Messages' SMS bridge (`kdeconnect.sms.request`, the
 * same packets the desktop's `BlueMessagesApp/sms.rs` speaks):
 *
 *  * `{sendSms: true, phoneNumber, messageBody}` → sends the text message.
 *  * `{requestAllConversations: true}` → replies with ONE
 *    `kdeconnect.sms.messages` packet `{messages: [{address, body, date, type}]}`
 *    newest first, trimmed so the whole reply fits into a single TLS record
 *    ([Protocol.MAX_SINGLE_READ_REPLY_BYTES]) — the desktop reads it with one
 *    `read()` call.
 *
 * Only ever reached for PAIRED devices (the engine checks the pinned
 * certificate first), and only works once the person granted the SMS
 * permissions in the app.
 */
class SmsPlugin(private val context: Context, private val log: (String) -> Unit) : PacketHandler {

    companion object {
        const val MAX_BODY_CHARS = 1000
        private const val MAX_ROWS = 200
        private const val ENVELOPE_OVERHEAD = 120 // {"type":"…","body":{"messages":[]}}\n

        fun hasSendPermission(c: Context) =
            ContextCompat.checkSelfPermission(c, Manifest.permission.SEND_SMS) == PackageManager.PERMISSION_GRANTED

        fun hasReadPermission(c: Context) =
            ContextCompat.checkSelfPermission(c, Manifest.permission.READ_SMS) == PackageManager.PERMISSION_GRANTED

        /** Only digits, +, spaces and the usual separators — never let a packet smuggle anything else into the dialer API. */
        fun isPlausibleNumber(n: String): Boolean =
            n.length in 3..32 && n.all { it.isDigit() || it in "+-() *#" } && n.count { it.isDigit() } >= 3
    }

    override fun handle(device: DeviceInfo, packet: Protocol.Packet, reply: (ByteArray) -> Unit) {
        if (packet.type != Protocol.TYPE_SMS_REQUEST) return
        val body = packet.body
        when {
            body.optBoolean("sendSms", false) -> send(device, body.optString("phoneNumber", ""), body.optString("messageBody", ""))
            body.optBoolean("requestAllConversations", false) -> reply(buildMessagesReply(device))
        }
    }

    private fun send(device: DeviceInfo, number: String, text: String) {
        if (!hasSendPermission(context)) { log("${device.name} asked to send an SMS, but the SMS permission is not granted"); return }
        if (!isPlausibleNumber(number) || text.isEmpty()) { log("Ignored an invalid SMS request from ${device.name}"); return }
        try {
            val sms = smsManager()
            val parts = sms.divideMessage(text)
            if (parts.size > 1) sms.sendMultipartTextMessage(number, null, parts, null, null)
            else sms.sendTextMessage(number, null, text, null, null)
            log("Sent an SMS to $number for ${device.name}")
        } catch (e: Exception) {
            log("Could not send the SMS: ${e.message}")
        }
    }

    @Suppress("DEPRECATION")
    private fun smsManager(): SmsManager =
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) context.getSystemService(SmsManager::class.java) else SmsManager.getDefault()

    private fun buildMessagesReply(device: DeviceInfo): ByteArray {
        val messages = JSONArray()
        if (!hasReadPermission(context)) {
            log("${device.name} asked for SMS history, but the READ_SMS permission is not granted")
            return Protocol.frame(Protocol.TYPE_SMS_MESSAGES, JSONObject().put("messages", messages))
        }
        var budget = Protocol.MAX_SINGLE_READ_REPLY_BYTES - ENVELOPE_OVERHEAD
        try {
            val args = Bundle().apply {
                putString(ContentResolver.QUERY_ARG_SQL_SORT_ORDER, "${Telephony.Sms.DATE} DESC")
                putInt(ContentResolver.QUERY_ARG_LIMIT, MAX_ROWS)
            }
            context.contentResolver.query(
                Telephony.Sms.CONTENT_URI,
                arrayOf(Telephony.Sms.ADDRESS, Telephony.Sms.BODY, Telephony.Sms.DATE, Telephony.Sms.TYPE),
                args, null,
            )?.use { c ->
                while (c.moveToNext()) {
                    val msg = JSONObject()
                        .put("address", c.getString(0) ?: "")
                        .put("body", (c.getString(1) ?: "").take(MAX_BODY_CHARS))
                        .put("date", c.getLong(2))
                        .put("type", c.getInt(3))
                    val size = msg.toString().toByteArray(Charsets.UTF_8).size + 1
                    if (size > budget) break
                    budget -= size
                    messages.put(msg)
                }
            }
        } catch (e: Exception) {
            log("Could not read SMS history: ${e.message}")
        }
        return Protocol.frame(Protocol.TYPE_SMS_MESSAGES, JSONObject().put("messages", messages))
    }
}
