import 'dart:async';
import 'dart:ui' as ui;

import 'package:file_picker/file_picker.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter/scheduler.dart';
import 'package:flutter/services.dart';

import 'src/input_mask.dart';
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
    with TickerProviderStateMixin {
  static const _screenWidth = 160;
  static const _screenHeight = 144;
  static const _emulatorFrameMicros = 16743;
  static const _maxCatchUpFrames = 4;
  static const _mobileDisplayFrameMicros = 33333;

  GameBoyEmulator? _emulator;
  Ticker? _ticker;
  final FocusNode _focusNode = FocusNode(debugLabel: 'gameboy-input');
  final ValueNotifier<ui.Image?> _frameImageNotifier = ValueNotifier(null);
  final ScrollController _scrollController = ScrollController();
  bool _loadingRom = false;
  bool _isRendering = false;
  Duration? _lastTick;
  int _pendingFrameMicros = 0;
  int _pendingDisplayMicros = 0;
  final Set<ButtonType> _touchButtons = <ButtonType>{};
  final Set<ButtonType> _dpadButtons = <ButtonType>{};
  final Set<ButtonType> _keyboardButtons = <ButtonType>{};
  int _inputRevision = 0;
  int _lastSyncedMask = -1;
  bool _inputDirty = true;
  int _emulatorSession = 0;
  String? _status;

  bool get _shouldReadRomBytes {
    if (kIsWeb) {
      return true;
    }

    return switch (defaultTargetPlatform) {
      TargetPlatform.android || TargetPlatform.iOS => true,
      _ => false,
    };
  }

  bool get _shouldEnableScroll {
    if (kIsWeb) {
      return true;
    }

    return switch (defaultTargetPlatform) {
      TargetPlatform.windows ||
      TargetPlatform.linux ||
      TargetPlatform.macOS => true,
      _ => false,
    };
  }

  int get _targetDisplayMicros {
    if (kIsWeb) {
      return _emulatorFrameMicros;
    }
    return switch (defaultTargetPlatform) {
      TargetPlatform.android || TargetPlatform.iOS => _mobileDisplayFrameMicros,
      _ => _emulatorFrameMicros,
    };
  }

  @override
  void dispose() {
    _ticker?.dispose();
    _focusNode.dispose();
    _scrollController.dispose();
    _frameImageNotifier.value?.dispose();
    _frameImageNotifier.dispose();
    super.dispose();
  }

  void _requestInputFocus() {
    if (!mounted) {
      return;
    }

    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted) {
        _focusNode.requestFocus();
      }
    });
  }

  Future<void> _pickRomAndStart() async {
    final session = ++_emulatorSession;

    setState(() {
      _loadingRom = true;
      _status = null;
    });

    try {
      final result = await FilePicker.platform.pickFiles(
        withData: _shouldReadRomBytes,
        type: FileType.custom,
        allowedExtensions: const ['gb', 'gbc'],
      );

      if (result == null || result.files.isEmpty) {
        setState(() {
          _loadingRom = false;
          _status = '未選擇 ROM。';
        });
        _requestInputFocus();
        return;
      }

      final selectedFile = result.files.single;
      final bytes = selectedFile.bytes;
      final path = selectedFile.path;

      if (_shouldReadRomBytes) {
        if (bytes == null || bytes.isEmpty) {
          setState(() {
            _loadingRom = false;
            _status = 'ROM 內容為空，請重新選擇檔案。';
          });
          _requestInputFocus();
          return;
        }
      } else if (path == null || path.isEmpty) {
        setState(() {
          _loadingRom = false;
          _status = '找不到 ROM 檔案路徑，請重新選擇檔案。';
        });
        _requestInputFocus();
        return;
      }

      _emulator = null;
      _ticker?.dispose();
      _ticker = null;
      _frameImageNotifier.value?.dispose();
      _frameImageNotifier.value = null;
      _lastTick = null;
      _isRendering = false;
      _pendingFrameMicros = _emulatorFrameMicros;
      _pendingDisplayMicros = _targetDisplayMicros;
      _touchButtons.clear();
      _dpadButtons.clear();
      _keyboardButtons.clear();
      _inputRevision = 0;
      _lastSyncedMask = -1;
      _inputDirty = true;

      final emulator = _shouldReadRomBytes
          ? await GameBoyEmulator.newInstance(romBytes: bytes!)
          : await RustLib.instance.api.crateApiGameBoyEmulatorNewFromPath(
              path: path!,
            );

      if (!mounted || session != _emulatorSession) {
        return;
      }

      _emulator = emulator;
      _ticker = createTicker((elapsed) {
        if (session != _emulatorSession) {
          return;
        }

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
        unawaited(_renderNextFrame(session));
      })..start();

      setState(() {
        _loadingRom = false;
        _status = 'ROM 已載入：${selectedFile.name}';
      });
      _requestInputFocus();
    } catch (error) {
      setState(() {
        _loadingRom = false;
        _status = '載入失敗：$error';
      });
      _requestInputFocus();
    }
  }

  Future<void> _renderNextFrame(int session) async {
    if (_isRendering) return;
    _isRendering = true;

    try {
      if (session != _emulatorSession) {
        return;
      }

      final emulator = _emulator;
      if (emulator == null || !mounted) {
        return;
      }

      var framesStepped = 0;
      while (_pendingFrameMicros >= _emulatorFrameMicros &&
          framesStepped < _maxCatchUpFrames) {
        final pressedMask = _currentPressedMask();
        if (_inputDirty || pressedMask != _lastSyncedMask) {
          _inputRevision += 1;
          await emulator.syncButtons(
            pressedMask: pressedMask,
            revision: _inputRevision,
          );
          _lastSyncedMask = pressedMask;
          _inputDirty = false;
        }

        await emulator.stepFrame();
        _pendingFrameMicros -= _emulatorFrameMicros;
        _pendingDisplayMicros += _emulatorFrameMicros;
        framesStepped += 1;
      }

      if (framesStepped == 0) {
        return;
      }

      final shouldRefreshImage =
          _frameImageNotifier.value == null ||
          _pendingDisplayMicros >= _targetDisplayMicros;
      if (!shouldRefreshImage) {
        return;
      }

      _pendingDisplayMicros %= _targetDisplayMicros;

      final bytes = await emulator.getFrameBuffer();
      if (session != _emulatorSession) {
        return;
      }

      if (bytes.length < _screenWidth * _screenHeight * 4) {
        return;
      }

      final image = await _decodeFrame(bytes);
      if (session != _emulatorSession) {
        image.dispose();
        return;
      }

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
      if (session == _emulatorSession &&
          _pendingFrameMicros >= _emulatorFrameMicros &&
          mounted) {
        unawaited(_renderNextFrame(session));
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

  int _currentPressedMask() {
    return pressedMaskForButtons({
      ..._touchButtons,
      ..._dpadButtons,
      ..._keyboardButtons,
    });
  }

  void _syncButtons() {
    _inputDirty = true;
    if (kDebugMode && mounted) {
      setState(() {});
    }
  }

  void _setButton(ButtonType button, bool pressed) {
    _requestInputFocus();

    if (pressed) {
      if (!_touchButtons.add(button)) {
        return;
      }
    } else {
      if (!_touchButtons.remove(button)) {
        return;
      }
    }

    _syncButtons();
  }

  void _setDPadButtons(Set<ButtonType> buttons) {
    _requestInputFocus();

    if (setEquals(buttons, _dpadButtons)) {
      return;
    }

    _dpadButtons
      ..clear()
      ..addAll(buttons);

    _syncButtons();
  }

  ButtonType? _buttonForKey(LogicalKeyboardKey key) {
    if (key == LogicalKeyboardKey.arrowUp || key == LogicalKeyboardKey.keyW) {
      return ButtonType.up;
    }
    if (key == LogicalKeyboardKey.arrowDown || key == LogicalKeyboardKey.keyS) {
      return ButtonType.down;
    }
    if (key == LogicalKeyboardKey.arrowLeft || key == LogicalKeyboardKey.keyA) {
      return ButtonType.left;
    }
    if (key == LogicalKeyboardKey.arrowRight ||
        key == LogicalKeyboardKey.keyD) {
      return ButtonType.right;
    }
    if (key == LogicalKeyboardKey.keyJ || key == LogicalKeyboardKey.keyZ) {
      return ButtonType.a;
    }
    if (key == LogicalKeyboardKey.keyK || key == LogicalKeyboardKey.keyX) {
      return ButtonType.b;
    }
    if (key == LogicalKeyboardKey.enter) {
      return ButtonType.start;
    }
    if (key == LogicalKeyboardKey.shiftLeft ||
        key == LogicalKeyboardKey.shiftRight ||
        key == LogicalKeyboardKey.space) {
      return ButtonType.select;
    }

    return null;
  }

  KeyEventResult _handleKeyEvent(FocusNode node, KeyEvent event) {
    final button = _buttonForKey(event.logicalKey);
    if (button == null) {
      return KeyEventResult.ignored;
    }

    var changed = false;
    if (event is KeyDownEvent || event is KeyRepeatEvent) {
      changed = _keyboardButtons.add(button);
    } else if (event is KeyUpEvent) {
      changed = _keyboardButtons.remove(button);
    }

    if (changed) {
      _syncButtons();
    }

    return KeyEventResult.handled;
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: Focus(
        autofocus: true,
        canRequestFocus: true,
        descendantsAreFocusable: false,
        focusNode: _focusNode,
        onFocusChange: (_) {
          if (kDebugMode && mounted) {
            setState(() {});
          }
        },
        onKeyEvent: _handleKeyEvent,
        child: Listener(
          behavior: HitTestBehavior.translucent,
          onPointerDown: (_) => _requestInputFocus(),
          child: SafeArea(
            child: LayoutBuilder(
              builder: (context, constraints) {
                return Scrollbar(
                  controller: _scrollController,
                  thumbVisibility: _shouldEnableScroll,
                  child: SingleChildScrollView(
                    controller: _scrollController,
                    physics: _shouldEnableScroll
                        ? const ClampingScrollPhysics()
                        : const NeverScrollableScrollPhysics(),
                    padding: const EdgeInsets.all(20),
                    child: ConstrainedBox(
                      constraints: BoxConstraints(
                        minHeight: constraints.maxHeight,
                      ),
                      child: Column(
                        children: [
                          Row(
                            children: [
                              Expanded(
                                child: Text(
                                  'Flutter GameBoy',
                                  style: Theme.of(
                                    context,
                                  ).textTheme.headlineMedium,
                                ),
                              ),
                              FilledButton(
                                onPressed: _loadingRom
                                    ? null
                                    : _pickRomAndStart,
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
                                                filterQuality:
                                                    FilterQuality.none,
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
                  ),
                );
              },
            ),
          ),
        ),
      ),
    );
  }

  Widget _buildDPad() {
    return _DPadControl(onChanged: _setDPadButtons);
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

class _DPadControl extends StatefulWidget {
  const _DPadControl({required this.onChanged});

  final ValueChanged<Set<ButtonType>> onChanged;

  @override
  State<_DPadControl> createState() => _DPadControlState();
}

class _DPadControlState extends State<_DPadControl> {
  static const _size = 150.0;
  static const _deadZone = 18.0;
  static const _axisThreshold = 24.0;

  final Map<int, Set<ButtonType>> _pointerDirections = <int, Set<ButtonType>>{};

  void _updatePointer(int pointer, Offset position) {
    _pointerDirections[pointer] = _directionsForPosition(position);
    _notify();
  }

  void _removePointer(int pointer) {
    if (_pointerDirections.remove(pointer) != null) {
      _notify();
    }
  }

  void _notify() {
    final active = <ButtonType>{};
    for (final directions in _pointerDirections.values) {
      active.addAll(directions);
    }
    widget.onChanged(active);
  }

  Set<ButtonType> _directionsForPosition(Offset position) {
    final dx = position.dx - (_size / 2);
    final dy = position.dy - (_size / 2);

    if (dx.abs() < _deadZone && dy.abs() < _deadZone) {
      return <ButtonType>{};
    }

    final result = <ButtonType>{};
    if (dx <= -_axisThreshold) {
      result.add(ButtonType.left);
    } else if (dx >= _axisThreshold) {
      result.add(ButtonType.right);
    }

    if (dy <= -_axisThreshold) {
      result.add(ButtonType.up);
    } else if (dy >= _axisThreshold) {
      result.add(ButtonType.down);
    }

    if (result.isEmpty) {
      if (dx.abs() >= dy.abs()) {
        result.add(dx < 0 ? ButtonType.left : ButtonType.right);
      } else {
        result.add(dy < 0 ? ButtonType.up : ButtonType.down);
      }
    }

    return result;
  }

  @override
  Widget build(BuildContext context) {
    return Listener(
      behavior: HitTestBehavior.opaque,
      onPointerDown: (event) =>
          _updatePointer(event.pointer, event.localPosition),
      onPointerMove: (event) =>
          _updatePointer(event.pointer, event.localPosition),
      onPointerUp: (event) => _removePointer(event.pointer),
      onPointerCancel: (event) => _removePointer(event.pointer),
      child: SizedBox(
        width: _size,
        height: _size,
        child: Stack(
          alignment: Alignment.center,
          children: [
            Positioned(
              child: Container(
                width: 50,
                height: _size,
                decoration: BoxDecoration(
                  color: const Color(0xFF2E3D31),
                  borderRadius: BorderRadius.circular(8),
                ),
              ),
            ),
            Positioned(
              child: Container(
                width: _size,
                height: 50,
                decoration: BoxDecoration(
                  color: const Color(0xFF2E3D31),
                  borderRadius: BorderRadius.circular(8),
                ),
              ),
            ),
            const Positioned(
              top: 0,
              child: Icon(Icons.arrow_drop_up, color: Colors.white, size: 40),
            ),
            const Positioned(
              bottom: 0,
              child: Icon(Icons.arrow_drop_down, color: Colors.white, size: 40),
            ),
            const Positioned(
              left: 0,
              child: Icon(Icons.arrow_left, color: Colors.white, size: 40),
            ),
            const Positioned(
              right: 0,
              child: Icon(Icons.arrow_right, color: Colors.white, size: 40),
            ),
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
    return _MultiTouchButton(
      onChanged: onChanged,
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
        _MultiTouchButton(
          onChanged: onChanged,
          child: SizedBox(
            width: 116,
            height: 64,
            child: Center(
              child: Transform.rotate(
                angle: -0.4,
                child: Container(
                  width: 100,
                  height: 28,
                  alignment: Alignment.center,
                  decoration: BoxDecoration(
                    color: const Color(0xFF2E3D31),
                    borderRadius: BorderRadius.circular(14),
                    boxShadow: const [
                      BoxShadow(
                        color: Color(0x22000000),
                        blurRadius: 6,
                        offset: Offset(0, 2),
                      ),
                    ],
                  ),
                  child: Text(
                    label,
                    style: const TextStyle(
                      color: Color(0xFFE4EBD9),
                      fontSize: 12,
                      fontWeight: FontWeight.w800,
                      letterSpacing: 1.2,
                    ),
                  ),
                ),
              ),
            ),
          ),
        ),
      ],
    );
  }
}

class _MultiTouchButton extends StatefulWidget {
  const _MultiTouchButton({required this.child, required this.onChanged});

  final Widget child;
  final ValueChanged<bool> onChanged;

  @override
  State<_MultiTouchButton> createState() => _MultiTouchButtonState();
}

class _MultiTouchButtonState extends State<_MultiTouchButton> {
  final Set<int> _activePointers = <int>{};

  void _press(int pointer) {
    final wasEmpty = _activePointers.isEmpty;
    _activePointers.add(pointer);
    if (wasEmpty) {
      widget.onChanged(true);
    }
  }

  void _release(int pointer) {
    final removed = _activePointers.remove(pointer);
    if (removed && _activePointers.isEmpty) {
      widget.onChanged(false);
    }
  }

  @override
  Widget build(BuildContext context) {
    return Listener(
      behavior: HitTestBehavior.opaque,
      onPointerDown: (event) => _press(event.pointer),
      onPointerUp: (event) => _release(event.pointer),
      onPointerCancel: (event) => _release(event.pointer),
      child: widget.child,
    );
  }
}
