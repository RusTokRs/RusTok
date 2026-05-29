import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../module_summary.dart';
import '../modules_controller.dart';

typedef ModuleOpenCallback = void Function(
  BuildContext context,
  ModuleSummary module,
);

class ModulesMobileScreen extends ConsumerWidget {
  const ModulesMobileScreen({
    super.key,
    this.header,
    this.onOpenModule,
    this.resolveModulePath,
  });

  final Widget? header;
  final ModuleOpenCallback? onOpenModule;
  final String? Function(ModuleSummary module)? resolveModulePath;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final modules = ref.watch(modulesControllerProvider);
    return modules.when(
      data: (items) => _ModulesList(
        modules: items,
        header: header,
        onOpenModule: onOpenModule,
        resolveModulePath: resolveModulePath,
      ),
      loading: () => const Center(child: CircularProgressIndicator()),
      error: (error, _) => _ModulesErrorView(
        error: error,
        onRetry: () => ref.invalidate(modulesControllerProvider),
      ),
    );
  }
}

class _ModulesList extends StatelessWidget {
  const _ModulesList({
    required this.modules,
    required this.header,
    required this.onOpenModule,
    required this.resolveModulePath,
  });

  final List<ModuleSummary> modules;
  final Widget? header;
  final ModuleOpenCallback? onOpenModule;
  final String? Function(ModuleSummary module)? resolveModulePath;

  @override
  Widget build(BuildContext context) {
    if (modules.isEmpty) {
      return ListView(
        children: [
          if (header != null) header!,
          const _EmptyModulesView(),
        ],
      );
    }

    final children = <Widget>[
      if (header != null) header!,
      const ListTile(
        title: Text('Modules pilot'),
        subtitle: Text(
          'GraphQL-backed module registry flow mounted through the host shell.',
        ),
      ),
      for (final module in modules)
        Builder(
          builder: (context) {
            final path = resolveModulePath?.call(module);
            return Card(
              margin: const EdgeInsets.symmetric(horizontal: 12, vertical: 6),
              child: ListTile(
                title: Text(module.name.isEmpty ? module.slug : module.name),
                subtitle: Text(_buildSubtitle(module, path)),
                isThreeLine: true,
                trailing: _StatusChip(enabled: module.enabled),
                onTap: path == null || onOpenModule == null
                    ? null
                    : () => onOpenModule!(context, module),
              ),
            );
          },
        ),
    ];

    return ListView(children: children);
  }

  String _buildSubtitle(ModuleSummary module, String? path) {
    final parts = <String>[
      if (module.description.isNotEmpty) module.description,
      'kind: ${module.kind.isEmpty ? 'unknown' : module.kind}',
      'version: ${module.version.isEmpty ? 'unknown' : module.version}',
      if (path != null) 'mobile route: $path' else 'mobile route: not mounted',
    ];
    return parts.join('\n');
  }
}

class _StatusChip extends StatelessWidget {
  const _StatusChip({required this.enabled});

  final bool enabled;

  @override
  Widget build(BuildContext context) {
    return Chip(
      label: Text(enabled ? 'Enabled' : 'Disabled'),
      avatar: Icon(
        enabled ? Icons.check_circle : Icons.pause_circle,
        size: 18,
      ),
    );
  }
}

class _EmptyModulesView extends StatelessWidget {
  const _EmptyModulesView();

  @override
  Widget build(BuildContext context) {
    return const Padding(
      padding: EdgeInsets.all(24),
      child: Center(
        child: Text('No modules returned by the registry query.'),
      ),
    );
  }
}

class _ModulesErrorView extends StatelessWidget {
  const _ModulesErrorView({required this.error, required this.onRetry});

  final Object error;
  final VoidCallback onRetry;

  @override
  Widget build(BuildContext context) {
    return Center(
      child: Padding(
        padding: const EdgeInsets.all(24),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            const Icon(Icons.error_outline, size: 40),
            const SizedBox(height: 12),
            const Text('Failed to load module registry.'),
            const SizedBox(height: 8),
            Text(
              '$error',
              textAlign: TextAlign.center,
            ),
            const SizedBox(height: 12),
            FilledButton(onPressed: onRetry, child: const Text('Retry')),
          ],
        ),
      ),
    );
  }
}
