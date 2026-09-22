import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'module_summary.dart';
import 'modules_repository.dart';

final modulesRepositoryProvider = Provider<ModulesRepository>((ref) {
  throw UnimplementedError(
    'Host app must override modulesRepositoryProvider with a shared GraphQL-backed repository.',
  );
});

final modulesControllerProvider = FutureProvider<List<ModuleSummary>>((ref) {
  return ref.watch(modulesRepositoryProvider).listModules();
});
