import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:rustok_modules_mobile/rustok_modules_mobile.dart';

void main() {
  testWidgets('renders GraphQL-backed module summaries and route hints', (
    tester,
  ) async {
    await tester.pumpWidget(
      ProviderScope(
        overrides: [
          modulesRepositoryProvider.overrideWithValue(
            const _FakeModulesRepository([
              ModuleSummary(
                slug: 'blog',
                name: 'Blog Module',
                description: 'Editorial content',
                version: '1.2.3',
                kind: 'optional',
                enabled: true,
                ownership: 'platform',
                trustLevel: 'trusted',
                recommendedAdminSurfaces: ['posts'],
                showcaseAdminSurfaces: [],
              ),
            ]),
          ),
        ],
        child: MaterialApp(
          home: Scaffold(
            body: ModulesMobileScreen(
              resolveModulePath: (module) => '/modules/${module.slug}',
            ),
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();

    expect(find.text('Modules pilot'), findsOneWidget);
    expect(find.text('Blog Module'), findsOneWidget);
    expect(find.textContaining('mobile route: /modules/blog'), findsOneWidget);
    expect(find.text('Enabled'), findsOneWidget);
  });

  testWidgets('shows retryable error state', (tester) async {
    await tester.pumpWidget(
      ProviderScope(
        overrides: [
          modulesRepositoryProvider.overrideWithValue(const _FailingRepository()),
        ],
        child: const MaterialApp(
          home: Scaffold(body: ModulesMobileScreen()),
        ),
      ),
    );
    await tester.pumpAndSettle();

    expect(find.text('Failed to load module registry.'), findsOneWidget);
    expect(find.text('Retry'), findsOneWidget);
  });
}

class _FakeModulesRepository implements ModulesRepository {
  const _FakeModulesRepository(this.modules);

  final List<ModuleSummary> modules;

  @override
  Future<List<ModuleSummary>> listModules() async => modules;
}

class _FailingRepository implements ModulesRepository {
  const _FailingRepository();

  @override
  Future<List<ModuleSummary>> listModules() async {
    throw StateError('registry unavailable');
  }
}
