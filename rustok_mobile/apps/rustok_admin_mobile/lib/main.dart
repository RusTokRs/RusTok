import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'app_shell/auth_bootstrap.dart';
import 'registry/mobile_module_registry.dart';
import 'routes/app_router.dart';

final mobileRegistryProvider =
    Provider((ref) => buildAdaptedMobileModuleRegistry());

void main() {
  runApp(const ProviderScope(child: RusTokAdminMobileApp()));
}

class RusTokAdminMobileApp extends ConsumerWidget {
  const RusTokAdminMobileApp({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    ref.watch(graphQlClientProvider);
    final router = buildRouter(ref.watch(mobileRegistryProvider));
    return MaterialApp.router(
      title: 'RusTok Admin Mobile',
      theme: ThemeData(useMaterial3: true, colorSchemeSeed: Colors.deepPurple),
      routerConfig: router,
    );
  }
}
