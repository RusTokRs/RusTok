import 'package:flutter/widgets.dart';
import 'package:rustok_i18n/rustok_i18n.dart';

/// An InheritedWidget that provides [UiMessages] and active locale context down the widget tree.
class FluentScope extends InheritedWidget {
  const FluentScope({
    super.key,
    required this.messages,
    required this.currentLocale,
    required super.child,
  });

  final UiMessages messages;
  final String currentLocale;

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
        messages != oldWidget.messages;
  }
}

/// A wrapper widget that automatically wraps its child in Flutter [Directionality]
/// according to the active locale detected by [FluentScope].
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
