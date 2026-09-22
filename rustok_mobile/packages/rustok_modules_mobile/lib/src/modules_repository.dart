import 'package:graphql/client.dart';

import 'module_summary.dart';

const moduleRegistryQuery = r'''
  query ModuleRegistry {
    moduleRegistry {
      moduleSlug
      name
      description
      version
      kind
      dependencies
      enabled
      ownership
      trustLevel
      recommendedAdminSurfaces
      showcaseAdminSurfaces
    }
  }
''';

const toggleModuleMutation = r'''
  mutation ToggleModule($moduleSlug: String!, $enabled: Boolean!) {
    toggleModule(moduleSlug: $moduleSlug, enabled: $enabled) {
      moduleSlug
      enabled
      settings
    }
  }
''';

const failedModuleOperationRecoveryPlansQuery = r'''
  query FailedModuleOperationRecoveryPlans($moduleSlug: String!, $limit: Int!) {
    failedModuleOperationRecoveryPlans(moduleSlug: $moduleSlug, limit: $limit) {
      operationId
      moduleSlug
      requestedEnabled
      previousEffectiveEnabled
      status
      issue
      retryable
      recommendedAction
      correlationId
      requestedBy
      errorMessage
    }
  }
''';

const retryFailedModuleOperationPostHookMutation = r'''
  mutation RetryFailedModuleOperationPostHook($operationId: UUID!) {
    retryFailedModuleOperationPostHook(operationId: $operationId) {
      operationId
      moduleSlug
      requestedEnabled
      previousEffectiveEnabled
      status
      issue
      retryable
      recommendedAction
      correlationId
      requestedBy
      errorMessage
    }
  }
''';

const compensateFailedModuleOperationMutation = r'''
  mutation CompensateFailedModuleOperation($operationId: UUID!) {
    compensateFailedModuleOperation(operationId: $operationId) {
      moduleSlug
      enabled
      settings
    }
  }
''';

abstract interface class ModulesRepository {
  Future<List<ModuleSummary>> listModules();

  Future<ModuleToggleResult> toggleModule({
    required String moduleSlug,
    required bool enabled,
  });

  Future<List<ModuleOperationRecoveryPlan>> failedRecoveryPlans({
    required String moduleSlug,
    int limit = 1,
  });

  Future<ModuleOperationRecoveryPlan> retryFailedPostHook({
    required String operationId,
  });

  Future<ModuleToggleResult> compensateFailedOperation({
    required String operationId,
  });
}

class GraphQlModulesRepository implements ModulesRepository {
  const GraphQlModulesRepository(this._client);

  final GraphQLClient _client;

  @override
  Future<List<ModuleSummary>> listModules() async {
    final result = await _client.query(
      QueryOptions(
        document: gql(moduleRegistryQuery),
        fetchPolicy: FetchPolicy.cacheAndNetwork,
      ),
    );

    if (result.hasException) {
      throw result.exception!;
    }

    final payload = result.data?['moduleRegistry'];
    if (payload is! List) {
      return const <ModuleSummary>[];
    }

    return List.unmodifiable(
      payload.whereType<Map<String, dynamic>>().map(ModuleSummary.fromJson),
    );
  }

  @override
  Future<ModuleToggleResult> toggleModule({
    required String moduleSlug,
    required bool enabled,
  }) async {
    final result = await _client.mutate(
      MutationOptions(
        document: gql(toggleModuleMutation),
        variables: <String, dynamic>{
          'moduleSlug': moduleSlug,
          'enabled': enabled,
        },
      ),
    );

    if (result.hasException) {
      throw result.exception!;
    }

    final payload = result.data?['toggleModule'];
    if (payload is! Map<String, dynamic>) {
      throw const FormatException('toggleModule response payload is missing.');
    }

    return ModuleToggleResult.fromJson(payload);
  }

  @override
  Future<List<ModuleOperationRecoveryPlan>> failedRecoveryPlans({
    required String moduleSlug,
    int limit = 1,
  }) async {
    final result = await _client.query(
      QueryOptions(
        document: gql(failedModuleOperationRecoveryPlansQuery),
        fetchPolicy: FetchPolicy.networkOnly,
        variables: <String, dynamic>{
          'moduleSlug': moduleSlug,
          'limit': limit,
        },
      ),
    );

    if (result.hasException) {
      throw result.exception!;
    }

    final payload = result.data?['failedModuleOperationRecoveryPlans'];
    if (payload is! List) {
      return const <ModuleOperationRecoveryPlan>[];
    }

    return List.unmodifiable(
      payload
          .whereType<Map<String, dynamic>>()
          .map(ModuleOperationRecoveryPlan.fromJson),
    );
  }

  @override
  Future<ModuleOperationRecoveryPlan> retryFailedPostHook({
    required String operationId,
  }) async {
    final result = await _client.mutate(
      MutationOptions(
        document: gql(retryFailedModuleOperationPostHookMutation),
        variables: <String, dynamic>{'operationId': operationId},
      ),
    );

    if (result.hasException) {
      throw result.exception!;
    }

    final payload = result.data?['retryFailedModuleOperationPostHook'];
    if (payload is! Map<String, dynamic>) {
      throw const FormatException(
        'retryFailedModuleOperationPostHook response payload is missing.',
      );
    }

    return ModuleOperationRecoveryPlan.fromJson(payload);
  }

  @override
  Future<ModuleToggleResult> compensateFailedOperation({
    required String operationId,
  }) async {
    final result = await _client.mutate(
      MutationOptions(
        document: gql(compensateFailedModuleOperationMutation),
        variables: <String, dynamic>{'operationId': operationId},
      ),
    );

    if (result.hasException) {
      throw result.exception!;
    }

    final payload = result.data?['compensateFailedModuleOperation'];
    if (payload is! Map<String, dynamic>) {
      throw const FormatException(
        'compensateFailedModuleOperation response payload is missing.',
      );
    }

    return ModuleToggleResult.fromJson(payload);
  }
}

class ModuleToggleResult {
  const ModuleToggleResult({
    required this.moduleSlug,
    required this.enabled,
    required this.settings,
  });

  final String moduleSlug;
  final bool enabled;
  final String settings;

  factory ModuleToggleResult.fromJson(Map<String, dynamic> json) {
    return ModuleToggleResult(
      moduleSlug: _readToggleString(json, 'moduleSlug'),
      enabled: json['enabled'] == true,
      settings: _readToggleString(json, 'settings'),
    );
  }
}

class ModuleOperationRecoveryPlan {
  const ModuleOperationRecoveryPlan({
    required this.operationId,
    required this.moduleSlug,
    required this.requestedEnabled,
    required this.previousEffectiveEnabled,
    required this.status,
    required this.issue,
    required this.retryable,
    required this.recommendedAction,
    this.correlationId,
    this.requestedBy,
    this.errorMessage,
  });

  final String operationId;
  final String moduleSlug;
  final bool requestedEnabled;
  final bool previousEffectiveEnabled;
  final String status;
  final String issue;
  final bool retryable;
  final String recommendedAction;
  final String? correlationId;
  final String? requestedBy;
  final String? errorMessage;

  factory ModuleOperationRecoveryPlan.fromJson(Map<String, dynamic> json) {
    return ModuleOperationRecoveryPlan(
      operationId: _readToggleString(json, 'operationId'),
      moduleSlug: _readToggleString(json, 'moduleSlug'),
      requestedEnabled: json['requestedEnabled'] == true,
      previousEffectiveEnabled: json['previousEffectiveEnabled'] == true,
      status: _readToggleString(json, 'status'),
      issue: _readToggleString(json, 'issue'),
      retryable: json['retryable'] == true,
      recommendedAction: _readToggleString(json, 'recommendedAction'),
      correlationId: _readOptionalToggleString(json, 'correlationId'),
      requestedBy: _readOptionalToggleString(json, 'requestedBy'),
      errorMessage: _readOptionalToggleString(json, 'errorMessage'),
    );
  }
}

String _readToggleString(Map<String, dynamic> json, String key) {
  final value = json[key];
  return value is String ? value : '';
}

String? _readOptionalToggleString(Map<String, dynamic> json, String key) {
  final value = json[key];
  return value is String && value.isNotEmpty ? value : null;
}
