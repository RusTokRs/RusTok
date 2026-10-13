import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../widgets/fluent_scope.dart';
import 'fluent_providers.dart';

/// Publishes Riverpod state through [FluentScope].
///
/// Riverpod ([currentLocaleProvider], [uiMessagesProvider]) is the single source
/// of truth for the locale and messages. This widget mirrors them into the widget
/// tree so that `context.t(...)` and [DirectionalityHandler] keep working.
///
/// With a `LazyCatalog`, the current locale is loaded through [localeReadyProvider].
/// Dependents rebuild when loading completes. If [loading] is given, it is shown
/// (inside the scope) while the locale is loading.
///
/// Place it inside [ProviderScope] and above `MaterialApp`:
/// ```dart
/// ProviderScope(
///   overrides: [messageCatalogProvider.overrideWithValue(catalog)],
///   child: RiverpodFluentScope(child: MyApp()),
/// );
/// ```
class RiverpodFluentScope extends ConsumerStatefulWidget {
  const RiverpodFluentScope({super.key, required this.child, this.loading});

  final Widget child;

  /// Shown while the active locale is loading. Defaults to [child].
  final Widget? loading;

  @override
  ConsumerState<RiverpodFluentScope> createState() => _RiverpodFluentScopeState();
}

class _RiverpodFluentScopeState extends ConsumerState<RiverpodFluentScope> {
  int _revision = 0;
  Object? _lastReady;
  Future<void>? _lastFuture;

  @override
  Widget build(BuildContext context) {
    final messages = ref.watch(uiMessagesProvider);
    final locale = ref.watch(currentLocaleProvider);
    final provider = localeReadyProvider(locale);
    final ready = ref.watch(provider);
    final future = ref.watch(provider.future);
    // Riverpod removes build-scoped listeners before a rebuild and flushes a
    // provider before attaching a new listener. A synchronous refresh can land
    // in that gap. Compare snapshots in build instead of counting callbacks;
    // the future identity also distinguishes retries completed between frames.
    if (!identical(ready, _lastReady) || !identical(future, _lastFuture)) {
      _revision++;
      _lastReady = ready;
      _lastFuture = future;
    }
    final showLoading = ready.isLoading && widget.loading != null;

    return FluentScope(
      messages: messages,
      currentLocale: locale,
      revision: _revision,
      child: showLoading ? widget.loading! : widget.child,
    );
  }
}
