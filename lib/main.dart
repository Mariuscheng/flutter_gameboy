import 'dart:async';
import 'dart:typed_data';
import 'dart:ui' as ui;

import 'package:file_picker/file_picker.dart';
import 'package:flutter/material.dart';
import 'package:flutter/scheduler.dart';

import 'src/rust/api.dart';
import 'src/rust/frb_generated.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  await RustLib.init();
  runApp(const GameBoyApp());
}

class GameBoyApp extends StatelessWidget {
  const GameBoyApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'Flutter GameBoy',
      debugShowCheckedModeBanner: false,
      theme: ThemeData(
        colorScheme: const ColorScheme.light(
          primary: Color(0xFF1D402A),
          secondary: Color(0xFF8FA77E),
          surface: Color(0xFFE4EBD9),
        ),
        scaffoldBackgroundColor: const Color(0xFFD2D8C8),
        useMaterial3: true,
      ),
      home: const GameBoyHomePage(),
    );
  }
}

class GameBoyHomePage extends StatefulWidget {
  const GameBoyHomePage({super.key});

  @override
  State<GameBoyHomePage> createState() => _GameBoyHomePageState();
}

class _GameBoyHomePageState extends State<GameBoyHomePage>
    with SingleTickerProviderStateMixin {
  static const _screenWidth = 160;
  static const _screenHeight = 144;
  static const _emulatorFrameMicros = 16743;
  static const _maxCatchUpFrames = 4;

  GameBoyEmulator? _emulator;
  Ticker? _ticker;
  final ValueNotifier<ui.Image?> _frameImageNotifier = ValueNotifier(null);
  bool _loadingRom = false;
  bool _isRendering = false;
  Duration? _lastTick;
  int _pendingFrameMicros = 0;
  String? _status;

  @override
  void dispose() {
    _ticker?.dispose();
    _frameImageNotifier.value?.dispose();
    _frameImageNotifier.dispose();
    super.dispose();
  }

  Future<void> _pickRomAndStart() async {
    setState(() {
      _loadingRom = true;
      _status = null;
    });

    try {
      final result = await FilePicker.platform.pickFiles(
        withData: true,
        type: FileType.custom,
        allowedExtensions: const ['gb', 'gbc'],
      );

      if (result == null || result.files.isEmpty) {
        setState(() {
          _loadingRom = false;
          _status = '未選擇 ROM。';
        });
        return;
      }

      final bytes = result.files.single.bytes;
      if (bytes == null || bytes.isEmpty) {
        setState(() {
          _loadingRom = false;
          _status = 'ROM 內容為空，請重新選擇檔案。';
        });
        return;
      }

      _ticker?.dispose();
      _frameImageNotifier.value?.dispose();
      _frameImageNotifier.value = null;
      _lastTick = null;
      _pendingFrameMicros = _emulatorFrameMicros;

      final emulator = await GameBoyEmulator.newInstance(romBytes: bytes);
      _emulator = emulator;
      _ticker = createTicker((elapsed) {
        if (_lastTick == null) {
          _lastTick = elapsed;
          return;
        }

        final delta = elapsed - _lastTick!;
        _lastTick = elapsed;
        final clampedMicros = delta.inMicroseconds.clamp(
          0,
          _emulatorFrameMicros * _maxCatchUpFrames,
        );
        _pendingFrameMicros = (_pendingFrameMicros + clampedMicros).clamp(
          0,
          _emulatorFrameMicros * _maxCatchUpFrames,
        );
        unawaited(_renderNextFrame());
      })..start();

      setState(() {
        _loadingRom = false;
        _status = 'ROM 已載入：${result.files.single.name}';
      });
    } catch (error) {
      setState(() {
        _loadingRom = false;
        _status = '載入失敗：$error';
      });
    }
  }

  Future<void> _renderNextFrame() async {
    if (_isRendering) return;
    _isRendering = true;

    try {
      final emulator = _emulator;
      if (emulator == null || !mounted) {
        return;
      }

      var framesStepped = 0;
      while (_pendingFrameMicros >= _emulatorFrameMicros &&
          framesStepped < _maxCatchUpFrames) {
        await emulator.stepFrame();
        _pendingFrameMicros -= _emulatorFrameMicros;
        framesStepped += 1;
      }

      if (framesStepped == 0) {
        return;
      }

      final bytes = await emulator.getFrameBuffer();
      if (bytes.length < _screenWidth * _screenHeight * 4) {
        return;
      }

      final image = await _decodeFrame(bytes);
      if (!mounted) {
        image.dispose();
        return;
      }

      final previous = _frameImageNotifier.value;
      _frameImageNotifier.value = image;
      previous?.dispose();
    } catch (error) {
      _ticker?.stop();
      if (!mounted) {
        return;
      }
      setState(() {
        _status = '執行模擬器時發生錯誤：$error';
      });
    } finally {
      _isRendering = false;
      if (_pendingFrameMicros >= _emulatorFrameMicros && mounted) {
        unawaited(_renderNextFrame());
      }
    }
  }

  Future<ui.Image> _decodeFrame(Uint8List pixels) {
    final completer = Completer<ui.Image>();
    ui.decodeImageFromPixels(
      pixels,
      _screenWidth,
      _screenHeight,
      ui.PixelFormat.rgba8888,
      completer.complete,
    );
    return completer.future;
  }

  Future<void> _setButton(ButtonType button, bool pressed) async {
    final emulator = _emulator;
    if (emulator == null) {
      return;
    }

    if (pressed) {
      await emulator.pressButton(button: button);
    } else {
      await emulator.releaseButton(button: button);
    }
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: SafeArea(
        child: LayoutBuilder(
          builder: (context, constraints) {
            return SingleChildScrollView(
              padding: const EdgeInsets.all(20),
              child: ConstrainedBox(
                constraints: BoxConstraints(minHeight: constraints.maxHeight),
                child: Column(
                  children: [
                    Row(
                      children: [
                        Expanded(
                          child: Text(
                            'Flutter GameBoy',
                            style: Theme.of(context).textTheme.headlineMedium,
                          ),
                        ),
                        FilledButton(
                          onPressed: _loadingRom ? null : _pickRomAndStart,
                          child: Text(_loadingRom ? '載入中...' : '選擇 ROM'),
                        ),
                      ],
                    ),
                    const SizedBox(height: 16),
                    Container(
                      width: constraints.maxWidth,
                      padding: const EdgeInsets.all(20),
                      decoration: BoxDecoration(
                        color: const Color(0xFFB7C5A4),
                        borderRadius: BorderRadius.circular(28),
                        boxShadow: const [
                          BoxShadow(
                            color: Color(0x33000000),
                            blurRadius: 24,
                            offset: Offset(0, 10),
                          ),
                        ],
                      ),
                      child: AspectRatio(
                        aspectRatio: 10 / 9,
                        child: Container(
                          padding: const EdgeInsets.all(16),
                          decoration: BoxDecoration(
                            color: const Color(0xFF2A3329),
                            borderRadius: BorderRadius.circular(18),
                          ),
                          child: ClipRRect(
                            borderRadius: BorderRadius.circular(10),
                            child: ColoredBox(
                              color: const Color(0xFF8BAC0F),
                              child: ValueListenableBuilder<ui.Image?>(
                                valueListenable: _frameImageNotifier,
                                builder: (context, image, child) {
                                  return image == null
                                      ? const Center(
                                          child: Text(
                                            '請先載入 .gb ROM',
                                            style: TextStyle(
                                              color: Color(0xFF1F2A1A),
                                              fontWeight: FontWeight.w700,
                                            ),
                                          ),
                                        )
                                      : RawImage(
                                          image: image,
                                          fit: BoxFit.contain,
                                          filterQuality: FilterQuality.none,
                                        );
                                },
                              ),
                            ),
                          ),
                        ),
                      ),
                    ),
                    const SizedBox(height: 16),
                    if (_status != null)
                      Text(_status!, textAlign: TextAlign.center),
                    const SizedBox(height: 24),
                    Row(
                      mainAxisAlignment: MainAxisAlignment.spaceEvenly,
                      crossAxisAlignment: CrossAxisAlignment.center,
                      children: [_buildDPad(), _buildActionButtons()],
                    ),
                    const SizedBox(height: 32),
                    _buildSystemButtons(),
                    const SizedBox(height: 24),
                  ],
                ),
              ),
            );
          },
        ),
      ),
    );
  }

  Widget _buildDPad() {
    return SizedBox(
      width: 150,
      height: 150,
      child: Stack(
        alignment: Alignment.center,
        children: [
          // Background vertical structure
          Positioned(
            child: Container(
              width: 50,
              height: 150,
              decoration: BoxDecoration(
                color: const Color(0xFF2E3D31),
                borderRadius: BorderRadius.circular(8),
              ),
            ),
          ),
          // Background horizontal structure
          Positioned(
            child: Container(
              width: 150,
              height: 50,
              decoration: BoxDecoration(
                color: const Color(0xFF2E3D31),
                borderRadius: BorderRadius.circular(8),
              ),
            ),
          ),
          // Buttons
          Positioned(
            top: 0,
            child: _DPadDirection(
              icon: Icons.arrow_drop_up,
              onChanged: (pressed) => _setButton(ButtonType.up, pressed),
            ),
          ),
          Positioned(
            bottom: 0,
            child: _DPadDirection(
              icon: Icons.arrow_drop_down,
              onChanged: (pressed) => _setButton(ButtonType.down, pressed),
            ),
          ),
          Positioned(
            left: 0,
            child: _DPadDirection(
              icon: Icons.arrow_left,
              onChanged: (pressed) => _setButton(ButtonType.left, pressed),
            ),
          ),
          Positioned(
            right: 0,
            child: _DPadDirection(
              icon: Icons.arrow_right,
              onChanged: (pressed) => _setButton(ButtonType.right, pressed),
            ),
          ),
          // Center circle design element
          Positioned(
            child: Container(
              width: 30,
              height: 30,
              decoration: BoxDecoration(
                color: const Color(0xFF2E3D31),
                shape: BoxShape.circle,
                border: Border.all(color: const Color(0xFF1D2A1F), width: 2),
              ),
            ),
          ),
        ],
      ),
    );
  }

  Widget _buildActionButtons() {
    return SizedBox(
      width: 140,
      height: 120,
      child: Stack(
        children: [
          Positioned(
            bottom: 0,
            left: 0,
            child: _RoundButton(
              label: 'B',
              onChanged: (pressed) => _setButton(ButtonType.b, pressed),
            ),
          ),
          Positioned(
            top: 0,
            right: 0,
            child: _RoundButton(
              label: 'A',
              onChanged: (pressed) => _setButton(ButtonType.a, pressed),
            ),
          ),
        ],
      ),
    );
  }

  Widget _buildSystemButtons() {
    return Row(
      mainAxisAlignment: MainAxisAlignment.center,
      children: [
        _PillButton(
          label: 'SELECT',
          onChanged: (pressed) => _setButton(ButtonType.select, pressed),
        ),
        const SizedBox(width: 48),
        _PillButton(
          label: 'START',
          onChanged: (pressed) => _setButton(ButtonType.start, pressed),
        ),
      ],
    );
  }
}

class _DPadDirection extends StatelessWidget {
  const _DPadDirection({required this.icon, required this.onChanged});

  final IconData icon;
  final ValueChanged<bool> onChanged;

  @override
  Widget build(BuildContext context) {
    return Listener(
      onPointerDown: (_) => onChanged(true),
      onPointerUp: (_) => onChanged(false),
      onPointerCancel: (_) => onChanged(false),
      child: Container(
        width: 50,
        height: 50,
        color: Colors.transparent, // Invisible click area
        alignment: Alignment.center,
        child: Icon(icon, color: Colors.white, size: 40),
      ),
    );
  }
}

class _RoundButton extends StatelessWidget {
  const _RoundButton({required this.label, required this.onChanged});

  final String label;
  final ValueChanged<bool> onChanged;

  @override
  Widget build(BuildContext context) {
    return Listener(
      onPointerDown: (_) => onChanged(true),
      onPointerUp: (_) => onChanged(false),
      onPointerCancel: (_) => onChanged(false),
      child: Container(
        width: 64,
        height: 64,
        alignment: Alignment.center,
        decoration: const BoxDecoration(
          color: Color(0xFF8B2C42),
          shape: BoxShape.circle,
        ),
        child: Text(
          label,
          style: const TextStyle(
            color: Colors.white,
            fontSize: 28,
            fontWeight: FontWeight.w800,
          ),
        ),
      ),
    );
  }
}

class _PillButton extends StatelessWidget {
  const _PillButton({required this.label, required this.onChanged});

  final String label;
  final ValueChanged<bool> onChanged;

  @override
  Widget build(BuildContext context) {
    return Column(
      mainAxisSize: MainAxisSize.min,
      children: [
        Listener(
          onPointerDown: (_) => onChanged(true),
          onPointerUp: (_) => onChanged(false),
          onPointerCancel: (_) => onChanged(false),
          child: Transform.rotate(
            angle: -0.4,
            child: Container(
              width: 64,
              height: 20,
              decoration: BoxDecoration(
                color: const Color(0xFF2E3D31),
                borderRadius: BorderRadius.circular(10),
              ),
            ),
          ),
        ),
        const SizedBox(height: 8),
        Text(
          label,
          style: const TextStyle(
            color: Color(0xFF314D36),
            fontSize: 14,
            fontWeight: FontWeight.w900,
            letterSpacing: 2,
          ),
        ),
      ],
    );
  }
}
