import 'dart:convert';

import '../locale/fallback_chain.dart';
import '../locale/locale_tag.dart';
import '../locale/likely_subtags.dart';

const int maxAcceptLanguageLength = 4096;
const int maxAcceptLanguageRanges = 64;

/// Which raw-input bound rejected an Accept-Language field value.
enum AcceptLanguageLimit { headerBytes, ranges }

/// A whole-header resource-limit failure, not a malformed individual range.
class AcceptLanguageLimitException implements Exception {
  const AcceptLanguageLimitException(this.limit);

  final AcceptLanguageLimit limit;

  @override
  String toString() => limit == AcceptLanguageLimit.headerBytes
      ? 'Accept-Language exceeds $maxAcceptLanguageLength UTF-8 bytes'
      : 'Accept-Language exceeds $maxAcceptLanguageRanges raw ranges';
}

/// Represents a single parsed item from the HTTP `Accept-Language` header.
class AcceptLanguagePreference {
  const AcceptLanguagePreference({
    required this.tag,
    required this.quality,
    this.order = 0,
  });

  /// Canonical complete locale (including extensions), or `*`.
  final String tag;

  /// Quality weight scaled to thousandths (1.0 = 1000, 0.8 = 800).
  final int quality;

  /// Original 0-based appearance index in header for strictly stable RFC sort.
  final int order;

  bool get isWildcard => tag == '*';

  bool get isRejected => quality == 0;
}

/// Accepted preferences in descending quality, with source order breaking ties.
///
/// Compatibility projection: q=0 exclusions are omitted. Whole-header limit
/// failures yield an empty list. Use [parseAcceptLanguageStrict] when exclusions
/// or typed limit diagnostics are needed (for example during negotiation).
List<AcceptLanguagePreference> parseAcceptLanguage(String? header) {
  if (header == null) return const [];
  try {
    return List.unmodifiable(
      parseAcceptLanguageStrict(header).where((preference) => !preference.isRejected),
    );
  } on AcceptLanguageLimitException {
    return const [];
  }
}

/// Parses a bounded raw field value, retaining wildcards and q=0 exclusions.
///
/// Throws [AcceptLanguageLimitException] above 4096 UTF-8 bytes or 64 raw
/// comma-separated ranges, before trimming or filtering. Malformed individual
/// ranges/qualities are skipped. Unknown parameters are ignored, as in the Rust
/// owner contract. Underscore separators are accepted for compatibility, but
/// POSIX encoding/modifier suffixes are not HTTP locale identifiers.
List<AcceptLanguagePreference> parseAcceptLanguageStrict(String header) {
  // UTF-16 length is a cheap lower bound on UTF-8 bytes. Encode only bounded
  // input, avoiding a huge temporary allocation for an oversized raw header.
  if (header.length > maxAcceptLanguageLength ||
      utf8.encode(header).length > maxAcceptLanguageLength) {
    throw const AcceptLanguageLimitException(AcceptLanguageLimit.headerBytes);
  }
  final rawRanges = header.split(',');
  if (rawRanges.length > maxAcceptLanguageRanges) {
    throw const AcceptLanguageLimitException(AcceptLanguageLimit.ranges);
  }

  final preferences = <AcceptLanguagePreference>[];
  for (var sourceIndex = 0; sourceIndex < rawRanges.length; sourceIndex++) {
    final subParts = rawRanges[sourceIndex].trim().split(';');
    final rawTag = subParts.first.trim();
    if (rawTag.isEmpty || rawTag.contains('.') || rawTag.contains('@')) continue;
    final String tag;
    if (rawTag == '*') {
      tag = '*';
    } else {
      final parsed = LocaleTag.tryParse(rawTag);
      if (parsed == null) continue;
      tag = parsed.toLanguageTag();
    }

    var quality = 1000;
    var qualitySeen = false;
    var malformed = false;
    for (var i = 1; i < subParts.length; i++) {
      final param = subParts[i].trim();
      final separator = param.indexOf('=');
      final name = (separator < 0 ? param : param.substring(0, separator)).trim();
      if (name.toLowerCase() != 'q') continue;
      if (separator < 0 || qualitySeen) {
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
    if (malformed) continue;
    preferences.add(AcceptLanguagePreference(
      tag: tag,
      quality: quality,
      order: sourceIndex,
    ));
  }

  preferences.sort((a, b) {
    final cmp = b.quality.compareTo(a.quality);
    if (cmp != 0) return cmp;
    return a.order.compareTo(b.order);
  });
  return List.unmodifiable(preferences);
}

/// Parses an RFC 9110 quality value into thousandths, or `null` when malformed.
///
/// Mirrors `rustok-ui-i18n::parse_quality`: `0`, `1`, `0.<0..3 digits>` and
/// `1.<0..3 zeroes>` are accepted, so values such as `NaN`, `.5`, `1.001` or `0.1234` are
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

// A scriptless language-only catalog is generic. Regional catalogs use CLDR.
String? _matchingScript(LocaleTag tag) {
  if (tag.script != null && tag.script != 'Zzzz') return tag.script;
  if (tag.region == null || tag.region == 'ZZ') return null;
  return maximizeLocale(tag)?.script;
}

/// Picks a supported catalog locale using this library's lookup policy.
///
/// Explicit accepted preferences are tried in quality order: exact/parent
/// fallback chain, then same-language siblings with no conflicting explicit
/// or known region-implied script. Generic language catalogs remain eligible.
/// Wildcards never outrank an explicit preference. If none match, use the
/// canonically matching default, then the first allowed supported locale.
///
/// A q=0 range excludes its locale and descendants. The most-specific matching
/// range wins (a specific positive range can override a broader exclusion).
/// Candidates are ranked by the lower of the requesting range's quality and
/// their most-specific matching range's quality; ties keep lookup order.
/// Canonical duplicates use their highest quality. Exclusions also apply to
/// fallback choices. Returns null if no valid supported locale is allowed;
/// otherwise preserves the caller's original supported-locale spelling.
/// Malformed/oversized headers behave as absent preferences. This is a host
/// convenience, not automatic effective-locale policy for Flutter providers.
String? negotiateLocale({
  required String? acceptLanguageHeader,
  required List<String> supportedLocales,
  String defaultLocale = 'en',
}) {
  List<AcceptLanguagePreference> preferences;
  try {
    preferences = acceptLanguageHeader == null
        ? const []
        : parseAcceptLanguageStrict(acceptLanguageHeader);
  } on AcceptLanguageLimitException {
    preferences = const [];
  }

  // Catalog identities deliberately discard formatting extensions.
  final qualities = <String, int>{};
  for (final preference in preferences) {
    final key = preference.isWildcard
        ? '*'
        : LocaleTag.parse(preference.tag).toCatalogTag().toLowerCase();
    // Preferences are already ordered highest quality first.
    qualities.putIfAbsent(key, () => preference.quality);
  }

  int effectiveQuality(String candidate) {
    var specificity = -1;
    var quality = 1000;
    for (final entry in qualities.entries) {
      final range = entry.key;
      if (range != '*' && candidate != range && !candidate.startsWith('$range-')) {
        continue;
      }
      final current = range == '*' ? 0 : range.split('-').length;
      if (current > specificity) {
        specificity = current;
        quality = entry.value;
      }
    }
    return quality;
  }

  final canonicalToOriginal = <String, String>{};
  final tags = <String, LocaleTag>{};
  final scripts = <String, String?>{};
  final candidateQualities = <String, int>{};
  for (final supported in supportedLocales) {
    final tag = LocaleTag.tryParse(supported);
    if (tag == null) continue;
    final canonical = tag.toCatalogTag().toLowerCase();
    final quality = effectiveQuality(canonical);
    if (quality == 0) continue;
    candidateQualities[canonical] = quality;
    canonicalToOriginal.putIfAbsent(canonical, () => supported);
    tags.putIfAbsent(canonical, () => tag);
    scripts.putIfAbsent(canonical, () => _matchingScript(tag));
  }
  if (canonicalToOriginal.isEmpty) return null;

  String? selected;
  var selectedQuality = -1;
  void consider(String canonical, AcceptLanguagePreference preference) {
    final quality = candidateQualities[canonical];
    if (quality == null) return;
    // A broad high-quality range must not promote a candidate that has its
    // own more-specific, lower weight. Fallback cannot increase the request's
    // weight either. Ties preserve preference and exact/parent/sibling order.
    final score = quality < preference.quality ? quality : preference.quality;
    if (score > selectedQuality) {
      selectedQuality = score;
      selected = canonicalToOriginal[canonical];
    }
  }

  for (final preference in preferences) {
    if (preference.isWildcard || preference.isRejected) continue;
    final tag = LocaleTag.parse(preference.tag);
    final requestedScript = _matchingScript(tag);
    for (final candidate in localeFallbackChain(preference.tag)) {
      consider(candidate.toLowerCase(), preference);
    }
    for (final entry in tags.entries) {
      final supported = entry.value;
      if (supported.language == tag.language &&
          (requestedScript == null || scripts[entry.key] == null ||
              requestedScript == scripts[entry.key])) {
        consider(entry.key, preference);
      }
    }
  }

  if (selected != null) return selected;
  final defaultKey = LocaleTag.tryParse(defaultLocale)?.toCatalogTag().toLowerCase();
  return canonicalToOriginal[defaultKey] ?? canonicalToOriginal.values.first;
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

