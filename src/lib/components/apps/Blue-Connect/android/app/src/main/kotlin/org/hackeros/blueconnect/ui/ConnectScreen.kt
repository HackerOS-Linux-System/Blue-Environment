package org.hackeros.blueconnect.ui

import android.Manifest
import android.os.Build
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import org.hackeros.blueconnect.android.BlueConnectRuntime
import org.hackeros.blueconnect.android.SmsPlugin
import org.hackeros.blueconnect.core.ConnectException
import org.hackeros.blueconnect.core.DeviceInfo

private fun spaced(sas: String) = if (sas.length == 6) "${sas.take(3)} ${sas.drop(3)}" else sas

@Composable
fun ConnectScreen(onStopService: () -> Unit, onStartService: () -> Unit) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()

    val devices by BlueConnectRuntime.devices.collectAsState()
    val incoming by BlueConnectRuntime.incoming.collectAsState()
    val outgoing by BlueConnectRuntime.outgoing.collectAsState()
    val running by BlueConnectRuntime.running.collectAsState()
    val message by BlueConnectRuntime.message.collectAsState()
    val self by BlueConnectRuntime.self.collectAsState()

    var scanning by remember { mutableStateOf(false) }
    var busyId by remember { mutableStateOf<String?>(null) }
    var smsGranted by remember { mutableStateOf(SmsPlugin.hasReadPermission(context) && SmsPlugin.hasSendPermission(context)) }

    val notifLauncher = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { }
    val smsLauncher = rememberLauncherForActivityResult(ActivityResultContracts.RequestMultiplePermissions()) {
        smsGranted = SmsPlugin.hasReadPermission(context) && SmsPlugin.hasSendPermission(context)
    }
    LaunchedEffect(Unit) {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) notifLauncher.launch(Manifest.permission.POST_NOTIFICATIONS)
    }

    fun scan() {
        val engine = BlueConnectRuntime.engine ?: return
        scope.launch {
            scanning = true
            try {
                withContext(Dispatchers.IO) { engine.scan(3000) }
                BlueConnectRuntime.refreshDevices()
            } catch (e: Exception) {
                BlueConnectRuntime.postMessage("Scan failed: ${e.message}")
            } finally {
                scanning = false
            }
        }
    }

    fun pair(d: DeviceInfo) {
        val engine = BlueConnectRuntime.engine ?: return
        scope.launch {
            busyId = d.id
            try {
                withContext(Dispatchers.IO) { engine.pair(d.id) }
                BlueConnectRuntime.postMessage("Paired with ${d.name}")
            } catch (e: ConnectException) {
                BlueConnectRuntime.postMessage(e.message)
            } catch (e: Exception) {
                BlueConnectRuntime.postMessage("Pairing failed: ${e.message}")
            } finally {
                BlueConnectRuntime.showOutgoing(null)
                busyId = null
                BlueConnectRuntime.refreshDevices()
            }
        }
    }

    fun ping(d: DeviceInfo) {
        val engine = BlueConnectRuntime.engine ?: return
        scope.launch {
            busyId = d.id
            try {
                withContext(Dispatchers.IO) { engine.ping(d.id, "Hello from ${self?.deviceName ?: "Android"}") }
                BlueConnectRuntime.postMessage("Ping sent to ${d.name}")
            } catch (e: Exception) {
                BlueConnectRuntime.postMessage(e.message)
            } finally {
                busyId = null
            }
        }
    }

    Surface(modifier = Modifier.fillMaxSize(), color = MaterialTheme.colorScheme.background) {
        Column(modifier = Modifier.statusBarsPadding().padding(horizontal = 16.dp)) {
            Spacer(Modifier.height(12.dp))
            Text("Blue Connect", fontSize = 26.sp, fontWeight = FontWeight.Bold)
            Text(
                if (running) "Visible as “${self?.deviceName ?: "…"}” on this network" else "Not running",
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            Spacer(Modifier.height(12.dp))

            Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                Button(onClick = { scan() }, enabled = running && !scanning) { Text(if (scanning) "Scanning…" else "Scan for devices") }
                if (running) OutlinedButton(onClick = onStopService) { Text("Stop") }
                else Button(onClick = onStartService) { Text("Start") }
            }

            message?.let {
                Spacer(Modifier.height(8.dp))
                Text(it, color = MaterialTheme.colorScheme.onSurfaceVariant, fontSize = 13.sp)
            }

            Spacer(Modifier.height(12.dp))
            SmsCard(smsGranted) {
                smsLauncher.launch(arrayOf(Manifest.permission.SEND_SMS, Manifest.permission.READ_SMS))
            }

            Spacer(Modifier.height(12.dp))
            Text("Devices", fontWeight = FontWeight.SemiBold)
            Spacer(Modifier.height(6.dp))

            if (devices.isEmpty()) {
                Text(
                    "Nothing found yet. On your computer open Blue Connect and press Scan — or press “Scan for devices” here.",
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            LazyColumn(verticalArrangement = Arrangement.spacedBy(8.dp), modifier = Modifier.weight(1f)) {
                items(devices, key = { it.id }) { d ->
                    DeviceCard(
                        d, busy = busyId == d.id,
                        onPair = { pair(d) }, onPing = { ping(d) },
                        onForget = { BlueConnectRuntime.engine?.forget(d.id); BlueConnectRuntime.refreshDevices() },
                    )
                }
            }

            self?.let {
                Text(
                    "This device's certificate: ${it.fingerprint.take(16)}…",
                    color = MaterialTheme.colorScheme.onSurfaceVariant, fontSize = 11.sp, fontFamily = FontFamily.Monospace,
                    modifier = Modifier.padding(vertical = 8.dp),
                )
            }
        }
    }

    incoming?.let { pending ->
        AlertDialog(
            onDismissRequest = { BlueConnectRuntime.answerIncoming(pending, false) },
            title = { Text("Pair with ${pending.request.deviceName}?") },
            text = {
                Column(horizontalAlignment = Alignment.CenterHorizontally, modifier = Modifier.fillMaxWidth()) {
                    Text("Make sure this code is EXACTLY the same on the other device (${pending.request.address}).")
                    Spacer(Modifier.height(12.dp))
                    Text(spaced(pending.request.sas), fontSize = 40.sp, fontWeight = FontWeight.Bold, fontFamily = FontFamily.Monospace)
                    Spacer(Modifier.height(8.dp))
                    Text("If it differs, someone may be intercepting the connection — reject it.", fontSize = 12.sp, color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
            },
            confirmButton = { Button(onClick = { BlueConnectRuntime.answerIncoming(pending, true) }) { Text("Codes match — pair") } },
            dismissButton = { TextButton(onClick = { BlueConnectRuntime.answerIncoming(pending, false) }) { Text("Reject") } },
        )
    }

    outgoing?.let { info ->
        AlertDialog(
            onDismissRequest = { BlueConnectRuntime.showOutgoing(null) },
            title = { Text("Confirm on ${info.deviceName}") },
            text = {
                Column(horizontalAlignment = Alignment.CenterHorizontally, modifier = Modifier.fillMaxWidth()) {
                    Text("Check that ${info.deviceName} shows this same code, then accept the request there.")
                    Spacer(Modifier.height(12.dp))
                    Text(spaced(info.sas), fontSize = 40.sp, fontWeight = FontWeight.Bold, fontFamily = FontFamily.Monospace)
                }
            },
            confirmButton = { TextButton(onClick = { BlueConnectRuntime.showOutgoing(null) }) { Text("Hide") } },
        )
    }
}

@Composable
private fun SmsCard(granted: Boolean, onEnable: () -> Unit) {
    Card(colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surface), modifier = Modifier.fillMaxWidth()) {
        Column(Modifier.padding(12.dp)) {
            Text("SMS relay", fontWeight = FontWeight.SemiBold)
            Text(
                if (granted) "On — paired computers can read and send text messages through this phone (Blue Messages)."
                else "Off — turn on to use this phone's SMS from Blue Messages on your computer.",
                color = MaterialTheme.colorScheme.onSurfaceVariant, fontSize = 13.sp,
            )
            if (!granted) {
                Spacer(Modifier.height(6.dp))
                OutlinedButton(onClick = onEnable) { Text("Allow SMS access") }
            }
        }
    }
}

@Composable
private fun DeviceCard(d: DeviceInfo, busy: Boolean, onPair: () -> Unit, onPing: () -> Unit, onForget: () -> Unit) {
    Card(colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceVariant), modifier = Modifier.fillMaxWidth()) {
        Column(Modifier.padding(12.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Column(Modifier.weight(1f)) {
                    Text(d.name, fontWeight = FontWeight.SemiBold)
                    Text(
                        "${d.deviceType} · ${d.address}${if (d.paired) " · paired" else ""}",
                        color = MaterialTheme.colorScheme.onSurfaceVariant, fontSize = 12.sp,
                    )
                }
            }
            if (d.paired) {
                d.pinnedCertSha256?.let {
                    Text("pinned ${it.take(16)}…", fontSize = 11.sp, fontFamily = FontFamily.Monospace, color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
            }
            Spacer(Modifier.height(8.dp))
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                if (d.paired) {
                    Button(onClick = onPing, enabled = !busy) { Text("Ping") }
                    OutlinedButton(onClick = onForget, enabled = !busy) { Text("Forget") }
                } else {
                    Button(onClick = onPair, enabled = !busy) { Text(if (busy) "Waiting…" else "Pair") }
                    OutlinedButton(onClick = onForget, enabled = !busy) { Text("Remove") }
                }
            }
        }
    }
}
