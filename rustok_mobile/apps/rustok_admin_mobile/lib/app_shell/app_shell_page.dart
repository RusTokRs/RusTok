import 'package:flutter/material.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'auth_bootstrap.dart';

class AppShellPage extends ConsumerWidget {
  const AppShellPage({super.key, required this.child});

  final Widget child;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final probe = ref.watch(authBootstrapProbeProvider);
    return Scaffold(
      appBar: AppBar(title: const Text('RusTok Admin Mobile')),
      body: probe.when(
        data: (data) {
          if (!data.isAuthenticated) {
            return const _UnauthenticatedView();
          }
          return Column(
            children: [
              if (kDebugMode) _AuthContextBanner(data: data),
              Expanded(child: child),
            ],
          );
        },
        loading: () => const Center(child: CircularProgressIndicator()),
        error: (_, _) => _BootstrapErrorView(
          onRetry: () {
            ref.invalidate(authSessionProvider);
            ref.invalidate(authBootstrapProbeProvider);
          },
        ),
      ),
    );
  }
}

class _UnauthenticatedView extends ConsumerWidget {
  const _UnauthenticatedView();

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    return Center(
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          const Text(
            'No active session. Provide valid auth session in host bootstrap.',
            textAlign: TextAlign.center,
          ),
          const SizedBox(height: 12),
          FilledButton(
            onPressed: () {
              ref.invalidate(authSessionProvider);
              ref.invalidate(authBootstrapProbeProvider);
            },
            child: const Text('Retry session check'),
          ),
        ],
      ),
    );
  }
}

class _BootstrapErrorView extends StatelessWidget {
  const _BootstrapErrorView({required this.onRetry});

  final VoidCallback onRetry;

  @override
  Widget build(BuildContext context) {
    return Center(
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          const Text('Failed to initialize host context.'),
          const SizedBox(height: 12),
          FilledButton(onPressed: onRetry, child: const Text('Retry bootstrap')),
        ],
      ),
    );
  }
}

class _AuthContextBanner extends StatelessWidget {
  const _AuthContextBanner({required this.data});

  final BootstrapProbeResult data;

  @override
  Widget build(BuildContext context) {
    final meEmail = data.userEmail ?? 'unknown';
    final tenantSlug = data.tenantSlug ?? 'unknown';
    return Material(
      color: Theme.of(context).colorScheme.surfaceContainerHighest,
      child: ListTile(
        dense: true,
        title: Text('me: $meEmail'),
        subtitle: Text('tenant: $tenantSlug'),
      ),
    );
  }
}
