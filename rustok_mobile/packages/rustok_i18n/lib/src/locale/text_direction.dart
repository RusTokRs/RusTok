/// Text directionality representation.
///
/// Named [LocaleTextDirection] (not `TextDirection`) so that importing this
/// package together with `package:flutter/widgets.dart` never produces an
/// ambiguous `TextDirection` name.
enum LocaleTextDirection {
  ltr,
  rtl;

  bool get isRtl => this == LocaleTextDirection.rtl;
  bool get isLtr => this == LocaleTextDirection.ltr;
}

/// Languages that are always written right-to-left.
const Set<String> _rtlLanguages = {
  'ar', // Arabic
  'arc', // Aramaic
  'azb', // South Azerbaijani (Perso-Arabic script)
  'ckb', // Sorani Kurdish
  'dv', // Divehi
  'fa', // Persian
  'glk', // Gilaki
  'he', // Hebrew
  'iw', // Hebrew (legacy)
  'ji', // Yiddish (legacy)
  'khw', // Khowar
  'lrc', // Northern Luri
  'mzn', // Mazanderani
  'pnb', // Western Punjabi
  'ps', // Pashto
  'ug', // Uyghur
  'ur', // Urdu
  'yi', // Yiddish
};

/// Languages whose default script is Arabic-based (RTL) per CLDR.
const Set<String> _arabicDefaultLanguages = {
  'ks', // Kashmiri
  'sd', // Sindhi
};

/// Languages that are RTL only when written in an RTL script (their default
/// script is Latin, e.g. Latin-script Kurmanji or Hausa).
const Set<String> _arabicScriptOptionalLanguages = {
  'ha', // Hausa
  'ku', // Kurdish (Kurmanji)
};

/// Known RTL script codes (ISO 15924).
const Set<String> _rtlScripts = {
  'Adlm',
  'Armi',
  'Avst',
  'Hebr',
  'Hung',
  'Khar',
  'Mand',
  'Mend',
  'Narb',
  'Nkoo',
  'Palm',
  'Phnx',
  'Prti',
  'Rohg',
  'Samr',
  'Sarb',
  'Syrc',
  'Thaa',
  'Yezi',
  'Arab',
};

String _normalizeScript(String script) {
  if (script.isEmpty) return script;
  return script.substring(0, 1).toUpperCase() + script.substring(1).toLowerCase();
}

/// Determines the text direction for a language and optional script.
///
/// An explicit script always decides the direction: `ku-Latn` is LTR and
/// `ku-Arab` is RTL. Without a script, the language's default script decides.
LocaleTextDirection detectTextDirection({required String language, String? script}) {
  if (script != null && script.isNotEmpty) {
    return _rtlScripts.contains(_normalizeScript(script))
        ? LocaleTextDirection.rtl
        : LocaleTextDirection.ltr;
  }

  final lang = language.trim().toLowerCase();
  if (_rtlLanguages.contains(lang) || _arabicDefaultLanguages.contains(lang)) {
    return LocaleTextDirection.rtl;
  }
  if (_arabicScriptOptionalLanguages.contains(lang)) {
    return LocaleTextDirection.ltr;
  }
  return LocaleTextDirection.ltr;
}
