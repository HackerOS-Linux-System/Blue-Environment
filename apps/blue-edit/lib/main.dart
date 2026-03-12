import 'package:flutter/material.dart';

void main() {
  runApp(const MyApp());
}

class MyApp extends StatelessWidget {
  const MyApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'Flutter Editor',
      debugShowCheckedModeBanner: false,
      theme: ThemeData.dark().copyWith(
        scaffoldBackgroundColor: const Color(0xFF1E1E1E),
        textTheme: const TextTheme(
          bodyMedium: TextStyle(color: Colors.white),
        ),
      ),
      home: const EditorScreen(),
    );
  }
}

class EditorScreen extends StatefulWidget {
  const EditorScreen({super.key});

  @override
  State<EditorScreen> createState() => _EditorScreenState();
}

class _EditorScreenState extends State<EditorScreen> {
  List<Map<String, dynamic>> tabs = [];
  int currentTab = -1;
  String statusText = 'Gotowy';

  final List<String> explorerFiles = [
    'main.dart',
    'lib/app.dart',
    'pubspec.yaml',
    'README.md',
    'example.txt',
  ];

  @override
  void initState() {
    super.initState();
    // Domyślna zakładka przy uruchomieniu
    _addTab('main.dart', 'void main() {\n  print("Witaj w edytorze inspirowanym Kate i VS Code!");\n}');
  }

  void _addTab(String name, [String initialText = '']) {
    final controller = TextEditingController(text: initialText);
    tabs.add({
      'name': name,
      'controller': controller,
    });
    currentTab = tabs.length - 1;
    setState(() {});
    _updateStatus();
  }

  void _closeTab(int index) {
    if (tabs.length == 1) {
      tabs[0]['controller'].clear();
      setState(() {});
      _updateStatus();
      return;
    }

    tabs.removeAt(index);
    if (currentTab >= tabs.length) {
      currentTab = tabs.length - 1;
    }
    if (currentTab < 0) currentTab = 0;

    setState(() {});
    _updateStatus();
  }

  void _openFile(String filename) {
    final existingIndex = tabs.indexWhere((t) => t['name'] == filename);
    if (existingIndex != -1) {
      currentTab = existingIndex;
      setState(() {});
      _updateStatus();
      return;
    }

    String initialText = '// Plik: $filename\n';
    if (filename.endsWith('.dart')) {
      initialText += 'void main() {\n  // kod Dart\n}\n';
    } else if (filename.endsWith('.md')) {
      initialText += '# Nagłówek\n\nTo jest przykładowy plik Markdown.';
    } else if (filename.endsWith('.yaml')) {
      initialText += 'name: text_editor_flutter\nversion: 1.0.0';
    }

    _addTab(filename, initialText);
  }

  void _newFile() {
    final name = 'untitled${tabs.length + 1}.dart';
    _addTab(name);
  }

  void _saveFile() {
    if (tabs.isEmpty || currentTab < 0) return;

    final content = tabs[currentTab]['controller'].text;
    final fileName = tabs[currentTab]['name'];

    showDialog(
      context: context,
      builder: (context) => AlertDialog(
        title: const Text('Zapisano plik'),
        content: Text('Plik: $fileName\n\n${content.length > 300 ? content.substring(0, 300) + '...' : content}'),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(context),
            child: const Text('OK'),
          ),
        ],
      ),
    );
  }

  void _updateStatus() {
    if (tabs.isEmpty || currentTab < 0 || currentTab >= tabs.length) {
      statusText = 'Brak otwartych plików';
    } else {
      final ctrl = tabs[currentTab]['controller'] as TextEditingController;
      final lines = ctrl.text.isEmpty ? 0 : ctrl.text.split('\n').length;
      statusText = 'Ln 1, Col 1 • $lines linii • UTF-8 • Spaces: 4';
    }
    setState(() {});
  }

  Widget _buildTab(int index) {
    final tab = tabs[index];
    final isActive = index == currentTab;

    return GestureDetector(
      onTap: () {
        currentTab = index;
        setState(() {});
        _updateStatus();
      },
      child: Container(
        height: 35,
        padding: const EdgeInsets.symmetric(horizontal: 16),
        color: isActive ? const Color(0xFF2D2D2D) : const Color(0xFF1E1E1E),
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            Text(
              tab['name'],
              style: TextStyle(
                color: isActive ? Colors.white : Colors.grey[400],
                fontSize: 13,
              ),
            ),
            const SizedBox(width: 12),
            GestureDetector(
              onTap: () => _closeTab(index),
              child: Icon(
                Icons.close,
                size: 16,
                color: isActive ? Colors.white : Colors.grey[500],
              ),
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
        backgroundColor: const Color(0xFF1E1E1E),
        title: const Text('Flutter Editor – Kate / VS Code'),
        actions: [
          IconButton(
            icon: const Icon(Icons.add),
            tooltip: 'Nowa zakładka',
            onPressed: _newFile,
          ),
          IconButton(
            icon: const Icon(Icons.save),
            tooltip: 'Zapisz',
            onPressed: _saveFile,
          ),
          const SizedBox(width: 8),
        ],
      ),
      body: Row(
        children: [
          // === PANEL BOCZNY – EKSPLORATOR ===
          Container(
            width: 260,
            color: const Color(0xFF252526),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                const Padding(
                  padding: EdgeInsets.all(12),
                  child: Text(
                    'EKSPLORATOR',
                    style: TextStyle(fontSize: 12, color: Colors.grey, letterSpacing: 1.5),
                  ),
                ),
                Expanded(
                  child: ListView.builder(
                    itemCount: explorerFiles.length,
                    itemBuilder: (context, index) {
                      return ListTile(
                        dense: true,
                        leading: const Icon(Icons.insert_drive_file, size: 18, color: Colors.blue),
                        title: Text(
                          explorerFiles[index],
                          style: const TextStyle(fontSize: 13),
                        ),
                        onTap: () => _openFile(explorerFiles[index]),
                      );
                    },
                  ),
                ),
              ],
            ),
          ),

          const VerticalDivider(width: 1, color: Colors.black),

          // === GŁÓWNA CZĘŚĆ ===
          Expanded(
            child: Column(
              children: [
                // === PASEK ZAKŁADEK ===
                Container(
                  height: 35,
                  color: const Color(0xFF1F1F1F),
                  child: SingleChildScrollView(
                    scrollDirection: Axis.horizontal,
                    child: Row(
                      children: List.generate(tabs.length, (i) => _buildTab(i)),
                    ),
                  ),
                ),

                // === EDYTOR ===
                Expanded(
                  child: tabs.isEmpty || currentTab < 0
                      ? const Center(
                          child: Text(
                            'Otwórz plik z eksploratora lub kliknij +',
                            style: TextStyle(color: Colors.grey, fontSize: 16),
                          ),
                        )
                      : Container(
                          padding: const EdgeInsets.all(12),
                          color: const Color(0xFF1E1E1E),
                          child: TextField(
                            controller: tabs[currentTab]['controller'],
                            maxLines: null,
                            expands: true,
                            style: const TextStyle(
                              fontFamily: 'monospace',
                              fontSize: 14,
                              height: 1.4,
                            ),
                            decoration: const InputDecoration(
                              border: InputBorder.none,
                              contentPadding: EdgeInsets.zero,
                              isDense: true,
                            ),
                            onChanged: (_) => _updateStatus(),
                          ),
                        ),
                ),

                // === PASEK STATUSU ===
                Container(
                  height: 28,
                  color: const Color(0xFF007ACC),
                  padding: const EdgeInsets.symmetric(horizontal: 12),
                  child: Row(
                    children: [
                      Text(
                        statusText,
                        style: const TextStyle(color: Colors.white, fontSize: 12),
                      ),
                      const Spacer(),
                      const Text(
                        'Powered by Flutter • Dart',
                        style: TextStyle(color: Colors.white70, fontSize: 12),
                      ),
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
