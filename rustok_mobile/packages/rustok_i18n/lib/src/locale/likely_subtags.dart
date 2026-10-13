import 'locale_tag.dart';

part 'likely_subtags_data.dart';

/// Adds missing language/script/region using all CLDR 48.2 likely subtags.
///
/// Input must be canonical (as produced by [LocaleTag.tryParse]); this operation
/// does not implement additional CLDR alias rewriting. Follows UTS #35 release
/// 48 Add Likely Subtags: L-S-R, L-S, L-R, L lookup, in that order. `und`, `Zzzz`
/// and `ZZ` are unknown fields. Explicit known fields, variants and extensions
/// are retained. No unrelated `und`/English guess is made for unknown languages.
/// Returns null if no mapping exists, not an invented locale.
///
/// This is inference, not catalog canonicalization or language-distance matching.
LocaleTag? maximizeLocale(LocaleTag locale) {
  final language = locale.language;
  final script = locale.script == 'Zzzz' ? null : locale.script;
  final region = locale.region == 'ZZ' ? null : locale.region;

  LocaleTag result(String l, String s, String r) => LocaleTag(
    language: l,
    script: s,
    region: r,
    variants: List.unmodifiable(locale.variants),
    extensions: Map.unmodifiable(locale.extensions),
  );

  if (language != 'und' && script != null && region != null) {
    return result(language, script, region);
  }
  final keys = <String>{
    if (script != null && region != null) '$language-$script-$region',
    if (script != null) '$language-$script',
    if (region != null) '$language-$region',
    language,
  };
  for (final key in keys) {
    final match = _likelySubtags[key];
    if (match == null) continue;
    final parts = match.split('-');
    return result(language == 'und' ? parts[0] : language,
        script ?? parts[1], region ?? parts[2]);
  }
  return null;
}
