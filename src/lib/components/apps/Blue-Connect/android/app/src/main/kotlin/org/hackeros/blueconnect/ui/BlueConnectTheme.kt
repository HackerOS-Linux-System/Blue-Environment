package org.hackeros.blueconnect.ui

import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color

// Same deep-navy / electric-blue palette as the Blue Environment shell.
private val BlueScheme = darkColorScheme(
    primary = Color(0xFF3B82F6),
    onPrimary = Color.White,
    secondary = Color(0xFF6366F1),
    background = Color(0xFF060D1F),
    onBackground = Color(0xFFE2E8F0),
    surface = Color(0xFF0F1C3F),
    onSurface = Color(0xFFE2E8F0),
    surfaceVariant = Color(0xFF16254F),
    onSurfaceVariant = Color(0xFF94A3B8),
    error = Color(0xFFF87171),
)

@Composable
fun BlueConnectTheme(content: @Composable () -> Unit) {
    MaterialTheme(colorScheme = BlueScheme, content = content)
}
