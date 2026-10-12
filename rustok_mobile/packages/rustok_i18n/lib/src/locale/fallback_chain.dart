import 'locale_tag.dart';

/// Common regional script mappings where region implies likely script.
const Map<String, String> _regionalLikelyScripts = {
  'zh-tw': 'Hant',
  'zh-hk': 'Hant',
  'zh-mo': 'Hant',
  'zh-cn': 'Hans',
  'zh-sg': 'Hans',
  'sr-rs': 'Cyrl',
  'sr-me': 'Cyrl',
  'sr-ba': 'Cyrl',
  'az-az': 'Latn',
  'uz-uz': 'Latn',
};

/// Generates a deduplicated, ordered list of candidate locale tags for resolution.
///
/// Order of precedence:
/// 1. Requested catalog locale from most-specific to least-specific.
/// 2. Default locale chain.
/// 3. Platform canonical fallback (`en`).
///
/// Returns an empty list when [locale] is not a valid locale tag.
List<String> localeCandidates({
  String? locale,
  String defaultLocale = 'en',
  String platformFallback = 'en',
}) {
  final candidates = <String>[];
  if (locale != null) {
    for (final candidate in localeFallbackChain(locale)) {
      _pushUnique(candidates, candidate);
    }
  }
  for (final candidate in localeFallbackChain(defaultLocale)) {
    _pushUnique(candidates, candidate);
  }
  for (final candidate in localeFallbackChain(platformFallback)) {
    _pushUnique(candidates, candidate);
  }
  return List.unmodifiable(candidates);
}

/// Generates the fallback chain for a single locale without default/platform fallbacks.
///
/// For `zh-TW` this yields `zh-TW`, `zh-Hant-TW`, `zh-Hant`, `zh`.
List<String> localeFallbackChain(String locale) {
  final candidates = <String>[];
  final tag = LocaleTag.tryParse(locale);
  if (tag == null) return const [];

  // 1. Full tag, including variants.
  _pushUnique(candidates, tag.toCatalogTag());

  // 2. Drop variants.
  if (tag.variants.isNotEmpty) {
    _pushUnique(
      candidates,
      LocaleTag(language: tag.language, script: tag.script, region: tag.region)
          .toCatalogTag(),
    );
  }

  // 3. Regional likely-script expansion (e.g. zh-TW -> zh-Hant-TW -> zh-Hant).
  if (tag.script == null && tag.region != null) {
    final key = '${tag.language}-${tag.region}'.toLowerCase();
    final likelyScript = _regionalLikelyScripts[key];
    if (likelyScript != null) {
      _pushUnique(
        candidates,
        LocaleTag(language: tag.language, script: likelyScript, region: tag.region)
            .toCatalogTag(),
      );
      _pushUnique(
        candidates,
        LocaleTag(language: tag.language, script: likelyScript).toCatalogTag(),
      );
    }
  }

  // 4. Strip region.
  if (tag.region != null) {
    _pushUnique(
      candidates,
      LocaleTag(language: tag.language, script: tag.script).toCatalogTag(),
    );
  }

  // 5. Strip script -> base language.
  if (tag.script != null) {
    _pushUnique(candidates, LocaleTag(language: tag.language).toCatalogTag());
  }

  return List.unmodifiable(candidates);
}

void _pushUnique(List<String> list, String value) {
  if (!list.contains(value)) {
    list.add(value);
  }
}
