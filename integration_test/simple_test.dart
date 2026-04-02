import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_gameboy/main.dart';
import 'package:integration_test/integration_test.dart';

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();
  testWidgets('GameBoy home renders', (WidgetTester tester) async {
    await tester.pumpWidget(const GameBoyApp());
    await tester.pumpAndSettle();

    expect(find.text('Flutter GameBoy'), findsOneWidget);
    expect(find.text('選擇 ROM'), findsOneWidget);
  });
}
