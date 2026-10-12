import 'text_direction.dart';

/// Maximum allowable length for raw locale tag inputs (matches RusToK DoS boundary).
const int maxLocaleTagLength = 64;

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
      detectTextDirection(language: language, script: script);

  bool get isRtl => direction.isRtl;

  /// Parses a BCP-47 or POSIX locale string (e.g. `ru_RU.UTF-8`, `zh-Hant-TW`, `en-US`).
  ///
  /// Returns `null` if the input is empty or exceeds [maxLocaleTagLength].
  static LocaleTag? tryParse(String? raw) {
    if (raw == null || raw.length > maxLocaleTagLength) return null;
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

    final segments = trimmed.split('-');
    if (segments.isEmpty || segments.first.isEmpty) return null;

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
      if (seg.isEmpty) {
        index++;
        continue;
      }

      // Extension singleton (single alphanumeric char, e.g. 'u', 't', 'x')
      if (seg.length == 1) {
        final singleton = seg.toLowerCase();
        final extSubtags = <String>[];
        index++;
        while (index < segments.length && segments[index].length > 1) {
          extSubtags.add(segments[index].toLowerCase());
          index++;
        }
        extensions[singleton] = extSubtags.join('-');
        continue;
      }

      // Variant (4-8 alphanumeric chars)
      variants.add(seg.toLowerCase());
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
    for (final entry in extensions.entries) {
      buffer.write('-${entry.key}-${entry.value}');
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
          _listEquals(variants, other.variants);

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
