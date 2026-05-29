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

abstract interface class ModulesRepository {
  Future<List<ModuleSummary>> listModules();
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
}
