import 'rust/api.dart';

int pressedMaskForButtons(Iterable<ButtonType> buttons) {
  var mask = 0;

  for (final button in buttons) {
    mask |= switch (button) {
      ButtonType.a => 1 << 0,
      ButtonType.b => 1 << 1,
      ButtonType.start => 1 << 2,
      ButtonType.select => 1 << 3,
      ButtonType.up => 1 << 4,
      ButtonType.down => 1 << 5,
      ButtonType.left => 1 << 6,
      ButtonType.right => 1 << 7,
    };
  }

  return mask;
}
