import 'package:flutter_gameboy/src/input_mask.dart';
import 'package:flutter_gameboy/src/rust/api.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  test('pressedMaskForButtons follows Rust joypad bit order', () {
    expect(pressedMaskForButtons([ButtonType.a]), 0x01);
    expect(pressedMaskForButtons([ButtonType.b]), 0x02);
    expect(pressedMaskForButtons([ButtonType.start]), 0x04);
    expect(pressedMaskForButtons([ButtonType.select]), 0x08);
    expect(pressedMaskForButtons([ButtonType.up]), 0x10);
    expect(pressedMaskForButtons([ButtonType.down]), 0x20);
    expect(pressedMaskForButtons([ButtonType.left]), 0x40);
    expect(pressedMaskForButtons([ButtonType.right]), 0x80);
  });

  test('pressedMaskForButtons combines multiple pressed buttons', () {
    expect(
      pressedMaskForButtons([ButtonType.start, ButtonType.left, ButtonType.a]),
      0x45,
    );
  });
}
