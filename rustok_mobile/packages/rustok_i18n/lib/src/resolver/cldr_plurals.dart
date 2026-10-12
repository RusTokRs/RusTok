/// Resolves the CLDR plural category (`zero`, `one`, `two`, `few`, `many`, `other`)
/// for a given number and language code.
///
/// Covers the languages used by RusToK and the common Slavic, Baltic, Romance,
/// Germanic and CJK families. Other languages use the English-style rule
/// (`one` for exactly 1, `other` otherwise).
///
/// Limitation: fractional values (e.g. `1.5`) are categorized as `other` for
/// most languages; CLDR's visible-fraction-digit rules (`v != 0`) are not modelled.
String resolveCldrPluralCategory(String language, num value) {
  // NaN and infinities have no plural category; never call toInt() on them.
  if (!value.isFinite) return 'other';

  final normalized = language.trim().toLowerCase().replaceAll('_', '-');
  final lang = normalized.split('-').first;
  final n = value.abs();
  final isInteger = n == n.truncateToDouble();

  if (!isInteger) {
    // French and Brazilian Portuguese use only the integer part: 0 and 1 are 'one'.
    if (normalized == 'pt-br' || lang == 'fr') {
      final integerPart = n.truncateToDouble();
      return (integerPart == 0 || integerPart == 1) ? 'one' : 'other';
    }
    return 'other';
  }

  // Integer residues. Doubles are used for n so very large values stay exact.
  final int mod10 = (n % 10).toInt();
  final int mod100 = (n % 100).toInt();

  if (normalized == 'pt-br') {
    return (n == 0 || n == 1) ? 'one' : 'other';
  }

  switch (lang) {
    // East Slavic: Russian, Ukrainian, Belarusian
    case 'ru':
    case 'uk':
    case 'be':
      if (mod10 == 1 && mod100 != 11) return 'one';
      if (mod10 >= 2 && mod10 <= 4 && !(mod100 >= 12 && mod100 <= 14)) return 'few';
      return 'many';

    // South Slavic: Serbian, Croatian, Bosnian
    case 'sr':
    case 'hr':
    case 'bs':
      if (mod10 == 1 && mod100 != 11) return 'one';
      if (mod10 >= 2 && mod10 <= 4 && !(mod100 >= 12 && mod100 <= 14)) return 'few';
      return 'other';

    // West Slavic: Czech, Slovak
    case 'cs':
    case 'sk':
      if (n == 1) return 'one';
      if (n >= 2 && n <= 4) return 'few';
      return 'other';

    // Polish
    case 'pl':
      if (n == 1) return 'one';
      if (mod10 >= 2 && mod10 <= 4 && !(mod100 >= 12 && mod100 <= 14)) return 'few';
      return 'many';

    // Slovenian
    case 'sl':
      if (mod100 == 1) return 'one';
      if (mod100 == 2) return 'two';
      if (mod100 == 3 || mod100 == 4) return 'few';
      return 'other';

    // Lithuanian
    case 'lt':
      if (mod10 == 1 && !(mod100 >= 11 && mod100 <= 19)) return 'one';
      if (mod10 >= 2 && mod10 <= 9 && !(mod100 >= 11 && mod100 <= 19)) return 'few';
      return 'other';

    // Latvian
    case 'lv':
      if (mod10 == 0 || (mod100 >= 11 && mod100 <= 19)) return 'zero';
      if (mod10 == 1 && mod100 != 11) return 'one';
      return 'other';

    // Romanian
    case 'ro':
      if (n == 1) return 'one';
      if (n == 0 || (mod100 >= 2 && mod100 <= 19)) return 'few';
      return 'other';

    // French (0 and 1 are 'one')
    case 'fr':
      return (n == 0 || n == 1) ? 'one' : 'other';

    // Portuguese (generic)
    case 'pt':
      return (n == 0 || n == 1) ? 'one' : 'other';

    // Arabic (zero, one, two, few, many, other)
    case 'ar':
      if (n == 0) return 'zero';
      if (n == 1) return 'one';
      if (n == 2) return 'two';
      if (mod100 >= 3 && mod100 <= 10) return 'few';
      if (mod100 >= 11 && mod100 <= 99) return 'many';
      return 'other';

    // East Asian and Southeast Asian languages: no plural variation
    case 'zh':
    case 'ja':
    case 'ko':
    case 'vi':
    case 'th':
      return 'other';

    // Germanic / Romance baseline (en, de, es, it, nl, etc.)
    default:
      return n == 1 ? 'one' : 'other';
  }
}
