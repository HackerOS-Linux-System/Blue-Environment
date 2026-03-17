import 'dart:io';
import 'package:flutter/material.dart';
import 'package:just_audio/just_audio.dart';
import 'package:audio_video_progress_bar/audio_video_progress_bar.dart';
import 'package:file_picker/file_picker.dart';
import 'package:path/path.dart' as p;

void main() {
  runApp(const BlueMusicApp());
}

class BlueMusicApp extends StatelessWidget {
  const BlueMusicApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'Blue Music',
      debugShowCheckedModeBanner: false,
      theme: ThemeData.dark().copyWith(
        primaryColor: Colors.blue.shade400,
        scaffoldBackgroundColor: const Color(0xFF0A0E1A),
        appBarTheme: const AppBarTheme(
          backgroundColor: Color(0xFF0A0E1A),
          elevation: 0,
        ),
      ),
      home: const PlayerScreen(),
    );
  }
}

class PlayerScreen extends StatefulWidget {
  const PlayerScreen({super.key});

  @override
  State<PlayerScreen> createState() => _PlayerScreenState();
}

class _PlayerScreenState extends State<PlayerScreen> {
  final AudioPlayer _audioPlayer = AudioPlayer();
  List<AudioSource> _playlist = [];
  int _currentIndex = -1;
  bool _isPlaying = false;
  Duration _duration = Duration.zero;
  Duration _position = Duration.zero;

  @override
  void initState() {
    super.initState();
    _audioPlayer.playerStateStream.listen((state) {
      setState(() => _isPlaying = state.playing);
    });

    _audioPlayer.durationStream.listen((d) {
      if (d != null) setState(() => _duration = d);
    });

    _audioPlayer.positionStream.listen((p) {
      setState(() => _position = p);
    });

    _audioPlayer.processingStateStream.listen((state) {
      if (state == ProcessingState.completed) {
        _playNext();
      }
    });
  }

  @override
  void dispose() {
    _audioPlayer.dispose();
    super.dispose();
  }

  Future<void> _pickFiles() async {
    FilePickerResult? result = await FilePicker.platform.pickFiles(
      type: FileType.audio,
      allowMultiple: true,
    );

    if (result != null) {
      final sources = result.paths
          .whereType<String>()
          .map((path) => AudioSource.file(path))
          .toList();

      setState(() {
        _playlist = sources;
        _currentIndex = 0;
      });

      await _playCurrent();
    }
  }

  Future<void> _pickFolder() async {
    String? selectedDirectory = await FilePicker.platform.getDirectoryPath();

    if (selectedDirectory != null) {
      final dir = Directory(selectedDirectory);
      final audioFiles = dir
          .listSync(recursive: true)
          .whereType<File>()
          .where((file) =>
              ['.mp3', '.flac', '.wav', '.ogg', '.m4a']
                  .contains(p.extension(file.path).toLowerCase()))
          .toList();

      if (audioFiles.isEmpty) {
        ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(content: Text('Nie znaleziono plików audio w folderze')),
        );
        return;
      }

      final sources = audioFiles
          .map((file) => AudioSource.file(file.path))
          .toList();

      setState(() {
        _playlist = sources;
        _currentIndex = 0;
      });

      await _playCurrent();
    }
  }

  Future<void> _playCurrent() async {
    if (_playlist.isEmpty || _currentIndex < 0) return;

    await _audioPlayer.setAudioSource(_playlist[_currentIndex]);
    await _audioPlayer.play();
  }

  void _playPause() {
    if (_isPlaying) {
      _audioPlayer.pause();
    } else {
      _audioPlayer.play();
    }
  }

  void _playNext() {
    if (_playlist.isEmpty) return;
    setState(() {
      _currentIndex = (_currentIndex + 1) % _playlist.length;
    });
    _playCurrent();
  }

  void _playPrevious() {
    if (_playlist.isEmpty) return;
    setState(() {
      _currentIndex = (_currentIndex - 1 + _playlist.length) % _playlist.length;
    });
    _playCurrent();
  }

  String _formatDuration(Duration d) {
    String twoDigits(int n) => n.toString().padLeft(2, '0');
    final hours = twoDigits(d.inHours);
    final minutes = twoDigits(d.inMinutes.remainder(60));
    final seconds = twoDigits(d.inSeconds.remainder(60));
    return d.inHours > 0 ? '$hours:$minutes:$seconds' : '$minutes:$seconds';
  }

  @override
  Widget build(BuildContext context) {
    final currentFileName = _playlist.isNotEmpty && _currentIndex >= 0
        ? p.basename(_playlist[_currentIndex].uri.toString())
        : 'Brak utworu';

    return Scaffold(
      appBar: AppBar(
        title: const Text('Blue Music'),
        centerTitle: true,
      ),
      body: Padding(
        padding: const EdgeInsets.all(24.0),
        child: Column(
          children: [
            // Ikona / okładka (można później dodać album art)
            Container(
              width: 220,
              height: 220,
              decoration: BoxDecoration(
                color: Colors.blue.shade900.withOpacity(0.3),
                borderRadius: BorderRadius.circular(20),
              ),
              child: const Icon(
                Icons.music_note,
                size: 120,
                color: Colors.blue,
              ),
            ),

            const SizedBox(height: 40),

            // Nazwa pliku
            Text(
              currentFileName,
              style: const TextStyle(fontSize: 18, fontWeight: FontWeight.w500),
              textAlign: TextAlign.center,
              maxLines: 2,
              overflow: TextOverflow.ellipsis,
            ),

            const SizedBox(height: 40),

            // Progress bar
            ProgressBar(
              progress: _position,
              total: _duration,
              onSeek: (duration) => _audioPlayer.seek(duration),
              barHeight: 6,
              thumbRadius: 10,
              timeLabelLocation: TimeLabelLocation.sides,
              timeLabelTextStyle: const TextStyle(color: Colors.white70),
            ),

            const SizedBox(height: 30),

            // Przyciski sterowania
            Row(
              mainAxisAlignment: MainAxisAlignment.center,
              children: [
                IconButton(
                  iconSize: 48,
                  icon: const Icon(Icons.skip_previous),
                  onPressed: _playPrevious,
                ),
                const SizedBox(width: 20),
                IconButton(
                  iconSize: 72,
                  icon: Icon(_isPlaying ? Icons.pause_circle : Icons.play_circle),
                  color: Colors.blue.shade400,
                  onPressed: _playPause,
                ),
                const SizedBox(width: 20),
                IconButton(
                  iconSize: 48,
                  icon: const Icon(Icons.skip_next),
                  onPressed: _playNext,
                ),
              ],
            ),

            const SizedBox(height: 40),

            // Przyciski wyboru plików
            Row(
              mainAxisAlignment: MainAxisAlignment.center,
              children: [
                ElevatedButton.icon(
                  onPressed: _pickFiles,
                  icon: const Icon(Icons.add),
                  label: const Text('Dodaj pliki'),
                  style: ElevatedButton.styleFrom(
                    backgroundColor: Colors.blue.shade700,
                    padding: const EdgeInsets.symmetric(horizontal: 24, vertical: 16),
                  ),
                ),
                const SizedBox(width: 16),
                ElevatedButton.icon(
                  onPressed: _pickFolder,
                  icon: const Icon(Icons.folder_open),
                  label: const Text('Otwórz folder'),
                  style: ElevatedButton.styleFrom(
                    backgroundColor: Colors.blue.shade700,
                    padding: const EdgeInsets.symmetric(horizontal: 24, vertical: 16),
                  ),
                ),
              ],
            ),

            const Spacer(),

            // Lista playlisty (prosta)
            if (_playlist.isNotEmpty)
              Expanded(
                child: ListView.builder(
                  itemCount: _playlist.length,
                  itemBuilder: (context, index) {
                    final name = p.basename(_playlist[index].uri.toString());
                    return ListTile(
                      leading: Icon(
                        index == _currentIndex ? Icons.play_arrow : Icons.music_note,
                        color: index == _currentIndex ? Colors.blue : Colors.white70,
                      ),
                      title: Text(
                        name,
                        style: TextStyle(
                          color: index == _currentIndex ? Colors.blue : null,
                        ),
                      ),
                      onTap: () {
                        setState(() => _currentIndex = index);
                        _playCurrent();
                      },
                    );
                  },
                ),
              ),
          ],
        ),
      ),
    );
  }
}
