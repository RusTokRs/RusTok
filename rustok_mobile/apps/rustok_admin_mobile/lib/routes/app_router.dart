import 'package:app_module_contracts/app_module_contracts.dart';
import 'package:app_route_contracts/app_route_contracts.dart';
import 'package:flutter/material.dart';
import 'package:go_router/go_router.dart';

import '../app_shell/app_shell_page.dart';

GoRouter buildRouter(List<MobileModuleEntry> entries) {
  return GoRouter(
    initialLocation: '/modules',
    routes: [
      ShellRoute(
        builder: (context, state, child) => AppShellPage(child: child),
        routes: [
          GoRoute(
            path: '/modules',
            builder: (context, state) => ModulesHomePage(entries: entries),
            routes: [
              for (final entry in entries)
                GoRoute(
                  path: entry.routeSegment,
                  name: entry.moduleKey,
                  builder: (context, state) {
                    const codec = RouteCodec(
                      RouteSanitizer({
                        QueryKeys.tab,
                        QueryKeys.productId,
                        QueryKeys.orderId,
                        QueryKeys.mediaId,
                      }),
                    );
                    final selection = codec.decode(
                      state.uri.path,
                      state.uri.queryParameters,
                    );
                    return ModulePlaceholderPage(
                      entry: entry,
                      selection: selection,
                    );
                  },
                ),
            ],
          ),
        ],
      ),
    ],
  );
}

class ModulesHomePage extends StatelessWidget {
  const ModulesHomePage({super.key, required this.entries});

  final List<MobileModuleEntry> entries;

  @override
  Widget build(BuildContext context) {
    return ListView(
      children: [
        const ListTile(title: Text('RusTok Modules')),
        for (final entry in entries)
          ListTile(
            title: Text(entry.nav.title),
            subtitle: Text('/modules/${entry.routeSegment}'),
            onTap: () => context.go('/modules/${entry.routeSegment}'),
          ),
      ],
    );
  }
}

class ModulePlaceholderPage extends StatelessWidget {
  const ModulePlaceholderPage({
    super.key,
    required this.entry,
    required this.selection,
  });

  final MobileModuleEntry entry;
  final RouteSelection selection;

  @override
  Widget build(BuildContext context) {
    return Center(
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          Text('Module: ${entry.moduleKey}'),
          Text('Location: ${selection.toLocation()}'),
        ],
      ),
    );
  }
}
