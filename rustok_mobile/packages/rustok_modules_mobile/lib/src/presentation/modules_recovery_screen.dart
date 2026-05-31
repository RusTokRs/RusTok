import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../modules_controller.dart';
import '../modules_repository.dart';

class ModulesRecoveryScreen extends ConsumerStatefulWidget {
  const ModulesRecoveryScreen({
    super.key,
    required this.moduleSlug,
    this.limit = 20,
    this.onBackToModules,
  });

  final String moduleSlug;
  final int limit;
  final VoidCallback? onBackToModules;

  @override
  ConsumerState<ModulesRecoveryScreen> createState() =>
      _ModulesRecoveryScreenState();
}

class _ModulesRecoveryScreenState extends ConsumerState<ModulesRecoveryScreen> {
  late Future<List<ModuleOperationRecoveryPlan>> _plansFuture;
  String? _runningOperationId;
  Object? _actionError;

  @override
  void initState() {
    super.initState();
    _plansFuture = _loadPlans();
  }

  @override
  void didUpdateWidget(covariant ModulesRecoveryScreen oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.moduleSlug != widget.moduleSlug ||
        oldWidget.limit != widget.limit) {
      _plansFuture = _loadPlans();
      _actionError = null;
      _runningOperationId = null;
    }
  }

  @override
  Widget build(BuildContext context) {
    return FutureBuilder<List<ModuleOperationRecoveryPlan>>(
      future: _plansFuture,
      builder: (context, snapshot) {
        final plans = snapshot.data;
        return RefreshIndicator(
          onRefresh: _reload,
          child: ListView(
            physics: const AlwaysScrollableScrollPhysics(),
            children: [
              ListTile(
                leading: IconButton(
                  tooltip: 'Back to modules',
                  onPressed: widget.onBackToModules,
                  icon: const Icon(Icons.arrow_back),
                ),
                title: Text('${widget.moduleSlug} recovery'),
                subtitle: const Text(
                  'Failed lifecycle operations and canonical recovery actions.',
                ),
                trailing: IconButton(
                  tooltip: 'Refresh recovery history',
                  onPressed: () => setState(() {
                    _actionError = null;
                    _plansFuture = _loadPlans();
                  }),
                  icon: const Icon(Icons.refresh),
                ),
              ),
              if (_actionError != null)
                Padding(
                  padding: const EdgeInsets.symmetric(horizontal: 16),
                  child: Text(
                    'Recovery action failed: $_actionError',
                    style: TextStyle(color: Theme.of(context).colorScheme.error),
                  ),
                ),
              if (snapshot.connectionState == ConnectionState.waiting &&
                  plans == null)
                const Padding(
                  padding: EdgeInsets.all(32),
                  child: Center(child: CircularProgressIndicator()),
                )
              else if (snapshot.hasError)
                _RecoveryHistoryError(
                  error: snapshot.error!,
                  onRetry: () => setState(() {
                    _actionError = null;
                    _plansFuture = _loadPlans();
                  }),
                )
              else if (plans == null || plans.isEmpty)
                const _EmptyRecoveryHistory()
              else
                for (final plan in plans)
                  _RecoveryOperationCard(
                    plan: plan,
                    isBusy: _runningOperationId == plan.operationId,
                    onRetry: plan.retryable
                        ? () => _retryPostHook(plan.operationId)
                        : null,
                    onCompensate: () => _compensateOperation(plan.operationId),
                  ),
            ],
          ),
        );
      },
    );
  }

  Future<List<ModuleOperationRecoveryPlan>> _loadPlans() {
    return ref.read(modulesRepositoryProvider).failedRecoveryPlans(
          moduleSlug: widget.moduleSlug,
          limit: widget.limit,
        );
  }

  Future<void> _reload() async {
    final future = _loadPlans();
    setState(() {
      _actionError = null;
      _plansFuture = future;
    });
    await future;
  }

  Future<void> _retryPostHook(String operationId) async {
    await _runRecoveryAction(
      operationId,
      () => ref
          .read(modulesRepositoryProvider)
          .retryFailedPostHook(operationId: operationId),
    );
  }

  Future<void> _compensateOperation(String operationId) async {
    await _runRecoveryAction(
      operationId,
      () => ref
          .read(modulesRepositoryProvider)
          .compensateFailedOperation(operationId: operationId),
    );
  }

  Future<void> _runRecoveryAction(
    String operationId,
    Future<Object?> Function() action,
  ) async {
    if (_runningOperationId != null) {
      return;
    }

    setState(() {
      _runningOperationId = operationId;
      _actionError = null;
    });

    try {
      await action();
      ref.invalidate(modulesControllerProvider);
      await _reload();
    } catch (error) {
      if (mounted) {
        setState(() => _actionError = error);
      }
    } finally {
      if (mounted) {
        setState(() => _runningOperationId = null);
      }
    }
  }
}

class _RecoveryOperationCard extends StatelessWidget {
  const _RecoveryOperationCard({
    required this.plan,
    required this.isBusy,
    required this.onRetry,
    required this.onCompensate,
  });

  final ModuleOperationRecoveryPlan plan;
  final bool isBusy;
  final VoidCallback? onRetry;
  final VoidCallback onCompensate;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final requestedState = plan.requestedEnabled ? 'enable' : 'disable';
    final previousState = plan.previousEffectiveEnabled ? 'enabled' : 'disabled';

    return Card(
      margin: const EdgeInsets.symmetric(horizontal: 12, vertical: 6),
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Row(
              children: [
                Expanded(
                  child: Text(
                    'Operation ${plan.operationId}',
                    style: theme.textTheme.titleMedium,
                  ),
                ),
                Chip(label: Text(plan.status.isEmpty ? 'unknown' : plan.status)),
              ],
            ),
            const SizedBox(height: 8),
            Text('Requested: $requestedState module ${plan.moduleSlug}'),
            Text('Previous effective state: $previousState'),
            Text('Issue: ${plan.issue.isEmpty ? 'unknown' : plan.issue}'),
            Text('Recommended action: ${_recommendedAction(plan)}'),
            if (plan.errorMessage != null) Text('Error: ${plan.errorMessage}'),
            if (plan.correlationId != null)
              Text('Correlation ID: ${plan.correlationId}'),
            if (plan.requestedBy != null) Text('Requested by: ${plan.requestedBy}'),
            const SizedBox(height: 12),
            Wrap(
              spacing: 8,
              runSpacing: 8,
              children: [
                FilledButton.tonalIcon(
                  onPressed: isBusy || onRetry == null ? null : onRetry,
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

String _recommendedAction(ModuleOperationRecoveryPlan plan) {
  return plan.recommendedAction.isEmpty ? 'review' : plan.recommendedAction;
}

class _EmptyRecoveryHistory extends StatelessWidget {
  const _EmptyRecoveryHistory();

  @override
  Widget build(BuildContext context) {
    return const Padding(
      padding: EdgeInsets.all(24),
      child: Center(
        child: Text('No failed module operations require recovery.'),
      ),
    );
  }
}

class _RecoveryHistoryError extends StatelessWidget {
  const _RecoveryHistoryError({required this.error, required this.onRetry});

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
            const Text('Failed to load recovery history.'),
            const SizedBox(height: 8),
            Text('$error', textAlign: TextAlign.center),
            const SizedBox(height: 12),
            FilledButton(onPressed: onRetry, child: const Text('Retry')),
          ],
        ),
      ),
    );
  }
}
