// lib/main.dart
import 'dart:io';
import 'package:flutter/material.dart';
import 'package:path_provider/path_provider.dart';
import 'package:screen_capturer/screen_capturer.dart';

void main() {
  runApp(const BlueScreenshotApp());
}

class BlueScreenshotApp extends StatelessWidget {
  const BlueScreenshotApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'Blue Screenshot',
      debugShowCheckedModeBanner: false,
      theme: ThemeData(
        primarySwatch: Colors.blue,
        scaffoldBackgroundColor: const Color(0xFF0A0F1E),
        appBarTheme: const AppBarTheme(
          backgroundColor: Color(0xFF1E3A8A),
          foregroundColor: Colors.white,
        ),
        elevatedButtonTheme: ElevatedButtonThemeData(
          style: ElevatedButton.styleFrom(
            backgroundColor: Colors.blue,
            foregroundColor: Colors.white,
            padding: const EdgeInsets.symmetric(horizontal: 32, vertical: 16),
          ),
        ),
      ),
      home: const HomeScreen(),
    );
  }
}

class HomeScreen extends StatefulWidget {
  const HomeScreen({super.key});

  @override
  State<HomeScreen> createState() => _HomeScreenState();
}

class _HomeScreenState extends State<HomeScreen> {
  CaptureMode _selectedMode = CaptureMode.screen;
  int _delaySeconds = 0;
  bool _copyToClipboard = true;

  final List<int> _delays = [0, 3, 5, 10];

  Future<void> _takeScreenshot() async {
    // Opóźnienie (jak w Spectacle)
    if (_delaySeconds > 0) {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: Text('Zrzut za $_delaySeconds sekund...'),
          duration: Duration(seconds: _delaySeconds),
        ),
      );
      await Future.delayed(Duration(seconds: _delaySeconds));
    }

    // Ścieżka zapisu
    final directory = await getDownloadsDirectory() ?? await getApplicationDocumentsDirectory();
    final fileName = 'blue_screenshot_${DateTime.now().year}'
        '${DateTime.now().month.toString().padLeft(2, '0')}'
        '${DateTime.now().day.toString().padLeft(2, '0')}_'
        '${DateTime.now().hour.toString().padLeft(2, '0')}'
        '${DateTime.now().minute.toString().padLeft(2, '0')}'
        '${DateTime.now().second.toString().padLeft(2, '0')}.png';
    final imagePath = '${directory.path}/$fileName';

    try {
      final CapturedData? capturedData = await ScreenCapturer.instance.capture(
        mode: _selectedMode,
        imagePath: imagePath,
        copyToClipboard: _copyToClipboard,
      );

      if (capturedData != null && File(imagePath).existsSync()) {
        if (!mounted) return;
        Navigator.of(context).push(
          MaterialPageRoute(
            builder: (_) => PreviewScreen(imagePath: imagePath),
          ),
        );
      } else {
        _showError('Nie udało się zrobić zrzutu (brak pliku)');
      }
    } catch (e) {
      _showError('Błąd: $e');
    }
  }

  void _showError(String message) {
    ScaffoldMessenger.of(context).showSnackBar(
      SnackBar(content: Text(message), backgroundColor: Colors.red),
    );
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: const Text('Blue Screenshot'),
        centerTitle: true,
        actions: [
          IconButton(
            icon: const Icon(Icons.info_outline),
            onPressed: () => _showInfoDialog(),
          ),
        ],
      ),
      body: Center(
        child: Container(
          constraints: const BoxConstraints(maxWidth: 520),
          padding: const EdgeInsets.all(24),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              // Tryby (jak w Spectacle)
              const Text(
                'Wybierz tryb zrzutu',
                style: TextStyle(fontSize: 18, color: Colors.white, fontWeight: FontWeight.bold),
              ),
              const SizedBox(height: 16),
              SegmentedButton<CaptureMode>(
                segments: const [
                  ButtonSegment(
                    value: CaptureMode.screen,
                    label: Text('Pełny ekran'),
                    icon: Icon(Icons.fullscreen),
                  ),
                  ButtonSegment(
                    value: CaptureMode.window,
                    label: Text('Okno'),
                    icon: Icon(Icons.window),
                  ),
                  ButtonSegment(
                    value: CaptureMode.region,
                    label: Text('Obszar'),
                    icon: Icon(Icons.crop),
                  ),
                ],
                selected: {_selectedMode},
                onSelectionChanged: (Set<CaptureMode> newSelection) {
                  setState(() => _selectedMode = newSelection.first);
                },
              ),

              const SizedBox(height: 32),

              // Opóźnienie
              Row(
                children: [
                  const Text('Opóźnienie:', style: TextStyle(color: Colors.white70)),
                  const Spacer(),
                  DropdownButton<int>(
                    value: _delaySeconds,
                    dropdownColor: const Color(0xFF1E3A8A),
                    style: const TextStyle(color: Colors.white),
                    items: _delays
                        .map((d) => DropdownMenuItem(
                              value: d,
                              child: Text(d == 0 ? 'Natychmiast' : '$d sekund'),
                            ))
                        .toList(),
                    onChanged: (value) {
                      if (value != null) setState(() => _delaySeconds = value);
                    },
                  ),
                ],
              ),

              const SizedBox(height: 16),

              // Kopia do schowka
              SwitchListTile(
                title: const Text('Kopiuj do schowka', style: TextStyle(color: Colors.white)),
                value: _copyToClipboard,
                activeColor: Colors.blue,
                onChanged: (v) => setState(() => _copyToClipboard = v),
              ),

              const SizedBox(height: 40),

              // Główny przycisk (duży, niebieski)
              SizedBox(
                width: double.infinity,
                child: ElevatedButton.icon(
                  icon: const Icon(Icons.camera_alt, size: 28),
                  label: const Text('Zrób zrzut ekranu', style: TextStyle(fontSize: 18)),
                  onPressed: _takeScreenshot,
                  style: ElevatedButton.styleFrom(
                    padding: const EdgeInsets.symmetric(vertical: 20),
                  ),
                ),
              ),

              const SizedBox(height: 24),
              const Text(
                'Zapisuje do folderu Pobrane • Tryb niebieski (Blue Environment)',
                style: TextStyle(fontSize: 12, color: Colors.white54),
                textAlign: TextAlign.center,
              ),
            ],
          ),
        ),
      ),
    );
  }

  void _showInfoDialog() {
    showDialog(
      context: context,
      builder: (_) => AlertDialog(
        title: const Text('Blue Screenshot'),
        content: const Text(
          'Aplikacja inspirowana KDE Spectacle.\n\n'
          '• Pełny ekran / Okno / Obszar prostokątny\n'
          '• Opóźnienie przed zrzutem\n'
          '• Automatycznie zapisuje do Pobranych\n'
          '• Kopia do schowka\n\n'
          'Uwagi:\n'
          '• Na macOS dodaj uprawnienie w entitlements (patrz README)\n'
          '• Na Windows zainstaluj C++ ATL w Visual Studio\n'
          '• Działa tylko na desktopie (Flutter desktop)',
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(context),
            child: const Text('OK'),
          ),
        ],
      ),
    );
  }
}

// Ekran podglądu
class PreviewScreen extends StatelessWidget {
  final String imagePath;

  const PreviewScreen({super.key, required this.imagePath});

  @override
  Widget build(BuildContext context) {
    final file = File(imagePath);

    return Scaffold(
      appBar: AppBar(title: const Text('Podgląd zrzutu')),
      body: Column(
        children: [
          Expanded(
            child: InteractiveViewer(
              child: Center(
                child: Image.file(
                  file,
                  fit: BoxFit.contain,
                  errorBuilder: (_, __, ___) => const Text(
                    'Nie można załadować obrazu',
                    style: TextStyle(color: Colors.red),
                  ),
                ),
              ),
            ),
          ),
          Container(
            padding: const EdgeInsets.all(16),
            color: const Color(0xFF1E3A8A),
            child: Column(
              children: [
                Text(
                  'Zapisano jako:\n$imagePath',
                  textAlign: TextAlign.center,
                  style: const TextStyle(color: Colors.white70, fontSize: 13),
                ),
                const SizedBox(height: 16),
                Row(
                  mainAxisAlignment: MainAxisAlignment.spaceEvenly,
                  children: [
                    ElevatedButton.icon(
                      icon: const Icon(Icons.folder_open),
                      label: const Text('Otwórz folder'),
                      onPressed: () {
                        // Na desktopie można otworzyć folder ręcznie (ścieżka pokazana)
                        ScaffoldMessenger.of(context).showSnackBar(
                          SnackBar(content: Text('Ścieżka: $imagePath')),
                        );
                      },
                    ),
                    ElevatedButton.icon(
                      icon: const Icon(Icons.close),
                      label: const Text('Zamknij'),
                      onPressed: () => Navigator.pop(context),
                    ),
                  ],
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}
