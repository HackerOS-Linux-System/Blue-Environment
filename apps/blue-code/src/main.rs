import 'package:flutter/material.dart';
import 'package:file_picker/file_picker.dart';
import 'package:path/path.dart' as p;
import 'dart:io' as io;

void main() {
  runApp(const BlueEditApp());
}

class BlueEditApp extends StatelessWidget {
  const BlueEditApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'Blue Edit',
      debugShowCheckedModeBanner: false,
      theme: ThemeData.dark().copyWith(
        scaffoldBackgroundColor: const Color(0xFF1E1E1E),
        primaryColor: const Color(0xFF2196F3),
        appBarTheme: const AppBarTheme(
          backgroundColor: Color(0xFF1E1E1E),
          elevation: 0,
        ),
        textTheme: const TextTheme(
          bodyMedium: TextStyle(color: Colors.white, fontSize: 13),
        ),
      ),
      home: const BlueEditScreen(),
    );
  }
}

class BlueEditScreen extends StatefulWidget {
  const BlueEditScreen({super.key});

  @override
  State<BlueEditScreen> createState() => _BlueEditScreenState();
}

class _BlueEditScreenState extends State<BlueEditScreen> {
  List<Map<String, dynamic>> tabs = [];
  int currentTab = -1;
  String? currentDir;
  List<io.FileSystemEntity> dirContents = [];
  bool isLoadingDir = false;
  String statusText = 'Gotowy – otwórz folder projektu';

  @override
  void initState() {
    super.initState();
    // przykładowa zakładka startowa
    _addNewTab();
  }

  // ==================== ZARZĄDZANIE ZAKŁADKAMI ====================

  void _addNewTab({String name = 'untitled.txt', String? path, String content = ''}) {
    final controller = TextEditingController(text: content);
    final tab = {
      'name': name,
      'path': path,
      'controller': controller,
      'isDirty': false,
    };
    tabs.add(tab);

    // listener do wykrywania zmian
    controller.addListener(() {
      final idx = tabs.indexWhere((t) => t['controller'] == controller);
      if (idx != -1 && tabs[idx]['isDirty'] == false) {
        setState(() => tabs[idx]['isDirty'] = true);
      }
    });

    currentTab = tabs.length - 1;
    setState(() {});
    _updateStatus();
  }

  Future<void> _saveTab(int index) async {
    if (index < 0 || index >= tabs.length) return;
    final tab = tabs[index];
    final content = tab['controller'].text as String;
    final path = tab['path'] as String?;

    try {
      if (path != null) {
        await io.File(path).writeAsString(content);
      } else {
        final savePath = await FilePicker.platform.saveFile(
          dialogTitle: 'Zapisz plik jako...',
          fileName: tab['name'],
        );
        if (savePath == null) return;
        await io.File(savePath).writeAsString(content);
        tab['path'] = savePath;
        tab['name'] = p.basename(savePath);
      }
      tab['isDirty'] = false;
      setState(() {});
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(content: Text('✓ Zapisano'), backgroundColor: Colors.green),
      );
      _updateStatus();
    } catch (e) {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text('Błąd zapisu: $e'), backgroundColor: Colors.red),
      );
    }
  }

  void _forceCloseTab(int index) {
    (tabs[index]['controller'] as TextEditingController).dispose();
    tabs.removeAt(index);
    if (currentTab >= tabs.length) currentTab = tabs.length - 1;
    if (currentTab < 0) currentTab = 0;
    setState(() {});
    _updateStatus();
  }

  void _closeTab(int index) async {
    final tab = tabs[index];
    if (tab['isDirty'] == true) {
      final result = await showDialog<String>(
        context: context,
        builder: (ctx) => AlertDialog(
          title: const Text('Niezapisane zmiany'),
          content: Text('Plik „${tab['name']}” zawiera niezapisane zmiany.'),
          actions: [
            TextButton(onPressed: () => Navigator.pop(ctx, 'cancel'), child: const Text('Anuluj')),
            TextButton(onPressed: () => Navigator.pop(ctx, 'discard'), child: const Text('Odrzuć')),
            TextButton(
              onPressed: () async {
                Navigator.pop(ctx, 'save');
                await _saveTab(index);
              },
              child: const Text('Zapisz i zamknij'),
            ),
          ],
        ),
      );

      if (result == 'cancel') return;
      if (result == 'save') {
        _forceCloseTab(index);
        return;
      }
    }
    _forceCloseTab(index);
  }

  // ==================== EKSPLORATOR PLIKÓW ====================

  Future<void> _openFolder() async {
    final selected = await FilePicker.platform.getDirectoryPath(dialogTitle: 'Otwórz folder projektu');
    if (selected == null) return;
    currentDir = selected;
    await _loadDirContents();
  }

  Future<void> _loadDirContents() async {
    if (currentDir == null) return;
    setState(() => isLoadingDir = true);
    try {
      final dir = io.Directory(currentDir!);
      dirContents = await dir.list().toList();

      dirContents.sort((a, b) {
        final aDir = a is io.Directory;
        final bDir = b is io.Directory;
        if (aDir && !bDir) return -1;
        if (!aDir && bDir) return 1;
        return p.basename(a.path).toLowerCase().compareTo(p.basename(b.path).toLowerCase());
      });
    } catch (e) {
      ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text('Błąd wczytywania folderu: $e')));
    }
    setState(() => isLoadingDir = false);
  }

  Future<void> _openFile(String fullPath) async {
    final file = io.File(fullPath);
    if (!await file.exists()) return;

    final existing = tabs.indexWhere((t) => t['path'] == fullPath);
    if (existing != -1) {
      currentTab = existing;
      setState(() {});
      _updateStatus();
      return;
    }

    try {
      final content = await file.readAsString();
      final name = p.basename(fullPath);
      _addNewTab(name: name, path: fullPath, content: content);
    } catch (e) {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text('Nie można otworzyć pliku (prawdopodobnie binarny)')),
      );
    }
  }

  void _updateStatus() {
    if (currentTab < 0 || tabs.isEmpty) {
      statusText = 'Gotowy';
      setState(() {});
      return;
    }
    final tab = tabs[currentTab];
    final path = tab['path'] ?? 'Nowy plik';
    final ctrl = tab['controller'] as TextEditingController;
    final lines = ctrl.text.split('\n').length;
    final dirty = tab['isDirty'] == true ? ' • Zmodyfikowany' : '';

    statusText = '${p.basename(path)}$dirty • $lines linii • UTF-8';
    setState(() {});
  }

  // ==================== BUILD ====================

  Widget _buildTab(int index) {
    final tab = tabs[index];
    final isActive = index == currentTab;
    final dirty = tab['isDirty'] == true ? '*' : '';

    return GestureDetector(
      onTap: () {
        currentTab = index;
        setState(() {});
        _updateStatus();
      },
      child: Container(
        height: 36,
        padding: const EdgeInsets.symmetric(horizontal: 16),
        decoration: BoxDecoration(
          color: isActive ? const Color(0xFF2D2D2D) : const Color(0xFF1E1E1E),
          border: isActive
              ? const Border(bottom: BorderSide(color: Color(0xFF2196F3), width: 3))
              : null,
        ),
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            Text(
              '$dirty${tab['name']}',
              style: TextStyle(
                color: isActive ? Colors.white : Colors.grey[400],
                fontSize: 13,
                fontWeight: isActive ? FontWeight.w500 : FontWeight.normal,
              ),
            ),
            const SizedBox(width: 8),
            GestureDetector(
              onTap: () => _closeTab(index),
              child: const Icon(Icons.close, size: 16, color: Colors.grey),
            ),
          ],
        ),
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: const Row(
          children: [
            Icon(Icons.code, color: Color(0xFF2196F3), size: 28),
            SizedBox(width: 8),
            Text('Blue Edit'),
          ],
        ),
        actions: [
          IconButton(
            icon: const Icon(Icons.folder_open),
            tooltip: 'Otwórz folder projektu',
            onPressed: _openFolder,
          ),
          IconButton(
            icon: const Icon(Icons.add),
            tooltip: 'Nowy plik',
            onPressed: () => _addNewTab(),
          ),
          IconButton(
            icon: const Icon(Icons.save),
            tooltip: 'Zapisz',
            onPressed: () => _saveTab(currentTab),
          ),
          const SizedBox(width: 12),
        ],
      ),
      body: Row(
        children: [
          // ==================== EKSPLORATOR ====================
          Container(
            width: 280,
            color: const Color(0xFF252526),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Padding(
                  padding: const EdgeInsets.all(12),
                  child: Row(
                    children: [
                      const Text('EKSPLORATOR', style: TextStyle(fontSize: 12, color: Colors.grey, letterSpacing: 1.5)),
                      const Spacer(),
                      if (currentDir != null)
                        IconButton(
                          icon: const Icon(Icons.refresh, size: 18),
                          onPressed: _loadDirContents,
                        ),
                    ],
                  ),
                ),
                if (currentDir != null)
                  Padding(
                    padding: const EdgeInsets.symmetric(horizontal: 12),
                    child: Text(
                      p.basename(currentDir!),
                      style: const TextStyle(fontWeight: FontWeight.bold, color: Color(0xFF2196F3)),
                    ),
                  ),
                const Divider(color: Colors.black38),
                Expanded(
                  child: currentDir == null
                      ? Center(
                          child: Column(
                            mainAxisSize: MainAxisSize.min,
                            children: [
                              const Icon(Icons.folder_open, size: 64, color: Colors.grey),
                              const SizedBox(height: 16),
                              const Text('Brak otwartego projektu', style: TextStyle(color: Colors.grey)),
                              const SizedBox(height: 8),
                              ElevatedButton.icon(
                                onPressed: _openFolder,
                                icon: const Icon(Icons.folder_open),
                                label: const Text('Otwórz folder projektu'),
                                style: ElevatedButton.styleFrom(backgroundColor: const Color(0xFF2196F3)),
                              ),
                            ],
                          ),
                        )
                      : isLoadingDir
                          ? const Center(child: CircularProgressIndicator())
                          : ListView.builder(
                              itemCount: dirContents.length,
                              itemBuilder: (context, i) {
                                final entity = dirContents[i];
                                final isDir = entity is io.Directory;
                                final name = p.basename(entity.path);
                                return ListTile(
                                  dense: true,
                                  leading: Icon(
                                    isDir ? Icons.folder : Icons.insert_drive_file,
                                    color: isDir ? Colors.amber : Colors.blue,
                                  ),
                                  title: Text(name, style: const TextStyle(fontSize: 13)),
                                  onTap: isDir
                                      ? () {
                                          currentDir = entity.path;
                                          _loadDirContents();
                                        }
                                      : () => _openFile(entity.path),
                                );
                              },
                            ),
                ),
              ],
            ),
          ),

          const VerticalDivider(width: 1, color: Colors.black),

          // ==================== EDYTOR ====================
          Expanded(
            child: Column(
              children: [
                // Pasek zakładek
                Container(
                  height: 36,
                  color: const Color(0xFF1F1F1F),
                  child: SingleChildScrollView(
                    scrollDirection: Axis.horizontal,
                    child: Row(children: List.generate(tabs.length, _buildTab)),
                  ),
                ),

                // Edytor
                Expanded(
                  child: tabs.isEmpty || currentTab < 0
                      ? const Center(
                          child: Text(
                            'Otwórz plik z eksploratora lub utwórz nowy',
                            style: TextStyle(color: Colors.grey, fontSize: 16),
                          ),
                        )
                      : Container(
                          color: const Color(0xFF1E1E1E),
                          padding: const EdgeInsets.all(12),
                          child: TextField(
                            controller: tabs[currentTab]['controller'],
                            maxLines: null,
                            expands: true,
                            style: const TextStyle(
                              fontFamily: 'monospace',
                              fontSize: 14,
                              height: 1.5,
                            ),
                            decoration: const InputDecoration(
                              border: InputBorder.none,
                              contentPadding: EdgeInsets.zero,
                            ),
                            onChanged: (_) => _updateStatus(),
                          ),
                        ),
                ),

                // Pasek statusu
                Container(
                  height: 28,
                  color: const Color(0xFF007ACC),
                  padding: const EdgeInsets.symmetric(horizontal: 16),
                  child: Row(
                    children: [
                      Text(statusText, style: const TextStyle(color: Colors.white, fontSize: 12)),
                      const Spacer(),
                      const Text('Blue Edit • Linux • Dart + Flutter', style: TextStyle(color: Colors.white70, fontSize: 12)),
                    ],
                  ),
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}
