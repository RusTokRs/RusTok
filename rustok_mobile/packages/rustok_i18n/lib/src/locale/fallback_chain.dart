import 'locale_tag.dart';
import 'likely_subtags.dart';

/// Generates a deduplicated, ordered list of candidate locale tags for resolution.
///
/// Order of precedence:
/// 1. Requested catalog locale from most-specific to least-specific.
/// 2. Default locale chain.
/// 3. Platform canonical fallback (`en`).
///
/// Invalid tags contribute no candidates; valid default/platform chains still
/// apply when [locale] is invalid.
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
/// Every known scriptless regional locale may gain script candidates from CLDR.
/// Exact catalog identities stay first; language-only locales do not gain an
/// invented region. Unknown locales retain structural fallback only. Explicit
/// `Zzzz`/`ZZ` catalog keys are tried first, then treated as absent for fallback.
/// A CLDR-equivalent regional spelling is tried at each variant level before
/// dropping variants: `en-Latn-GB` can find `en-GB`, but `az-Latn-IR` cannot
/// find the Arabic-script `az-IR` catalog through this equivalence.
List<String> localeFallbackChain(String locale) {
  final candidates = <String>[];
  final tag = LocaleTag.tryParse(locale);
  if (tag == null) return const [];

  // Unknown markers retain literal catalog priority but are absent for
  // inference. Never infer a region: catalog identity is not maximization.
  final script = tag.script == 'Zzzz' ? null : tag.script;
  final region = tag.region == 'ZZ' ? null : tag.region;
  final regionalScript = region == null ? null : maximizeLocale(
    LocaleTag(language: tag.language, region: region),
  )?.script;

  // Try equivalent regional spellings at each variant specificity before
  // discarding a variant. An explicit script can be omitted only if CLDR
  // confirms that the regional catalog uses that same script.
  for (var count = tag.variants.length; count >= 0; count--) {
    final variants = tag.variants.take(count).toList();
    _pushUnique(candidates, LocaleTag(
      language: tag.language, script: tag.script, region: tag.region,
      variants: variants,
    ).toCatalogTag());
    _pushUnique(candidates, LocaleTag(
      language: tag.language, script: script, region: region,
      variants: variants,
    ).toCatalogTag());
    if (region != null && regionalScript != null &&
        (script == null || script == regionalScript)) {
      _pushUnique(candidates, LocaleTag(
        language: tag.language,
        script: script == null ? regionalScript : null,
        region: region,
        variants: variants,
      ).toCatalogTag());
    }
  }

  // Strip region after all regional variants/equivalents. A language-only
  // request stays generic; no likely country is added to its candidate chain.
  final parentScript = script ?? regionalScript;
  if (parentScript != null) {
    _pushUnique(candidates, LocaleTag(
      language: tag.language, script: parentScript,
    ).toCatalogTag());
  }
  _pushUnique(candidates, LocaleTag(language: tag.language).toCatalogTag());

  return List.unmodifiable(candidates);
}

void _pushUnique(List<String> list, String value) {
  if (!list.contains(value)) {
    list.add(value);
  }
}
