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
/// 2. If a request has a region but no script, likely-script branch is added before base language.
/// 3. Default locale fallback chain.
/// 4. Platform canonical fallback (`en`).
List<String> localeCandidates({
  String? locale,
  String defaultLocale = 'en',
  String platformFallback = 'en',
}) {
  final candidates = <String>[];

  void addChain(String? raw) {
    if (raw == null) return;
    final tag = LocaleTag.tryParse(raw);
    if (tag == null) return;

    _pushUnique(candidates, tag.toCatalogTag());

    // 1. Remove variants
    if (tag.variants.isNotEmpty) {
      final noVariants = LocaleTag(
        language: tag.language,
        script: tag.script,
        region: tag.region,
      );
      _pushUnique(candidates, noVariants.toCatalogTag());
    }

    // 2. Regional likely script expansion (e.g. zh-TW -> zh-Hant-TW -> zh-Hant)
    if (tag.script == null && tag.region != null) {
      final key = '${tag.language}-${tag.region}'.toLowerCase();
      final likelyScript = _regionalLikelyScripts[key];
      if (likelyScript != null) {
        final withScript = LocaleTag(
          language: tag.language,
          script: likelyScript,
          region: tag.region,
        );
        _pushUnique(candidates, withScript.toCatalogTag());

        final scriptOnly = LocaleTag(
          language: tag.language,
          script: likelyScript,
        );
        _pushUnique(candidates, scriptOnly.toCatalogTag());
      }
    }

    // 3. Strip region
    if (tag.region != null) {
      final noRegion = LocaleTag(
        language: tag.language,
        script: tag.script,
      );
      _pushUnique(candidates, noRegion.toCatalogTag());
    }

    // 4. Strip script -> base language
    if (tag.script != null) {
      final baseLang = LocaleTag(language: tag.language);
      _pushUnique(candidates, baseLang.toCatalogTag());
    }
  }

  addChain(locale);
  addChain(defaultLocale);
  addChain(platformFallback);

  return List.unmodifiable(candidates);
}

/// Generates the fallback chain for a single locale without default/platform fallbacks.
List<String> localeFallbackChain(String locale) {
  final candidates = <String>[];
  final tag = LocaleTag.tryParse(locale);
  if (tag == null) return const [];

  _pushUnique(candidates, tag.toCatalogTag());

  if (tag.variants.isNotEmpty) {
    _pushUnique(
      candidates,
      LocaleTag(language: tag.language, script: tag.script, region: tag.region)
          .toCatalogTag(),
    );
  }

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

  if (tag.region != null) {
    _pushUnique(
      candidates,
      LocaleTag(language: tag.language, script: tag.script).toCatalogTag(),
    );
  }

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
