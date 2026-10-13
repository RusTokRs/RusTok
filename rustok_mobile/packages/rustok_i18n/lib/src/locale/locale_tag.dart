import 'dart:convert';

import 'text_direction.dart';
import 'likely_subtags.dart';

/// Maximum raw locale tag length in UTF-8 bytes, before trimming/normalization.
const int maxLocaleTagLength = 64;

/// BCP-47 variant subtag: 5-8 alphanumerics, or a digit followed by 3 alphanumerics.
final RegExp _variantPattern = RegExp(r'^[a-z0-9]{5,8}$');
final RegExp _digitVariantPattern = RegExp(r'^[0-9][a-z0-9]{3}$');

/// Grandfathered tags of RFC 5646 §2.2.8, mapped to the preferred value of the
/// IANA language subtag registry (lower case).
///
/// A `null` value marks one of the five tags without a preferred value: it is
/// kept as an opaque `language`, so a catalog can still be looked up by exactly
/// that string while the tag stays recognizable.
const Map<String, String?> _grandfatheredPreferred = <String, String?>{
  'art-lojban': 'jbo',
  'cel-gaulish': null,
  'en-gb-oed': 'en-GB-oxendict',
  'i-ami': 'ami',
  'i-bnn': 'bnn',
  'i-default': null,
  'i-enochian': null,
  'i-hak': 'hak',
  'i-klingon': 'tlh',
  'i-lux': 'lb',
  'i-mingo': null,
  'i-navajo': 'nv',
  'i-pwn': 'pwn',
  'i-tao': 'tao',
  'i-tay': 'tay',
  'i-tsu': 'tsu',
  'no-bok': 'nb',
  'no-nyn': 'nn',
  'sgn-be-fr': 'sfb',
  'sgn-be-nl': 'vgt',
  'sgn-ch-de': 'sgg',
  'zh-guoyu': 'cmn',
  'zh-hakka': 'hak',
  'zh-min': null,
  'zh-min-nan': 'nan',
  'zh-xiang': 'hsn',
};

/// Immutable representation of a parsed and canonicalized BCP-47 / Unicode locale identifier.
class LocaleTag {
  const LocaleTag({
    required this.language,
    this.script,
    this.region,
    this.variants = const <String>[],
    this.extensions = const <String, String>{},
  });

  final String language;
  final String? script;
  final String? region;
  final List<String> variants;
  final Map<String, String> extensions;

  /// Returns text directionality (LTR or RTL) for this locale.
  LocaleTextDirection get direction =>
      detectTextDirection(language: language,
          script: script == null || script == 'Zzzz'
              ? maximizeLocale(this)?.script : script);

  bool get isRtl => direction.isRtl;

  /// Parses a BCP-47 or POSIX locale string (e.g. `ru_RU.UTF-8`, `zh-Hant-TW`, `en-US`).
  ///
  /// Returns `null` if the input is empty, exceeds [maxLocaleTagLength], or is
  /// malformed (invalid language, script, region or variant subtag).
  static LocaleTag? tryParse(String? raw) {
    if (raw == null ||
        raw.length > maxLocaleTagLength ||
        utf8.encode(raw).length > maxLocaleTagLength) return null;
    var trimmed = raw.trim();
    if (trimmed.isEmpty) return null;

    // Remove encoding or modifier suffix if present (e.g. .UTF-8 or @euro)
    final dotIndex = trimmed.indexOf('.');
    if (dotIndex != -1) {
      trimmed = trimmed.substring(0, dotIndex);
    }
    final atIndex = trimmed.indexOf('@');
    if (atIndex != -1) {
      trimmed = trimmed.substring(0, atIndex);
    }

    // Normalize underscores to hyphens
    trimmed = trimmed.replaceAll('_', '-');

    // Grandfathered tags have no subtag structure (`i-klingon`, `zh-min-nan`,
    // `en-GB-oed`, `sgn-BE-FR`), so they are resolved before the tag is split.
    final lower = trimmed.toLowerCase();
    if (_grandfatheredPreferred.containsKey(lower)) {
      final preferred = _grandfatheredPreferred[lower];
      return preferred == null
          ? LocaleTag(language: lower)
          : tryParse(preferred);
    }

    final segments = trimmed.split('-');
    if (segments.any((segment) =>
        !RegExp(r'^[a-zA-Z0-9]{1,8}$').hasMatch(segment))) {
      return null;
    }

    var lang = segments.first.toLowerCase();
    // Normalize legacy Android/Java ISO 639-1 codes to canonical BCP-47
    const legacyAliases = {
      'in': 'id', // Indonesian
      'iw': 'he', // Hebrew
      'ji': 'yi', // Yiddish
    };
    lang = legacyAliases[lang] ?? lang;

    // Valid BCP-47 primary language subtag is 2-8 letters (typically 2-3)
    if (!RegExp(r'^[a-z]{2,8}$').hasMatch(lang)) return null;

    String? script;
    String? region;
    final variants = <String>[];
    final extensions = <String, String>{};

    int index = 1;

    // Check for 4-letter script subtag
    if (index < segments.length &&
        RegExp(r'^[a-zA-Z]{4}$').hasMatch(segments[index])) {
      final s = segments[index];
      script = s.substring(0, 1).toUpperCase() + s.substring(1).toLowerCase();
      index++;
    }

    // Check for 2-letter or 3-digit region subtag
    if (index < segments.length &&
        (RegExp(r'^[a-zA-Z]{2}$').hasMatch(segments[index]) ||
            RegExp(r'^[0-9]{3}$').hasMatch(segments[index]))) {
      region = segments[index].toUpperCase();
      index++;
    }

    // Process remaining segments (variants and extension singletons)
    while (index < segments.length) {
      final seg = segments[index];
      // Private use consumes the entire remainder, including one-character
      // subtags. Other extensions require 2-8 character subtags and unique keys.
      if (seg.length == 1) {
        final singleton = seg.toLowerCase();
        if (extensions.containsKey(singleton)) return null;
        final extSubtags = <String>[];
        index++;
        while (index < segments.length &&
            (singleton == 'x' || segments[index].length > 1)) {
          extSubtags.add(segments[index].toLowerCase());
          index++;
        }
        if (extSubtags.isEmpty) return null;
        extensions[singleton] = extSubtags.join('-');
        continue;
      }

      // Variant: reject malformed subtags instead of silently accepting them.
      final lower = seg.toLowerCase();
      if (!_variantPattern.hasMatch(lower) && !_digitVariantPattern.hasMatch(lower)) {
        return null;
      }
      if (variants.contains(lower)) return null;
      variants.add(lower);
      index++;
    }

    return LocaleTag(
      language: lang,
      script: script,
      region: region,
      variants: List.unmodifiable(variants),
      extensions: Map.unmodifiable(extensions),
    );
  }

  /// Parses a BCP-47 tag or throws [FormatException] on invalid input.
  static LocaleTag parse(String raw) {
    final result = tryParse(raw);
    if (result == null) {
      throw FormatException('Invalid or oversized locale identifier: "$raw"');
    }
    return result;
  }

  /// Returns canonical BCP-47 string representation (e.g. `zh-Hant-TW`, `en-US`, `ru`).
  String toLanguageTag() {
    final buffer = StringBuffer(language);
    if (script != null) buffer.write('-$script');
    if (region != null) buffer.write('-$region');
    for (final variant in variants) {
      buffer.write('-$variant');
    }
    // Extension order has no semantic significance. Serialize singleton blocks
    // deterministically, with private use last so it cannot swallow extensions.
    final singletons = extensions.keys.toList()
      ..sort((a, b) {
        if (a == b) return 0;
        if (a == 'x') return 1;
        if (b == 'x') return -1;
        return a.compareTo(b);
      });
    for (final singleton in singletons) {
      buffer.write('-$singleton-${extensions[singleton]}');
    }
    return buffer.toString();
  }

  /// Returns extension-free base language identifier for catalog lookup (e.g. `zh-Hant-TW`).
  String toCatalogTag() {
    final buffer = StringBuffer(language);
    if (script != null) buffer.write('-$script');
    if (region != null) buffer.write('-$region');
    for (final variant in variants) {
      buffer.write('-$variant');
    }
    return buffer.toString();
  }

  @override
  String toString() => toLanguageTag();

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is LocaleTag &&
          runtimeType == other.runtimeType &&
          language == other.language &&
          script == other.script &&
          region == other.region &&
          _listEquals(variants, other.variants) &&
          _mapEquals(extensions, other.extensions);

  // Extensions are intentionally excluded from the hash: equal objects still
  // hash equally, and the hash stays order-independent.
  @override
  int get hashCode => Object.hash(
        language,
        script,
        region,
        Object.hashAll(variants),
      );

  static bool _listEquals(List<String> a, List<String> b) {
    if (a.length != b.length) return false;
    for (int i = 0; i < a.length; i++) {
      if (a[i] != b[i]) return false;
    }
    return true;
  }

  static bool _mapEquals(Map<String, String> a, Map<String, String> b) {
    if (a.length != b.length) return false;
    for (final entry in a.entries) {
      if (b[entry.key] != entry.value) return false;
    }
    return true;
  }
}

/// Normalizes an admin UI effective locale to either 'ru' or 'en'.
///
/// If [locale] is absent, invalid, or not Russian, defaults to `'en'`.
/// Matches the exact contract of `rustok-ui-i18n::normalize_admin_locale`.
String normalizeAdminLocale(String? locale) {
  final tag = LocaleTag.tryParse(locale);
  if (tag == null) return 'en';
  return tag.language.toLowerCase() == 'ru' ? 'ru' : 'en';
}
