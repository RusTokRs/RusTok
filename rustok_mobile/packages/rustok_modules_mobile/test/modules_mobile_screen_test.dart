import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:graphql/client.dart';
import 'package:rustok_modules_mobile/rustok_modules_mobile.dart';

void main() {
  recoveryScreenTests();
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
              canManageModules: true,
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

  testWidgets('runs mutation-backed toggle action and refreshes registry', (
    tester,
  ) async {
    final repository = _MutableModulesRepository();

    await tester.pumpWidget(
      ProviderScope(
        overrides: [
          modulesRepositoryProvider.overrideWithValue(repository),
        ],
        child: const MaterialApp(
          home: Scaffold(
            body: ModulesMobileScreen(canManageModules: true),
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();

    expect(find.text('Enabled'), findsOneWidget);

    await tester.tap(find.text('Disable'));
    await tester.pumpAndSettle();

    expect(repository.toggleRequests, const [false]);
    expect(find.text('Disabled'), findsOneWidget);
    expect(find.text('Enable'), findsOneWidget);
  });

  testWidgets('shows recovery feedback for retryable post-hook failures', (
    tester,
  ) async {
    final repository = _PostHookFailureRepository();

    await tester.pumpWidget(
      ProviderScope(
        overrides: [
          modulesRepositoryProvider.overrideWithValue(repository),
        ],
        child: const MaterialApp(
          home: Scaffold(
            body: ModulesMobileScreen(canManageModules: true),
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();

    await tester.tap(find.text('Disable'));
    await tester.pumpAndSettle();

    expect(repository.recoveryQueries, const ['blog']);
    expect(find.text('Recovery available'), findsOneWidget);
    expect(find.textContaining('post hook timed out'), findsOneWidget);
    expect(find.text('Recommended action: retry_post_hook'), findsOneWidget);
  });

  testWidgets('runs retry post-hook recovery action', (tester) async {
    final repository = _PostHookFailureRepository();

    await tester.pumpWidget(
      ProviderScope(
        overrides: [
          modulesRepositoryProvider.overrideWithValue(repository),
        ],
        child: const MaterialApp(
          home: Scaffold(
            body: ModulesMobileScreen(canManageModules: true),
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();

    await tester.tap(find.text('Disable'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Retry post-hook'));
    await tester.pumpAndSettle();

    expect(repository.retryRequests, const ['op-1']);
    expect(find.text('Recovery available'), findsNothing);
  });

  testWidgets('runs compensation recovery action', (tester) async {
    final repository = _PostHookFailureRepository();

    await tester.pumpWidget(
      ProviderScope(
        overrides: [
          modulesRepositoryProvider.overrideWithValue(repository),
        ],
        child: const MaterialApp(
          home: Scaffold(
            body: ModulesMobileScreen(canManageModules: true),
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();

    await tester.tap(find.text('Disable'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Compensate'));
    await tester.pumpAndSettle();

    expect(repository.compensateRequests, const ['op-1']);
    expect(find.text('Recovery available'), findsNothing);
  });

  testWidgets('gates toggle action when modules manage permission is missing', (
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
        child: const MaterialApp(
          home: Scaffold(body: ModulesMobileScreen()),
        ),
      ),
    );
    await tester.pumpAndSettle();

    expect(find.text('Requires modules:manage'), findsOneWidget);
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

  @override
  Future<ModuleToggleResult> toggleModule({
    required String moduleSlug,
    required bool enabled,
  }) async {
    return ModuleToggleResult(
      moduleSlug: moduleSlug,
      enabled: enabled,
      settings: '{}',
    );
  }

  @override
  Future<List<ModuleOperationRecoveryPlan>> failedRecoveryPlans({
    required String moduleSlug,
    int limit = 1,
  }) async {
    return const <ModuleOperationRecoveryPlan>[];
  }

  @override
  Future<ModuleOperationRecoveryPlan> retryFailedPostHook({
    required String operationId,
  }) async {
    throw UnimplementedError('retry is not used by this test repository');
  }

  @override
  Future<ModuleToggleResult> compensateFailedOperation({
    required String operationId,
  }) async {
    throw UnimplementedError('compensation is not used by this test repository');
  }
}

class _MutableModulesRepository implements ModulesRepository {
  final List<bool> toggleRequests = <bool>[];
  bool _enabled = true;

  @override
  Future<List<ModuleSummary>> listModules() async => [
    ModuleSummary(
      slug: 'blog',
      name: 'Blog Module',
      description: 'Editorial content',
      version: '1.2.3',
      kind: 'optional',
      enabled: _enabled,
      ownership: 'platform',
      trustLevel: 'trusted',
      recommendedAdminSurfaces: const ['posts'],
      showcaseAdminSurfaces: const [],
    ),
  ];

  @override
  Future<ModuleToggleResult> toggleModule({
    required String moduleSlug,
    required bool enabled,
  }) async {
    toggleRequests.add(enabled);
    _enabled = enabled;
    return ModuleToggleResult(
      moduleSlug: moduleSlug,
      enabled: enabled,
      settings: '{}',
    );
  }

  @override
  Future<List<ModuleOperationRecoveryPlan>> failedRecoveryPlans({
    required String moduleSlug,
    int limit = 1,
  }) async {
    return const <ModuleOperationRecoveryPlan>[];
  }

  @override
  Future<ModuleOperationRecoveryPlan> retryFailedPostHook({
    required String operationId,
  }) async {
    throw UnimplementedError('retry is not used by this test repository');
  }

  @override
  Future<ModuleToggleResult> compensateFailedOperation({
    required String operationId,
  }) async {
    throw UnimplementedError('compensation is not used by this test repository');
  }
}

class _PostHookFailureRepository implements ModulesRepository {
  final List<String> recoveryQueries = <String>[];
  final List<String> retryRequests = <String>[];
  final List<String> compensateRequests = <String>[];

  @override
  Future<List<ModuleSummary>> listModules() async => const [
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
  ];

  @override
  Future<ModuleToggleResult> toggleModule({
    required String moduleSlug,
    required bool enabled,
  }) async {
    throw OperationException(
      graphqlErrors: [
        GraphQLError(
          message: 'Module post-hook failed',
          extensions: {
            'retryable_issue': true,
            'operation_issue': 'post_hook_failed',
          },
        ),
      ],
    );
  }

  @override
  Future<List<ModuleOperationRecoveryPlan>> failedRecoveryPlans({
    required String moduleSlug,
    int limit = 1,
  }) async {
    recoveryQueries.add(moduleSlug);
    return const [
      ModuleOperationRecoveryPlan(
        operationId: 'op-1',
        moduleSlug: 'blog',
        requestedEnabled: false,
        previousEffectiveEnabled: true,
        status: 'failed',
        issue: 'post_hook_failed',
        retryable: true,
        recommendedAction: 'retry_post_hook',
        errorMessage: 'post hook timed out',
      ),
    ];
  }

  @override
  Future<ModuleOperationRecoveryPlan> retryFailedPostHook({
    required String operationId,
  }) async {
    retryRequests.add(operationId);
    return const ModuleOperationRecoveryPlan(
      operationId: 'op-2',
      moduleSlug: 'blog',
      requestedEnabled: false,
      previousEffectiveEnabled: true,
      status: 'committed',
      issue: 'none',
      retryable: false,
      recommendedAction: 'none',
    );
  }

  @override
  Future<ModuleToggleResult> compensateFailedOperation({
    required String operationId,
  }) async {
    compensateRequests.add(operationId);
    return const ModuleToggleResult(
      moduleSlug: 'blog',
      enabled: true,
      settings: '{}',
    );
  }
}

class _FailingRepository implements ModulesRepository {
  const _FailingRepository();

  @override
  Future<List<ModuleSummary>> listModules() async {
    throw StateError('registry unavailable');
  }

  @override
  Future<ModuleToggleResult> toggleModule({
    required String moduleSlug,
    required bool enabled,
  }) async {
    throw StateError('registry unavailable');
  }

  @override
  Future<List<ModuleOperationRecoveryPlan>> failedRecoveryPlans({
    required String moduleSlug,
    int limit = 1,
  }) async {
    throw StateError('registry unavailable');
  }

  @override
  Future<ModuleOperationRecoveryPlan> retryFailedPostHook({
    required String operationId,
  }) async {
    throw StateError('registry unavailable');
  }

  @override
  Future<ModuleToggleResult> compensateFailedOperation({
    required String operationId,
  }) async {
    throw StateError('registry unavailable');
  }
}

void recoveryScreenTests() {
  testWidgets('renders operation history with recovery metadata', (tester) async {
    final repository = _RecoveryHistoryRepository();

    await tester.pumpWidget(
      ProviderScope(
        overrides: [
          modulesRepositoryProvider.overrideWithValue(repository),
        ],
        child: const MaterialApp(
          home: Scaffold(body: ModulesRecoveryScreen(moduleSlug: 'blog')),
        ),
      ),
    );
    await tester.pumpAndSettle();

    expect(repository.historyQueries, const ['blog:20']);
    expect(find.text('blog recovery'), findsOneWidget);
    expect(find.text('Operation op-1'), findsOneWidget);
    expect(find.text('Requested: disable module blog'), findsOneWidget);
    expect(find.text('Correlation ID: corr-1'), findsOneWidget);
    expect(find.text('Requested by: operator@example.com'), findsOneWidget);
  });

  testWidgets('runs recovery actions from operation history screen', (
    tester,
  ) async {
    final repository = _RecoveryHistoryRepository();

    await tester.pumpWidget(
      ProviderScope(
        overrides: [
          modulesRepositoryProvider.overrideWithValue(repository),
        ],
        child: const MaterialApp(
          home: Scaffold(body: ModulesRecoveryScreen(moduleSlug: 'blog')),
        ),
      ),
    );
    await tester.pumpAndSettle();

    await tester.tap(find.text('Retry post-hook'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Compensate'));
    await tester.pumpAndSettle();

    expect(repository.retryRequests, const ['op-1']);
    expect(repository.compensateRequests, const ['op-1']);
    expect(repository.historyQueries.length, 3);
  });
}

class _RecoveryHistoryRepository implements ModulesRepository {
  final List<String> historyQueries = <String>[];
  final List<String> retryRequests = <String>[];
  final List<String> compensateRequests = <String>[];

  @override
  Future<List<ModuleSummary>> listModules() async => const <ModuleSummary>[];

  @override
  Future<ModuleToggleResult> toggleModule({
    required String moduleSlug,
    required bool enabled,
  }) async {
    throw UnimplementedError('toggle is not used by this test repository');
  }

  @override
  Future<List<ModuleOperationRecoveryPlan>> failedRecoveryPlans({
    required String moduleSlug,
    int limit = 1,
  }) async {
    historyQueries.add('$moduleSlug:$limit');
    return const [
      ModuleOperationRecoveryPlan(
        operationId: 'op-1',
        moduleSlug: 'blog',
        requestedEnabled: false,
        previousEffectiveEnabled: true,
        status: 'failed',
        issue: 'post_hook_failed',
        retryable: true,
        recommendedAction: 'retry_post_hook',
        correlationId: 'corr-1',
        requestedBy: 'operator@example.com',
        errorMessage: 'post hook timed out',
      ),
    ];
  }

  @override
  Future<ModuleOperationRecoveryPlan> retryFailedPostHook({
    required String operationId,
  }) async {
    retryRequests.add(operationId);
    return const ModuleOperationRecoveryPlan(
      operationId: 'op-1',
      moduleSlug: 'blog',
      requestedEnabled: false,
      previousEffectiveEnabled: true,
      status: 'committed',
      issue: 'none',
      retryable: false,
      recommendedAction: 'none',
    );
  }

  @override
  Future<ModuleToggleResult> compensateFailedOperation({
    required String operationId,
  }) async {
    compensateRequests.add(operationId);
    return const ModuleToggleResult(
      moduleSlug: 'blog',
      enabled: true,
      settings: '{}',
    );
  }
}
