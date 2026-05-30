import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:graphql/client.dart';

import '../module_summary.dart';
import '../modules_controller.dart';
import '../modules_repository.dart';

const _modulesManagePermission = 'modules:manage';

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
    this.canManageModules = false,
  });

  final Widget? header;
  final ModuleOpenCallback? onOpenModule;
  final String? Function(ModuleSummary module)? resolveModulePath;
  final bool canManageModules;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final modules = ref.watch(modulesControllerProvider);
    return modules.when(
      data: (items) => _ModulesList(
        modules: items,
        header: header,
        onOpenModule: onOpenModule,
        resolveModulePath: resolveModulePath,
        canManageModules: canManageModules,
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
    required this.canManageModules,
  });

  final List<ModuleSummary> modules;
  final Widget? header;
  final ModuleOpenCallback? onOpenModule;
  final String? Function(ModuleSummary module)? resolveModulePath;
  final bool canManageModules;

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
        _ModuleCard(
          module: module,
          path: resolveModulePath?.call(module),
          onOpenModule: onOpenModule,
          canManageModules: canManageModules,
        ),
    ];

    return ListView(children: children);
  }
}

class _ModuleCard extends ConsumerStatefulWidget {
  const _ModuleCard({
    required this.module,
    required this.path,
    required this.onOpenModule,
    required this.canManageModules,
  });

  final ModuleSummary module;
  final String? path;
  final ModuleOpenCallback? onOpenModule;
  final bool canManageModules;

  @override
  ConsumerState<_ModuleCard> createState() => _ModuleCardState();
}

class _ModuleCardState extends ConsumerState<_ModuleCard> {
  Object? _toggleError;
  Object? _recoveryActionError;
  ModuleOperationRecoveryPlan? _recoveryPlan;
  bool _isToggling = false;
  bool _isRecovering = false;

  @override
  Widget build(BuildContext context) {
    final module = widget.module;
    final path = widget.path;
    final toggleLabel = module.enabled ? 'Disable' : 'Enable';
    final disabledReason = _toggleDisabledReason(module);
    final canToggle = disabledReason == null;

    return Card(
      margin: const EdgeInsets.symmetric(horizontal: 12, vertical: 6),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          ListTile(
            title: Text(module.name.isEmpty ? module.slug : module.name),
            subtitle: Text(_buildSubtitle(module, path)),
            isThreeLine: true,
            trailing: _StatusChip(enabled: module.enabled),
            onTap: path == null || widget.onOpenModule == null
                ? null
                : () => widget.onOpenModule!(context, module),
          ),
          Padding(
            padding: const EdgeInsets.fromLTRB(16, 0, 16, 12),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                if (_toggleError != null) ...[
                  Text(
                    'Toggle failed: $_toggleError',
                    style: TextStyle(
                      color: Theme.of(context).colorScheme.error,
                    ),
                  ),
                  const SizedBox(height: 8),
                ],
                if (_recoveryPlan != null) ...[
                  _RecoveryPlanNotice(
                    plan: _recoveryPlan!,
                    isBusy: _isRecovering,
                    actionError: _recoveryActionError,
                    onRetry: _retryPostHook,
                    onCompensate: _compensateOperation,
                  ),
                  const SizedBox(height: 8),
                ],
                Align(
                  alignment: Alignment.centerRight,
                  child: FilledButton.icon(
                    onPressed: canToggle && !_isToggling
                        ? () => _toggleModule(!module.enabled)
                        : null,
                    icon: _isToggling
                        ? const SizedBox.square(
                            dimension: 16,
                            child: CircularProgressIndicator(strokeWidth: 2),
                          )
                        : Icon(
                            module.enabled
                                ? Icons.pause_circle_outline
                                : Icons.play_circle_outline,
                          ),
                    label: Text(canToggle ? toggleLabel : disabledReason!),
                  ),
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }

  String? _toggleDisabledReason(ModuleSummary module) {
    if (!module.isOptional) {
      return 'Core module';
    }
    if (!widget.canManageModules) {
      return 'Requires $_modulesManagePermission';
    }
    return null;
  }

  Future<void> _toggleModule(bool enabled) async {
    setState(() {
      _isToggling = true;
      _toggleError = null;
      _recoveryActionError = null;
      _recoveryPlan = null;
    });

    try {
      final repository = ref.read(modulesRepositoryProvider);
      await repository.toggleModule(
        moduleSlug: widget.module.slug,
        enabled: enabled,
      );
      ref.invalidate(modulesControllerProvider);
    } catch (error) {
      final recoveryPlan = await _loadRecoveryPlan(error);
      if (mounted) {
        setState(() {
          if (recoveryPlan == null) {
            _toggleError = error;
          } else {
            _recoveryPlan = recoveryPlan;
          }
        });
      }
      if (recoveryPlan != null) {
        ref.invalidate(modulesControllerProvider);
      }
    } finally {
      if (mounted) {
        setState(() => _isToggling = false);
      }
    }
  }

  Future<ModuleOperationRecoveryPlan?> _loadRecoveryPlan(Object error) async {
    if (!_isRetryablePostHookFailure(error)) {
      return null;
    }

    try {
      final repository = ref.read(modulesRepositoryProvider);
      final plans = await repository.failedRecoveryPlans(
        moduleSlug: widget.module.slug,
      );
      return plans.isEmpty ? null : plans.first;
    } catch (_) {
      return null;
    }
  }

  bool _isRetryablePostHookFailure(Object error) {
    if (error is! OperationException) {
      return false;
    }
    return error.graphqlErrors.any((graphqlError) {
      final extensions = graphqlError.extensions;
      return extensions?['retryable_issue'] == true &&
          extensions?['operation_issue'] == 'post_hook_failed';
    });
  }

  Future<void> _retryPostHook() async {
    final plan = _recoveryPlan;
    if (plan == null || _isRecovering) {
      return;
    }

    setState(() {
      _isRecovering = true;
      _recoveryActionError = null;
    });

    try {
      final repository = ref.read(modulesRepositoryProvider);
      final nextPlan = await repository.retryFailedPostHook(
        operationId: plan.operationId,
      );
      if (mounted) {
        setState(() {
          _recoveryPlan = nextPlan.retryable ? nextPlan : null;
        });
      }
      ref.invalidate(modulesControllerProvider);
    } catch (error) {
      if (mounted) {
        setState(() => _recoveryActionError = error);
      }
    } finally {
      if (mounted) {
        setState(() => _isRecovering = false);
      }
    }
  }

  Future<void> _compensateOperation() async {
    final plan = _recoveryPlan;
    if (plan == null || _isRecovering) {
      return;
    }

    setState(() {
      _isRecovering = true;
      _recoveryActionError = null;
    });

    try {
      final repository = ref.read(modulesRepositoryProvider);
      await repository.compensateFailedOperation(
        operationId: plan.operationId,
      );
      if (mounted) {
        setState(() => _recoveryPlan = null);
      }
      ref.invalidate(modulesControllerProvider);
    } catch (error) {
      if (mounted) {
        setState(() => _recoveryActionError = error);
      }
    } finally {
      if (mounted) {
        setState(() => _isRecovering = false);
      }
    }
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

class _RecoveryPlanNotice extends StatelessWidget {
  const _RecoveryPlanNotice({
    required this.plan,
    required this.isBusy,
    required this.onRetry,
    required this.onCompensate,
    this.actionError,
  });

  final ModuleOperationRecoveryPlan plan;
  final bool isBusy;
  final VoidCallback onRetry;
  final VoidCallback onCompensate;
  final Object? actionError;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final action = plan.recommendedAction.isEmpty
        ? 'review operation recovery plan'
        : plan.recommendedAction;
    final message = plan.errorMessage ??
        'Module state was committed, but a post-hook failed.';

    return Material(
      color: theme.colorScheme.errorContainer,
      borderRadius: BorderRadius.circular(12),
      child: Padding(
        padding: const EdgeInsets.all(12),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(
              'Recovery available',
              style: theme.textTheme.labelLarge?.copyWith(
                color: theme.colorScheme.onErrorContainer,
              ),
            ),
            const SizedBox(height: 4),
            Text(
              message,
              style: TextStyle(color: theme.colorScheme.onErrorContainer),
            ),
            const SizedBox(height: 4),
            Text(
              'Recommended action: $action',
              style: TextStyle(color: theme.colorScheme.onErrorContainer),
            ),
            if (actionError != null) ...[
              const SizedBox(height: 8),
              Text(
                'Recovery action failed: $actionError',
                style: TextStyle(color: theme.colorScheme.onErrorContainer),
              ),
            ],
            const SizedBox(height: 12),
            Wrap(
              spacing: 8,
              runSpacing: 8,
              children: [
                FilledButton.tonalIcon(
                  onPressed: isBusy || !plan.retryable ? null : onRetry,
                  icon: isBusy
                      ? const SizedBox.square(
                          dimension: 16,
                          child: CircularProgressIndicator(strokeWidth: 2),
                        )
                      : const Icon(Icons.refresh),
                  label: const Text('Retry post-hook'),
                ),
                OutlinedButton.icon(
                  onPressed: isBusy ? null : onCompensate,
                  icon: const Icon(Icons.undo),
                  label: const Text('Compensate'),
                ),
              ],
            ),
          ],
        ),
      ),
    );
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
