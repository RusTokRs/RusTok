import '../locale/fallback_chain.dart';
import '../locale/locale_tag.dart';

const int maxAcceptLanguageLength = 4096;
const int maxAcceptLanguageRanges = 64;

/// Represents a single parsed item from the HTTP `Accept-Language` header.
class AcceptLanguagePreference {
  const AcceptLanguagePreference({
    required this.tag,
    required this.quality,
    this.order = 0,
  });

  /// The raw language range (e.g. `ru-RU`, `en`, `*`).
  final String tag;

  /// Quality weight scaled to thousandths (1.0 = 1000, 0.8 = 800).
  final int quality;

  /// Original 0-based appearance index in header for strictly stable RFC sort.
  final int order;

  bool get isWildcard => tag == '*';
}

/// Parses an HTTP `Accept-Language` header string into prioritized [LocaleTag]s.
///
/// Bounded to 4096 bytes and 64 ranges to guarantee safe execution. Ranges that
/// are explicitly unacceptable (`q=0`) and ranges with a malformed quality or a
/// duplicated `q` parameter are omitted, so one broken client preference cannot
/// discard later valid ones.
List<AcceptLanguagePreference> parseAcceptLanguage(String? header) {
  if (header == null) return const [];
  final trimmed = header.trim();
  if (trimmed.isEmpty || trimmed.length > maxAcceptLanguageLength) {
    return const [];
  }

  final rawRanges = trimmed.split(',');
  final preferences = <AcceptLanguagePreference>[];
  int order = 0;

  for (final rawRange in rawRanges) {
    if (preferences.length >= maxAcceptLanguageRanges) break;
    final part = rawRange.trim();
    if (part.isEmpty) continue;

    // The appearance index is captured before any validation so that it stays
    // the true position of the range in the header.
    final sourceIndex = order++;

    final subParts = part.split(';');
    final tag = subParts.first.trim();
    if (tag.isEmpty) continue;

    int quality = 1000;
    var qualitySeen = false;
    var malformed = false;

    for (int i = 1; i < subParts.length; i++) {
      final param = subParts[i].trim();
      final separator = param.indexOf('=');
      final name = (separator < 0 ? param : param.substring(0, separator)).trim();
      if (name.toLowerCase() != 'q') continue;

      if (separator < 0 || qualitySeen) {
        // A bare `q` or a duplicated quality parameter is malformed.
        malformed = true;
        break;
      }
      qualitySeen = true;

      final parsed = _parseQuality(param.substring(separator + 1).trim());
      if (parsed == null) {
        malformed = true;
        break;
      }
      quality = parsed;
    }

    if (malformed || quality <= 0) continue;

    preferences.add(AcceptLanguagePreference(
      tag: tag,
      quality: quality,
      order: sourceIndex,
    ));
  }

  // Strictly stable sort: by quality descending, then by appearance order ascending
  preferences.sort((a, b) {
    final cmp = b.quality.compareTo(a.quality);
    if (cmp != 0) return cmp;
    return a.order.compareTo(b.order);
  });
  return List.unmodifiable(preferences);
}

/// Parses an RFC 9110 quality value into thousandths, or `null` when malformed.
///
/// Mirrors `rustok-ui-i18n::parse_quality`: only `0`, `1`, `0.<1..3 digits>` and
/// `1.000` are accepted, so values such as `NaN`, `.5`, `1.001` or `0.1234` are
/// rejected without relying on floating-point parsing.
int? _parseQuality(String raw) {
  if (raw == '0') return 0;
  if (raw == '1') return 1000;

  final dot = raw.indexOf('.');
  if (dot < 0) return null;

  final whole = raw.substring(0, dot);
  final fraction = raw.substring(dot + 1);
  if (fraction.length > 3) return null;
  for (final unit in fraction.codeUnits) {
    if (unit < 0x30 || unit > 0x39) return null;
  }

  if (whole == '0') {
    final digits = fraction.isEmpty ? 0 : int.parse(fraction);
    var quality = digits;
    for (var i = fraction.length; i < 3; i++) {
      quality *= 10;
    }
    return quality;
  }

  if (whole == '1') {
    final isZeroFraction =
        fraction.codeUnits.every((unit) => unit == 0x30);
    return isZeroFraction ? 1000 : null;
  }

  return null;
}

/// Picks the best supported locale for an `Accept-Language` header.
///
/// Matching is done in quality order. For each requested range the exact tag
/// is tried first, then its less specific forms (`zh-Hant-TW` -> `zh-Hant` -> `zh`),
/// then any supported locale with the same primary language (`ru` -> `ru-RU`).
/// A `*` range is skipped. Returns [defaultLocale] (or the first supported
/// locale) when no explicit range matches.
String? negotiateLocale({
  required String? acceptLanguageHeader,
  required List<String> supportedLocales,
  String defaultLocale = 'en',
}) {
  final safeFallback = supportedLocales.contains(defaultLocale)
      ? defaultLocale
      : (supportedLocales.isNotEmpty ? supportedLocales.first : defaultLocale);

  final preferences = parseAcceptLanguage(acceptLanguageHeader);
  if (preferences.isEmpty) return safeFallback;

  // Canonical lower-case tag -> original spelling supplied by the caller.
  final canonicalToOriginal = <String, String>{};
  for (final supported in supportedLocales) {
    final canonical = LocaleTag.tryParse(supported)?.toCatalogTag().toLowerCase();
    if (canonical != null) {
      canonicalToOriginal.putIfAbsent(canonical, () => supported);
    }
  }

  for (final pref in preferences) {
    // `*` accepts anything, so it only means "use the default" when no explicit range matches.
    if (pref.isWildcard) continue;
    final tag = LocaleTag.tryParse(pref.tag);
    if (tag == null) continue;

    // 1. Exact tag, then progressively less specific forms.
    for (final candidate in localeFallbackChain(pref.tag)) {
      final match = canonicalToOriginal[candidate.toLowerCase()];
      if (match != null) return match;
    }

    // 2. Same primary language, e.g. `ru` matches a supported `ru-RU`.
    final language = tag.language.toLowerCase();
    for (final entry in canonicalToOriginal.entries) {
      if (entry.key.split('-').first == language) return entry.value;
    }
  }

  return safeFallback;
}

/// Returns accepted complete Unicode locales in quality order.
///
/// Wildcards and quality=0 ranges are omitted, duplicates keep their
/// highest-quality earliest occurrence, and every tag is canonicalized
/// (`iw` becomes `he`). Ranges that are not valid BCP-47 tags are skipped, so
/// this mirrors `rustok-ui-i18n::accept_language_locales` with the package's own
/// BCP-47 parser instead of ICU4X locale canonicalization.
List<String> acceptLanguageLocales(String? header) {
  final preferences = parseAcceptLanguage(header);
  final locales = <String>[];
  for (final pref in preferences) {
    if (pref.isWildcard) continue;
    final tag = LocaleTag.tryParse(pref.tag);
    if (tag == null) continue;
    final full = tag.toLanguageTag();
    if (!locales.contains(full)) {
      locales.add(full);
    }
  }
  return List.unmodifiable(locales);
}

/// Returns accepted extension-free Fluent catalog identities in quality order.
///
/// Direct parity with `rustok-ui-i18n::accept_language_catalog_locales`.
List<String> acceptLanguageCatalogLocales(String? header) {
  final preferences = parseAcceptLanguage(header);
  final locales = <String>[];
  for (final pref in preferences) {
    if (pref.isWildcard) continue;
    final tag = LocaleTag.tryParse(pref.tag);
    if (tag == null) continue;
    final cat = tag.toCatalogTag();
    if (!locales.contains(cat)) {
      locales.add(cat);
    }
  }
  return List.unmodifiable(locales);
}

/// Returns the highest-quality complete Unicode locale from an optional field value.
///
/// Direct parity with `rustok-ui-i18n::preferred_locale_from_accept_language`.
String? preferredLocaleFromAcceptLanguage(String? header) {
  final locales = acceptLanguageLocales(header);
  return locales.isNotEmpty ? locales.first : null;
}

/// Returns the highest-quality extension-free catalog locale from an optional field value.
///
/// Direct parity with `rustok-ui-i18n::preferred_catalog_locale_from_accept_language`.
String? preferredCatalogLocaleFromAcceptLanguage(String? header) {
  final locales = acceptLanguageCatalogLocales(header);
  return locales.isNotEmpty ? locales.first : null;
}

