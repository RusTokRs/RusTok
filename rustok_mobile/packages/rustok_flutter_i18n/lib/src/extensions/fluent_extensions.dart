import 'package:flutter/widgets.dart';
import 'package:rustok_i18n/rustok_i18n.dart';
import '../widgets/fluent_scope.dart';

/// Ergonomic Flutter [BuildContext] extensions for translation lookups.
extension FluentBuildContext on BuildContext {
  /// Resolves a localized string using the nearest [FluentScope].
  String t(
    String key, {
    Map<String, dynamic>? args,
    String? fallback,
  }) {
    final scope = FluentScope.of(this);
    return scope.messages.t(
      key,
      locale: scope.currentLocale,
      args: args,
      fallback: fallback,
    );
  }

  /// Safely resolves a localized string or returns [fallback] / [key] if no [FluentScope] exists.
  String maybeT(
    String key, {
    Map<String, dynamic>? args,
    String? fallback,
  }) {
    final scope = FluentScope.maybeOf(this);
    if (scope == null) return fallback ?? key;
    return scope.messages.t(
      key,
      locale: scope.currentLocale,
      args: args,
      fallback: fallback,
    );
  }

  /// Resolves an attribute of a localized message (e.g. `.placeholder`, `.aria-label`).
  String tAttr(
    String key,
    String attribute, {
    Map<String, dynamic>? args,
    String? fallback,
  }) {
    final scope = FluentScope.of(this);
    return scope.messages.tAttr(
      key,
      attribute,
      locale: scope.currentLocale,
      args: args,
      fallback: fallback,
    );
  }

  /// Resolves a localized message with full provenance metadata.
  ResolvedMessage resolveMessage(
    String key, {
    Map<String, dynamic>? args,
    String? attribute,
    String? fallback,
  }) {
    final scope = FluentScope.of(this);
    return scope.messages.resolve(
      key,
      locale: scope.currentLocale,
      args: args,
      attribute: attribute,
      fallback: fallback,
    );
  }

  /// The active locale string from [FluentScope].
  String get activeLocale => FluentScope.of(this).currentLocale;

  /// Whether the active locale in [FluentScope] is right-to-left.
  bool get isRtlLocale => FluentScope.of(this).isRtl;
}
