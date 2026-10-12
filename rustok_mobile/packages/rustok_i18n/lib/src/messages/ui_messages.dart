import '../catalog/message_catalog.dart';
import '../locale/fallback_chain.dart';
import '../locale/locale_tag.dart';
import 'resolved_message.dart';

/// Maximum allowable length for translation keys (DoS guard matching the
/// `rustok-ui-i18n::MAX_MESSAGE_KEY_LEN` contract).
const int maxMessageKeyLength = 256;

final RegExp _controlCharPattern = RegExp(r'[\x00-\x1F\x7F-\x9F]');

/// Canonical lower-case catalog form of a locale tag, or `null` if invalid.
String? _canonicalLower(String raw) =>
    LocaleTag.tryParse(raw)?.toCatalogTag().toLowerCase();

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
        trimmedKey.contains(_controlCharPattern)) {
      return _missing(key, fallback);
    }

    final kebabKey = normalizeKebabKey(trimmedKey);
    final dottedKey = _normalizeDottedKey(trimmedKey);

    // Provenance compares canonical forms, so `ru_RU` and `ru-RU` are the same locale.
    final targetKey = _canonicalLower(targetLocale) ?? targetLocale.toLowerCase();
    final defaultKey = _canonicalLower(defaultLocale) ?? defaultLocale.toLowerCase();
    final targetChain =
        localeFallbackChain(targetLocale).map((c) => c.toLowerCase()).toSet();

    for (final candidate in candidates) {
      // Try exact key, then kebab-case, then dotted key.
      String? value = _lookup(candidate, trimmedKey, attribute, args);
      if (value == null && kebabKey != trimmedKey) {
        value = _lookup(candidate, kebabKey, attribute, args);
      }
      if (value == null && dottedKey != trimmedKey) {
        value = _lookup(candidate, dottedKey, attribute, args);
      }

      // Dotted attribute syntax (e.g. 'login-input.placeholder') when no attribute was given.
      if (value == null && attribute == null && trimmedKey.contains('.')) {
        final lastDot = trimmedKey.lastIndexOf('.');
        final msgPart = trimmedKey.substring(0, lastDot);
        final attrPart = trimmedKey.substring(lastDot + 1);
        if (msgPart.isNotEmpty && attrPart.isNotEmpty) {
          value = _lookup(candidate, msgPart, attrPart, args);
          final kebabMsg = normalizeKebabKey(msgPart);
          if (value == null && kebabMsg != msgPart) {
            value = _lookup(candidate, kebabMsg, attrPart, args);
          }
        }
      }

      if (value != null) {
        final candidateKey = _canonicalLower(candidate) ?? candidate.toLowerCase();
        final ResolutionProvenance provenance;
        if (candidateKey == targetKey) {
          provenance = ResolutionProvenance.exactMatch;
        } else if (candidateKey == defaultKey || !targetChain.contains(candidateKey)) {
          // Came from the default or platform chain, not from the requested locale.
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

    return _missing(key, fallback);
  }

  /// Looks up one key (and optional attribute) in one candidate locale.
  /// An attribute given in kebab-case also matches an underscore or dotted spelling.
  String? _lookup(
    String candidate,
    String key,
    String? attribute,
    Map<String, dynamic>? args,
  ) {
    final direct = catalog.getMessage(candidate, key, args: args, attribute: attribute);
    if (direct != null || attribute == null) return direct;

    final kebabAttribute = normalizeKebabKey(attribute);
    if (kebabAttribute == attribute) return null;
    return catalog.getMessage(candidate, key, args: args, attribute: kebabAttribute);
  }

  ResolvedMessage _missing(String key, String? fallback) {
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

  String _normalizeDottedKey(String input) =>
      input.replaceAll('-', '.').replaceAll('_', '.');
}

/// Normalizes a message key by replacing dots and underscores with hyphens (kebab-case).
///
/// A superset of `rustok-ui-i18n::with_kebab_key`, which only maps `.`; the
/// underscore spelling is accepted as an additional alias for snake_case keys.
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
