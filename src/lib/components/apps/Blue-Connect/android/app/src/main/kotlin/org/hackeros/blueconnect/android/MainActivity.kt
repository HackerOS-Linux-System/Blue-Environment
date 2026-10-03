package org.hackeros.blueconnect.android

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import org.hackeros.blueconnect.ui.BlueConnectTheme
import org.hackeros.blueconnect.ui.ConnectScreen

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        // Idempotent: starting an already running foreground service just re-delivers onStartCommand.
        ConnectService.start(this)
        setContent {
            BlueConnectTheme {
                ConnectScreen(onStopService = { ConnectService.stop(this) }, onStartService = { ConnectService.start(this) })
            }
        }
    }
}
