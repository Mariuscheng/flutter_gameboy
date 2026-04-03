import 'package:flutter_test/flutter_test.dart';

import 'package:flutter_gameboy/main.dart';

void main() {
  testWidgets('GameBoy shell renders', (WidgetTester tester) async {
    await tester.pumpWidget(const GameBoyApp());

    expect(find.text('Flutter GameBoy'), findsOneWidget);
    expect(find.text('選擇 ROM'), findsOneWidget);
    expect(find.text('請先載入 .gb ROM'), findsOneWidget);
  });
}
