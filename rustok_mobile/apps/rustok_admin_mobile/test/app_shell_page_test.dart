import 'dart:async';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:rustok_admin_mobile/app_shell/app_shell_page.dart';
import 'package:rustok_admin_mobile/app_shell/auth_bootstrap.dart';

void main() {
  Widget _wrap({
    required dynamic override,
    Widget? child,
  }) {
    return ProviderScope(
      overrides: [override],
      child: MaterialApp(
        home: AppShellPage(
          child: child ?? const Text('module-content'),
        ),
      ),
    );
  }

  testWidgets('shows loading while bootstrap probe is pending', (tester) async {
    final completer = Completer<BootstrapProbeResult>();
    await tester.pumpWidget(
      _wrap(
        override: authBootstrapProbeProvider.overrideWith(
          (ref) => completer.future,
        ),
      ),
    );

    expect(find.byType(CircularProgressIndicator), findsOneWidget);
    completer.complete(const BootstrapProbeResult.unauthenticated());
    await tester.pumpAndSettle();
  });

  testWidgets('shows unauthenticated message', (tester) async {
    await tester.pumpWidget(
      _wrap(
        override: authBootstrapProbeProvider.overrideWith(
          (ref) async => const BootstrapProbeResult.unauthenticated(),
        ),
      ),
    );
    await tester.pumpAndSettle();

    expect(find.textContaining('No active session'), findsOneWidget);
    expect(find.text('module-content'), findsNothing);
  });

  testWidgets('shows module content and auth context when authenticated', (
    tester,
  ) async {
    await tester.pumpWidget(
      _wrap(
        override: authBootstrapProbeProvider.overrideWith(
          (ref) async => const BootstrapProbeResult.authenticated(
            userEmail: 'user@example.com',
            tenantSlug: 'tenant-a',
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();

    expect(find.text('module-content'), findsOneWidget);
    expect(find.text('me: user@example.com'), findsOneWidget);
    expect(find.textContaining('tenant: tenant-a'), findsOneWidget);
  });

  testWidgets('shows error fallback with retry button', (tester) async {
    await tester.pumpWidget(
      _wrap(
        override: authBootstrapProbeProvider.overrideWith((ref) async {
          throw StateError('boom');
        }),
      ),
    );
    await tester.pumpAndSettle();

    expect(find.text('Failed to initialize host context.'), findsOneWidget);
    expect(find.text('Retry bootstrap'), findsOneWidget);
  });
}
