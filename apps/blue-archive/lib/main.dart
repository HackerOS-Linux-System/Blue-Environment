import 'dart:io';
import 'dart:typed_data';

import 'package:archive/archive.dart';
import 'package:file_picker/file_picker.dart';
import 'package:flutter/material.dart';
import 'package:path/path.dart' as p;

void main() {
  runApp(const BlueArchiveApp());
}

class BlueArchiveApp extends StatelessWidget {
  const BlueArchiveApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'Blue Archive',
      theme: ThemeData(
        colorScheme: ColorScheme.fromSeed(seedColor: Colors.blue, brightness: Brightness.dark),
        useMaterial3: true,
      ),
      home: const ArchiveScreen(),
      debugShowCheckedModeBanner: false,
    );
  }
}

class ArchiveScreen extends StatefulWidget {
  const ArchiveScreen({super.key});

  @override
  State<ArchiveScreen> createState() => _ArchiveScreenState();
}

class _ArchiveScreenState extends State<ArchiveScreen> {
  String? _archivePath;
  List<ArchiveFile> _contents = [];
  String _status = 'Gotowy do pracy';

  Future<void> _pickArchive() async {
    final result = await FilePicker.platform.pickFiles(
      type: FileType.custom,
      allowedExtensions: ['zip', 'tar', 'gz', 'tgz'],
    );

    if (result == null || result.files.single.path == null) return;

    setState(() {
      _archivePath = result.files.single.path;
      _status = 'Ładowanie archiwum...';
    });

    await _loadArchive();
  }

  Future<void> _loadArchive() async {
    if (_archivePath == null) return;

    try {
      final bytes = await File(_archivePath!).readAsBytes();
      final fileName = p.basename(_archivePath!).toLowerCase();

      Archive? archive;

      if (fileName.endsWith('.zip')) {
        archive = ZipDecoder().decodeBytes(bytes);
      } else if (fileName.endsWith('.tar')) {
        archive = TarDecoder().decodeBytes(bytes);
      } else if (fileName.endsWith('.tar.gz') || fileName.endsWith('.tgz')) {
        final decompressed = GZipDecoder().decodeBytes(bytes);
        archive = TarDecoder().decodeBytes(decompressed);
      } else {
        throw Exception('Nieobsługiwany format');
      }

      setState(() {
        _contents = archive!.files.where((f) => f.name.isNotEmpty).toList();
        _status = 'Załadowano ${_contents.length} elementów';
      });
    } catch (e) {
      setState(() => _status = 'Błąd: $e');
    }
  }

  Future<void> _extractArchive() async {
    if (_contents.isEmpty) return;

    final extractPath = await FilePicker.platform.getDirectoryPath();
    if (extractPath == null) return;

    setState(() => _status = 'Wyodrębnianie...');

    try {
      for (final file in _contents) {
        final targetPath = p.join(extractPath, file.name);

        if (file.name.endsWith('/')) {
          // katalog
          await Directory(targetPath).create(recursive: true);
        } else {
          // plik
          final outFile = File(targetPath);
          await outFile.parent.create(recursive: true);
          final content = file.content as List<int>?;
          if (content != null) {
            await outFile.writeAsBytes(content);
          }
        }
      }

      setState(() => _status = 'Gotowe! Wyodrębniono do:\n$extractPath');
    } catch (e) {
      setState(() => _status = 'Błąd wyodrębniania: $e');
    }
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: const Text('Blue Archive'),
        backgroundColor: Colors.blue.shade900,
        foregroundColor: Colors.white,
      ),
      body: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            ElevatedButton.icon(
              onPressed: _pickArchive,
              icon: const Icon(Icons.folder_open),
              label: const Text('Wybierz archiwum (.zip / .tar / .tar.gz / .tgz)'),
              style: ElevatedButton.styleFrom(
                backgroundColor: Colors.blue,
                foregroundColor: Colors.white,
                padding: const EdgeInsets.symmetric(horizontal: 24, vertical: 16),
              ),
            ),
            const SizedBox(height: 16),
            if (_archivePath != null)
              Text(
                'Wybrane: $_archivePath',
                style: const TextStyle(fontSize: 14, color: Colors.blueAccent),
              ),
            const SizedBox(height: 8),
            Text(_status, style: const TextStyle(fontWeight: FontWeight.bold)),
            const Divider(),
            Expanded(
              child: _contents.isEmpty
                  ? const Center(
                      child: Text(
                        'Brak zawartości\nWybierz archiwum powyżej',
                        textAlign: TextAlign.center,
                        style: TextStyle(color: Colors.grey),
                      ),
                    )
                  : ListView.builder(
                      itemCount: _contents.length,
                      itemBuilder: (context, index) {
                        final file = _contents[index];
                        final size = file.content != null
                            ? (file.content as List<int>).length
                            : 0;
                        return ListTile(
                          leading: Icon(
                            file.name.endsWith('/') ? Icons.folder : Icons.insert_drive_file,
                            color: Colors.blueAccent,
                          ),
                          title: Text(file.name),
                          subtitle: Text('$size B'),
                          dense: true,
                        );
                      },
                    ),
            ),
            if (_contents.isNotEmpty)
              SizedBox(
                width: double.infinity,
                child: ElevatedButton.icon(
                  onPressed: _extractArchive,
                  icon: const Icon(Icons.save_alt),
                  label: const Text('WYODRĘBNIJ WSZYSTKO'),
                  style: ElevatedButton.styleFrom(
                    backgroundColor: Colors.green,
                    foregroundColor: Colors.white,
                    padding: const EdgeInsets.symmetric(vertical: 16),
                  ),
                ),
              ),
          ],
        ),
      ),
    );
  }
}
