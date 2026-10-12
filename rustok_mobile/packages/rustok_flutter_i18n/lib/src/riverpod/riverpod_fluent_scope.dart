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
/// With a [LazyCatalog], the current locale is loaded through [localeReadyProvider].
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
class RiverpodFluentScope extends ConsumerWidget {
  const RiverpodFluentScope({super.key, required this.child, this.loading});

  final Widget child;

  /// Shown while the active locale is loading. Defaults to [child].
  final Widget? loading;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final messages = ref.watch(uiMessagesProvider);
    final locale = ref.watch(currentLocaleProvider);
    final ready = ref.watch(localeReadyProvider(locale));
    final showLoading = ready.isLoading && loading != null;

    return FluentScope(
      messages: messages,
      currentLocale: locale,
      revision: ready.isLoading ? 0 : 1,
      child: showLoading ? loading! : child,
    );
  }
}
