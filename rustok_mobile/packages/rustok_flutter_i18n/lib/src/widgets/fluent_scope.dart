import 'package:flutter/widgets.dart';
import 'package:rustok_i18n/rustok_i18n.dart';

/// An InheritedWidget that provides [UiMessages] and active locale context down the widget tree.
class FluentScope extends InheritedWidget {
  const FluentScope({
    super.key,
    required this.messages,
    required this.currentLocale,
    this.revision = 0,
    required super.child,
  });

  final UiMessages messages;
  final String currentLocale;

  /// Changes whenever the messages become available (for example after a
  /// lazy catalog finishes loading). Dependents rebuild when it changes.
  final int revision;

  static FluentScope? maybeOf(BuildContext context) {
    return context.dependOnInheritedWidgetOfExactType<FluentScope>();
  }

  static FluentScope of(BuildContext context) {
    final scope = maybeOf(context);
    if (scope == null) {
      throw StateError(
        'FluentScope.of() called with a BuildContext that does not contain a FluentScope.\n'
        'Ensure that FluentScope is mounted higher up in the widget hierarchy.',
      );
    }
    return scope;
  }

  /// Returns whether the current locale uses right-to-left writing direction.
  bool get isRtl {
    final tag = LocaleTag.tryParse(currentLocale);
    return tag?.isRtl ?? false;
  }

  /// Returns Flutter framework [TextDirection] for the active locale.
  TextDirection get flutterTextDirection =>
      isRtl ? TextDirection.rtl : TextDirection.ltr;

  @override
  bool updateShouldNotify(FluentScope oldWidget) {
    return currentLocale != oldWidget.currentLocale ||
        messages != oldWidget.messages ||
        revision != oldWidget.revision;
  }
}

/// Wraps its child in Flutter [Directionality] according to the active locale
/// published by the nearest [FluentScope].
///
/// The handler must be placed below a [FluentScope]. With `MaterialApp` this means
/// using `MaterialApp.builder` (the scope itself must wrap `MaterialApp`):
/// ```dart
/// MaterialApp(
///   builder: (context, child) => DirectionalityHandler(child: child!),
/// );
/// ```
/// Without a scope it falls back to LTR.
class DirectionalityHandler extends StatelessWidget {
  const DirectionalityHandler({
    super.key,
    required this.child,
  });

  final Widget child;

  @override
  Widget build(BuildContext context) {
    final scope = FluentScope.maybeOf(context);
    final direction = scope?.flutterTextDirection ?? TextDirection.ltr;
    return Directionality(
      textDirection: direction,
      child: child,
    );
  }
}
