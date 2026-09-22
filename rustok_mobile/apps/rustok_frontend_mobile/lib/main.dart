import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'routes/storefront_router.dart';

void main() {
  runApp(const ProviderScope(child: RusTokFrontendMobileApp()));
}

class RusTokFrontendMobileApp extends ConsumerWidget {
  const RusTokFrontendMobileApp({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final router = ref.watch(storefrontRouterProvider);
    return MaterialApp.router(
      title: 'RusTok Frontend Mobile',
      theme: ThemeData(useMaterial3: true, colorSchemeSeed: Colors.teal),
      routerConfig: router,
    );
  }
}
