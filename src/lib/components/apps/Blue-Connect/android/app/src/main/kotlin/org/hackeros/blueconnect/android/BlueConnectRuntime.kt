package org.hackeros.blueconnect.android

import android.os.Handler
import android.os.Looper
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import org.legendaryos.blueconnect.core.ConnectEngine
import org.legendaryos.blueconnect.core.DeviceInfo
import org.legendaryos.blueconnect.core.IncomingPairingRequest
import org.legendaryos.blueconnect.core.OutgoingPairingSas
import org.legendaryos.blueconnect.core.PairingDecision
import org.legendaryos.blueconnect.core.Protocol

/** An incoming pairing request waiting for the person's answer on screen. */
class PendingPairing(val request: IncomingPairingRequest, val decision: PairingDecision)

/**
 * Process-wide state shared between the foreground service (which owns the
 * engine) and the UI (which only observes it). Everything the screen shows
 * is a [StateFlow] here.
 */
object BlueConnectRuntime {
    private val _devices = MutableStateFlow<List<DeviceInfo>>(emptyList())
    val devices: StateFlow<List<DeviceInfo>> = _devices

    private val _incoming = MutableStateFlow<PendingPairing?>(null)
    val incoming: StateFlow<PendingPairing?> = _incoming

    private val _outgoing = MutableStateFlow<OutgoingPairingSas?>(null)
    val outgoing: StateFlow<OutgoingPairingSas?> = _outgoing

    private val _running = MutableStateFlow(false)
    val running: StateFlow<Boolean> = _running

    private val _message = MutableStateFlow<String?>(null)
    /** Last status / error line for the UI (null = nothing to show). */
    val message: StateFlow<String?> = _message

    private val _self = MutableStateFlow<SelfInfo?>(null)
    val self: StateFlow<SelfInfo?> = _self

    class SelfInfo(val deviceId: String, val deviceName: String, val fingerprint: String)

    @Volatile var engine: ConnectEngine? = null
        private set

    private val main = Handler(Looper.getMainLooper())

    fun attach(e: ConnectEngine) {
        engine = e
        _self.value = SelfInfo(e.deviceId, e.deviceName, e.fingerprint)
        _running.value = true
        refreshDevices()
    }

    fun detach() {
        engine = null
        _running.value = false
        _incoming.value = null
        _outgoing.value = null
    }

    fun refreshDevices() {
        _devices.value = engine?.devices() ?: emptyList()
    }

    fun postMessage(text: String?) { _message.value = text }

    fun showIncoming(request: IncomingPairingRequest, decision: PairingDecision) {
        val pending = PendingPairing(request, decision)
        _incoming.value = pending
        // The engine gives up after the same timeout; close our dialog with it.
        main.postDelayed({ if (_incoming.value === pending) _incoming.value = null }, Protocol.PAIRING_DECISION_TIMEOUT_MS)
    }

    fun answerIncoming(pending: PendingPairing, accept: Boolean) {
        if (accept) pending.decision.accept() else pending.decision.reject()
        if (_incoming.value === pending) _incoming.value = null
    }

    fun showOutgoing(info: OutgoingPairingSas?) { _outgoing.value = info }
}
