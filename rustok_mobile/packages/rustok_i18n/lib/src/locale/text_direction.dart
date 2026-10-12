/// Text directionality representation.
///
/// Renamed to [LocaleTextDirection] to prevent ambiguous name collisions
/// with Flutter framework's `dart:ui.TextDirection`.
enum LocaleTextDirection {
  ltr,
  rtl;

  bool get isRtl => this == LocaleTextDirection.rtl;
  bool get isLtr => this == LocaleTextDirection.ltr;
}

/// Backwards-compatible alias for non-Flutter environments.
typedef TextDirection = LocaleTextDirection;

/// Known RTL language subtag prefixes per CLDR and BCP-47.
const Set<String> _rtlLanguages = {
  'ar', // Arabic
  'arc', // Aramaic
  'ckb', // Sorani Kurdish
  'dv', // Divehi
  'fa', // Persian
  'ha', // Hausa (when in Arabic script)
  'he', // Hebrew
  'iw', // Hebrew (legacy)
  'ji', // Yiddish (legacy)
  'khw', // Khowar
  'ks', // Kashmiri
  'ku', // Kurdish
  'mzn', // Mazanderani
  'pnb', // Western Punjabi
  'ps', // Pashto
  'sd', // Sindhi
  'ug', // Uyghur
  'ur', // Urdu
  'yi', // Yiddish
};

/// Known RTL script codes (ISO 15924).
const Set<String> _rtlScripts = {
  'Arab',
  'Armi',
  'Hebr',
  'Mand',
  'Mend',
  'Nkoo',
  'Samr',
  'Syrc',
  'Thaa',
};

/// Determines the text direction for a language or script tag.
LocaleTextDirection detectTextDirection({required String language, String? script}) {
  final langNormalized = language.trim().toLowerCase();
  if (script != null && script.isNotEmpty) {
    final scriptNormalized =
        script.substring(0, 1).toUpperCase() + script.substring(1).toLowerCase();
    if (_rtlScripts.contains(scriptNormalized)) {
      return LocaleTextDirection.rtl;
    }
  }
  if (_rtlLanguages.contains(langNormalized)) {
    return LocaleTextDirection.rtl;
  }
  return LocaleTextDirection.ltr;
}
