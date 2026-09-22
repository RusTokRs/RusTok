import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:rustok_admin_mobile/registry/registry_adaptation_summary.dart';
import 'package:rustok_admin_mobile/routes/registry_warnings_card.dart';

void main() {
  Widget wrap(Widget child) => MaterialApp(home: Scaffold(body: child));

  testWidgets('renders nothing when summary has no warnings', (tester) async {
    await tester.pumpWidget(
      wrap(
        const RegistryWarningsCard(
          summary: RegistryAdaptationSummary(
            hasWarnings: false,
            message: 'Registry adaptation completed with no rejected entries.',
          ),
        ),
      ),
    );

    expect(find.text('Registry adaptation warnings'), findsNothing);
  });

  testWidgets('renders warning title and message when warnings exist', (
    tester,
  ) async {
    await tester.pumpWidget(
      wrap(
        const RegistryWarningsCard(
          summary: RegistryAdaptationSummary(
            hasWarnings: true,
            message: 'Rejected modules: 1 · Rejected child pages: 2',
          ),
        ),
      ),
    );

    expect(find.text('Registry adaptation warnings'), findsOneWidget);
    expect(
      find.text('Rejected modules: 1 · Rejected child pages: 2'),
      findsOneWidget,
    );
  });
}
