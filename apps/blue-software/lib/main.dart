import 'dart:io';
import 'package:flutter/material.dart';

void main() {
  runApp(const BlueSoftwareApp());
}

class BlueSoftwareApp extends StatelessWidget {
  const BlueSoftwareApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'Blue Software',
      debugShowCheckedModeBanner: false,
      theme: ThemeData(
        primarySwatch: Colors.blue,
        scaffoldBackgroundColor: const Color(0xFF0A0F1E),
        appBarTheme: const AppBarTheme(
          backgroundColor: Color(0xFF1E3A8A),
          foregroundColor: Colors.white,
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

class _HomeScreenState extends State<HomeScreen> with SingleTickerProviderStateMixin {
  late TabController _tabController;
  final TextEditingController _searchController = TextEditingController();
  String _searchQuery = '';
  bool _isLoading = false;
  List<Map<String, String>> _currentResults = [];

  final List<String> _tabTitles = ['Flatpak', 'Snap', 'LPM (APT)', 'AppImage'];

  @override
  void initState() {
    super.initState();
    _tabController = TabController(length: 4, vsync: this);
    _tabController.addListener(_loadCurrentTab);
    _searchController.addListener(() {
      _searchQuery = _searchController.text.trim();
      _loadCurrentTab();
    });
    _loadCurrentTab(); // initial load
  }

  @override
  void dispose() {
    _tabController.dispose();
    _searchController.dispose();
    super.dispose();
  }

  Future<void> _loadCurrentTab() async {
    if (_searchQuery.isEmpty) {
      setState(() => _currentResults = []);
      return;
    }

    setState(() => _isLoading = true);
    final index = _tabController.index;

    List<Map<String, String>> results = [];
    if (index == 0) results = await _searchFlatpak(_searchQuery);
    if (index == 1) results = await _searchSnap(_searchQuery);
    if (index == 2) results = await _searchLPM(_searchQuery);
    if (index == 3) results = []; // AppImage nie ma wyszukiwania

    setState(() {
      _currentResults = results;
      _isLoading = false;
    });
  }

  // ================== FLATPAK ==================
  Future<List<Map<String, String>>> _searchFlatpak(String query) async {
    try {
      final result = await Process.run('flatpak', ['search', query]);
      final lines = result.stdout.toString().split('\n').skip(1).take(15);
      return lines.where((l) => l.trim().isNotEmpty).map((line) {
        final idMatch = RegExp(r'\b([a-z][a-z0-9_]*\.[a-z0-9_.\-]+)\b').firstMatch(line);
        final id = idMatch?.group(1) ?? line.trim().split(' ').first;
        return {
          'name': line.trim().split(RegExp(r'\s{2,}')).first,
          'id': id,
          'desc': line.trim(),
          'backend': 'flatpak',
        };
      }).toList();
    } catch (_) {
      return [];
    }
  }

  // ================== SNAP ==================
  Future<List<Map<String, String>>> _searchSnap(String query) async {
    try {
      final result = await Process.run('snap', ['find', query]);
      final lines = result.stdout.toString().split('\n').skip(1).take(15);
      return lines.where((l) => l.trim().isNotEmpty).map((line) {
        final parts = line.trim().split(RegExp(r'\s{2,}'));
        final name = parts.isNotEmpty ? parts[0] : 'unknown';
      return {
        'name': name,
        'id': name,
        'desc': parts.length > 4 ? parts[4] : line.trim(),
        'backend': 'snap',
      };
      }).toList();
    } catch (_) {
      return [];
    }
  }

  // ================== LPM (APT) ==================
  Future<List<Map<String, String>>> _searchLPM(String query) async {
    try {
      final result = await Process.run('apt-cache', ['search', query]);
      final lines = result.stdout.toString().split('\n').take(15);
      return lines.where((l) => l.trim().isNotEmpty).map((line) {
        final match = RegExp(r'^(\S+)\s*-\s*(.+)$').firstMatch(line.trim());
        if (match == null) return null;
        final name = match.group(1)!;
        return {
          'name': name,
          'id': name,
          'desc': match.group(2)!,
          'backend': 'lpm',
        };
      }).whereType<Map<String, String>>().toList();
    } catch (_) {
      return [];
    }
  }

  // ================== WYKONYWANIE POLECEŃ ==================
  Future<void> _runCommand(List<String> command, String title) async {
    setState(() => _isLoading = true);
    try {
      final result = await Process.run(command[0], command.sublist(1));
      final output = '${result.stdout}\n${result.stderr}';

      if (!mounted) return;
      showDialog(
        context: context,
        builder: (ctx) => AlertDialog(
          backgroundColor: const Color(0xFF1E3A8A),
          title: Text(title, style: const TextStyle(color: Colors.white)),
          content: SingleChildScrollView(child: Text(output, style: const TextStyle(color: Colors.white70, fontFamily: 'monospace'))),
          actions: [
            TextButton(
              onPressed: () => Navigator.pop(ctx),
              child: const Text('OK', style: TextStyle(color: Colors.white)),
            ),
          ],
        ),
      );
    } catch (e) {
      if (!mounted) return;
      ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text('Błąd: $e')));
    } finally {
      setState(() => _isLoading = false);
    }
  }

  Future<void> _installApp(Map<String, String> app) async {
    final backend = app['backend']!;
    final id = app['id']!;

    if (backend == 'flatpak') {
      await _runCommand(['flatpak', 'install', '-y', 'flathub', id], 'Instalacja Flatpak');
    } else if (backend == 'snap') {
      await _runCommand(['sudo', 'snap', 'install', id], 'Instalacja Snap');
    } else if (backend == 'lpm') {
      await _runCommand(['sudo', 'lpm', 'install', id], 'Instalacja LPM (APT)');
    }
  }

  Future<void> _removeApp(Map<String, String> app) async {
    final backend = app['backend']!;
    final id = app['id']!;

    if (backend == 'flatpak') {
      await _runCommand(['flatpak', 'uninstall', '-y', id], 'Usuwanie Flatpak');
    } else if (backend == 'snap') {
      await _runCommand(['sudo', 'snap', 'remove', id], 'Usuwanie Snap');
    } else if (backend == 'lpm') {
      await _runCommand(['sudo', 'lpm', 'remove', id], 'Usuwanie LPM (APT)');
    }
  }

  // ================== APPIMAGE ==================
  Future<void> _installAppImage() async {
    final urlController = TextEditingController();
    final nameController = TextEditingController();

    await showDialog(
      context: context,
      builder: (ctx) => AlertDialog(
        title: const Text('Zainstaluj AppImage'),
        content: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            TextField(controller: nameController, decoration: const InputDecoration(labelText: 'Nazwa aplikacji')),
            TextField(controller: urlController, decoration: const InputDecoration(labelText: 'URL .AppImage')),
          ],
        ),
        actions: [
          TextButton(onPressed: () => Navigator.pop(ctx), child: const Text('Anuluj')),
          TextButton(
            onPressed: () async {
              Navigator.pop(ctx);
              final name = nameController.text.trim();
              final url = urlController.text.trim();
              if (name.isEmpty || url.isEmpty) return;

              final home = Platform.environment['HOME']!;
              final dir = '$home/Applications';
          await Directory(dir).create(recursive: true);
          final path = '$dir/$name.AppImage';

          final wget = await Process.run('wget', [url, '-O', path]);
          if (wget.exitCode == 0) {
            await Process.run('chmod', ['+x', path]);
            final desktopPath = '$home/.local/share/applications/$name.desktop';
          await File(desktopPath).writeAsString('''
          [Desktop Entry]
          Name=$name
          Exec=$path
          Icon=application-x-executable
          Type=Application
          Categories=Utility;
          ''');
          ScaffoldMessenger.of(context).showSnackBar(const SnackBar(content: Text('AppImage zainstalowany!')));
          }
            },
            child: const Text('Zainstaluj'),
          ),
        ],
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: const Text('Blue Software'),
        bottom: TabBar(
          controller: _tabController,
          tabs: _tabTitles.map((t) => Tab(text: t)).toList(),
          onTap: (_) => _loadCurrentTab(),
        ),
      ),
      body: Column(
        children: [
          Padding(
            padding: const EdgeInsets.all(12),
            child: TextField(
              controller: _searchController,
              decoration: InputDecoration(
                hintText: 'Szukaj aplikacji...',
                prefixIcon: const Icon(Icons.search),
                filled: true,
                fillColor: Colors.white10,
                border: OutlineInputBorder(borderRadius: BorderRadius.circular(12)),
              ),
            ),
          ),
          if (_isLoading)
            const LinearProgressIndicator()
            else if (_tabController.index == 3)
              Expanded(
                child: Center(
                  child: Column(
                    mainAxisAlignment: MainAxisAlignment.center,
                    children: [
                      const Icon(Icons.android, size: 80, color: Colors.blue),
                      const SizedBox(height: 16),
                      const Text('AppImage — instalacja z URL', style: TextStyle(fontSize: 20)),
                      const SizedBox(height: 24),
                      ElevatedButton.icon(
                        onPressed: _installAppImage,
                        icon: const Icon(Icons.download),
                        label: const Text('Zainstaluj AppImage z URL'),
                        style: ElevatedButton.styleFrom(padding: const EdgeInsets.symmetric(horizontal: 32, vertical: 16)),
                      ),
                    ],
                  ),
                ),
              )
              else
                Expanded(
                  child: _currentResults.isEmpty
                  ? const Center(child: Text('Wpisz coś w wyszukiwarkę...', style: TextStyle(color: Colors.white54)))
                  : ListView.builder(
                    itemCount: _currentResults.length,
                    itemBuilder: (context, i) {
                      final app = _currentResults[i];
                      return Card(
                        margin: const EdgeInsets.symmetric(horizontal: 12, vertical: 6),
                        color: const Color(0xFF1E3A8A),
                        child: ListTile(
                          title: Text(app['name']!, style: const TextStyle(color: Colors.white, fontWeight: FontWeight.bold)),
                          subtitle: Text(app['desc']!, style: const TextStyle(color: Colors.white70)),
                          trailing: Row(
                            mainAxisSize: MainAxisSize.min,
                            children: [
                              ElevatedButton(
                                onPressed: () => _installApp(app),
                                style: ElevatedButton.styleFrom(backgroundColor: Colors.green),
                                child: const Text('Instaluj'),
                              ),
                              const SizedBox(width: 8),
                              ElevatedButton(
                                onPressed: () => _removeApp(app),
                                style: ElevatedButton.styleFrom(backgroundColor: Colors.red),
                                child: const Text('Usuń'),
                              ),
                            ],
                          ),
                        ),
                      );
                    },
                  ),
                ),
        ],
      ),
      floatingActionButton: FloatingActionButton(
        onPressed: _loadCurrentTab,
        child: const Icon(Icons.refresh),
      ),
    );
  }
}
