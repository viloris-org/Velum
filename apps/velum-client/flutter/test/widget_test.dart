import 'package:flutter_test/flutter_test.dart';
import 'package:velum_client/main.dart';

void main() {
  testWidgets('shows the disconnected client control surface', (tester) async {
    await tester.pumpWidget(const VelumClientApp());

    expect(find.text('Client control'), findsOneWidget);
    expect(find.text('Disconnected'), findsOneWidget);
    expect(find.text('Activity'), findsOneWidget);
    expect(find.text('Connect'), findsOneWidget);
  });
}
