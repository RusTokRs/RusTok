import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../widgets/fluent_scope.dart';
import 'fluent_providers.dart';

/// Publishes Riverpod state through [FluentScope].
///
/// Riverpod ([currentLocaleProvider], [uiMessagesProvider]) is the single source
/// of truth for the locale and messages; this widget only mirrors them into the
/// widget tree so that `context.t(...)` and [DirectionalityHandler] keep working.
///
/// Place it inside [ProviderScope] and above `MaterialApp`:
/// ```dart
/// ProviderScope(
///   overrides: [messageCatalogProvider.overrideWithValue(catalog)],
///   child: RiverpodFluentScope(child: MyApp()),
/// );
/// ```
class RiverpodFluentScope extends ConsumerWidget {
  const RiverpodFluentScope({super.key, required this.child});

  final Widget child;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final messages = ref.watch(uiMessagesProvider);
    final locale = ref.watch(currentLocaleProvider);
    return FluentScope(
      messages: messages,
      currentLocale: locale,
      child: child,
    );
  }
}
