import '../catalog/message_catalog.dart';
import '../locale/fallback_chain.dart';
import 'resolved_message.dart';

/// Maximum allowable length for translation keys (DoS guard matching RusToK contract).
const int maxMessageKeyLength = 128;

/// Main high-level facade for resolving internationalized messages across catalogs and locales.
class UiMessages {
  UiMessages({
    required this.catalog,
    this.defaultLocale = 'en',
    this.platformFallback = 'en',
  });

  final MessageCatalog catalog;
  final String defaultLocale;
  final String platformFallback;

  /// Resolves a message string with full provenance metadata.
  ResolvedMessage resolve(
    String key, {
    String? locale,
    Map<String, dynamic>? args,
    String? attribute,
    String? fallback,
  }) {
    final targetLocale = locale ?? defaultLocale;
    final candidates = localeCandidates(
      locale: targetLocale,
      defaultLocale: defaultLocale,
      platformFallback: platformFallback,
    );

    return _resolveWithCandidates(
      key,
      candidates: candidates,
      targetLocale: targetLocale,
      args: args,
      attribute: attribute,
      fallback: fallback,
    );
  }

  ResolvedMessage _resolveWithCandidates(
    String key, {
    required List<String> candidates,
    required String targetLocale,
    Map<String, dynamic>? args,
    String? attribute,
    String? fallback,
  }) {
    final trimmedKey = key.trim();
    if (trimmedKey.isEmpty ||
        trimmedKey.length > maxMessageKeyLength ||
        trimmedKey.contains(RegExp(r'[\x00-\x1F\x7F-\x9F]'))) {
      return ResolvedMessage(
        value: fallback ?? key,
        provenance: ResolutionProvenance.missingKeyFallback,
        originalKey: key,
      );
    }

    final kebabKey = _normalizeKebabKey(trimmedKey);
    final dottedKey = _normalizeDottedKey(trimmedKey);

    for (int i = 0; i < candidates.length; i++) {
      final candidate = candidates[i];

      // Try exact key, then kebab-case, then dotted key
      String? value = catalog.getMessage(
        candidate,
        trimmedKey,
        args: args,
        attribute: attribute,
      );

      if (value == null && kebabKey != trimmedKey) {
        value = catalog.getMessage(
          candidate,
          kebabKey,
          args: args,
          attribute: attribute,
        );
      }

      if (value == null && dottedKey != trimmedKey) {
        value = catalog.getMessage(
          candidate,
          dottedKey,
          args: args,
          attribute: attribute,
        );
      }

      // Check for dotted attribute syntax (e.g. 'login-input.placeholder') when attribute is not explicitly provided
      if (value == null && attribute == null && trimmedKey.contains('.')) {
        final lastDot = trimmedKey.lastIndexOf('.');
        final msgPart = trimmedKey.substring(0, lastDot);
        final attrPart = trimmedKey.substring(lastDot + 1);
        if (msgPart.isNotEmpty && attrPart.isNotEmpty) {
          value = catalog.getMessage(
            candidate,
            msgPart,
            args: args,
            attribute: attrPart,
          );
          if (value == null) {
            final kebabMsg = _normalizeKebabKey(msgPart);
            if (kebabMsg != msgPart) {
              value = catalog.getMessage(
                candidate,
                kebabMsg,
                args: args,
                attribute: attrPart,
              );
            }
          }
        }
      }

      if (value != null) {
        ResolutionProvenance provenance;
        if (candidate.toLowerCase() == targetLocale.toLowerCase()) {
          provenance = ResolutionProvenance.exactMatch;
        } else if (candidate.toLowerCase() == defaultLocale.toLowerCase()) {
          provenance = ResolutionProvenance.defaultLocale;
        } else {
          provenance = ResolutionProvenance.parentLanguage;
        }

        return ResolvedMessage(
          value: value,
          resolvedLocale: candidate,
          provenance: provenance,
          originalKey: key,
        );
      }
    }

    return ResolvedMessage(
      value: fallback ?? key,
      provenance: ResolutionProvenance.missingKeyFallback,
      originalKey: key,
    );
  }

  /// Shorthand helper returning just the resolved string value.
  String t(
    String key, {
    String? locale,
    Map<String, dynamic>? args,
    String? fallback,
  }) {
    return resolve(
      key,
      locale: locale,
      args: args,
      fallback: fallback,
    ).value;
  }

  /// Shorthand helper for resolving an attribute (e.g. `.placeholder` or `.aria-label`).
  String tAttr(
    String key,
    String attribute, {
    String? locale,
    Map<String, dynamic>? args,
    String? fallback,
  }) {
    return resolve(
      key,
      locale: locale,
      attribute: attribute,
      args: args,
      fallback: fallback,
    ).value;
  }

  /// Attempts to resolve a message. Returns `null` if missing across all fallback candidates.
  ResolvedMessage? tryResolve(
    String key, {
    String? locale,
    Map<String, dynamic>? args,
    String? attribute,
  }) {
    final res = resolve(
      key,
      locale: locale,
      args: args,
      attribute: attribute,
    );
    if (res.provenance == ResolutionProvenance.missingKeyFallback) {
      return null;
    }
    return res;
  }

  /// Returns a lightweight translator bound to a specific locale,
  /// caching candidate fallback computation for fast batch lookups.
  /// Matches `rustok-ui-i18n::UiTranslator::for_locale`.
  UiLocaleTranslator forLocale([String? locale]) {
    return UiLocaleTranslator(
      messages: this,
      locale: locale,
    );
  }

  String _normalizeKebabKey(String input) => normalizeKebabKey(input);

  String _normalizeDottedKey(String input) =>
      input.replaceAll('-', '.').replaceAll('_', '.');
}

/// Normalizes a message key by replacing dots and underscores with hyphens (kebab-case).
/// Direct parity with `rustok-ui-i18n::with_kebab_key`.
String normalizeKebabKey(String input) =>
    input.replaceAll('.', '-').replaceAll('_', '-');

/// A lightweight translator bound to a single effective locale.
///
/// Pre-computes and reuses the locale fallback candidate chain to avoid
/// reallocating candidate vectors during batch UI rendering.
/// Direct parity with `rustok-ui-i18n::UiLocaleTranslator`.
class UiLocaleTranslator {
  UiLocaleTranslator({
    required this.messages,
    String? locale,
  })  : locale = locale ?? messages.defaultLocale,
        candidates = localeCandidates(
          locale: locale ?? messages.defaultLocale,
          defaultLocale: messages.defaultLocale,
          platformFallback: messages.platformFallback,
        );

  final UiMessages messages;
  final String locale;
  final List<String> candidates;

  /// Translates a message key using the bound locale and precomputed candidate chain.
  String t(
    String key, {
    Map<String, dynamic>? args,
    String? fallback,
  }) =>
      resolve(
        key,
        args: args,
        fallback: fallback,
      ).value;

  /// Translates an attribute using the bound locale and precomputed candidate chain.
  String tAttr(
    String key,
    String attribute, {
    Map<String, dynamic>? args,
    String? fallback,
  }) =>
      resolve(
        key,
        attribute: attribute,
        args: args,
        fallback: fallback,
      ).value;

  /// Resolves a message with full provenance metadata using the precomputed candidate chain.
  ResolvedMessage resolve(
    String key, {
    Map<String, dynamic>? args,
    String? attribute,
    String? fallback,
  }) =>
      messages._resolveWithCandidates(
        key,
        candidates: candidates,
        targetLocale: locale,
        args: args,
        attribute: attribute,
        fallback: fallback,
      );

  /// Attempts to resolve a message, returning `null` if absent.
  ResolvedMessage? tryResolve(
    String key, {
    Map<String, dynamic>? args,
    String? attribute,
  }) {
    final res = resolve(
      key,
      args: args,
      attribute: attribute,
    );
    if (res.provenance == ResolutionProvenance.missingKeyFallback) {
      return null;
    }
    return res;
  }
}

