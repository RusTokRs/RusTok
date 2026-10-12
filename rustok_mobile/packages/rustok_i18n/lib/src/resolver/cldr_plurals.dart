/// Resolves the CLDR plural category (`zero`, `one`, `two`, `few`, `many`, `other`)
/// for a given number and language code.
String resolveCldrPluralCategory(String language, num value) {
  final normalized = language.toLowerCase().replaceAll('_', '-');
  final lang = normalized.split('-').first;
  final n = value.abs();
  final i = n.toInt();
  final isInteger = (n == i);

  // In CLDR, fractional numbers (e.g. 1.5, 0.5) fall into 'other' for most languages
  if (!isInteger) {
    return 'other';
  }

  // Brazilian Portuguese: 0 and 1 are 'one'
  if (normalized == 'pt-br') {
    if (i == 0 || i == 1) return 'one';
    return 'other';
  }

  switch (lang) {
    // Slavic languages: Russian, Ukrainian, Belarusian
    case 'ru':
    case 'uk':
    case 'be':
      final mod10 = i % 10;
      final mod100 = i % 100;
      if (mod10 == 1 && mod100 != 11) {
        return 'one';
      }
      if (mod10 >= 2 && mod10 <= 4 && (mod100 < 10 || mod100 >= 20)) {
        return 'few';
      }
      if (mod10 == 0 || (mod10 >= 5 && mod10 <= 9) || (mod100 >= 11 && mod100 <= 19)) {
        return 'many';
      }
      return 'other';

    // Polish
    case 'pl':
      if (i == 1) return 'one';
      final mod10 = i % 10;
      final mod100 = i % 100;
      if (mod10 >= 2 && mod10 <= 4 && (mod100 < 10 || mod100 >= 20)) {
        return 'few';
      }
      return 'many';

    // French (0 and 1 are 'one')
    case 'fr':
      if (i == 0 || i == 1) return 'one';
      return 'other';

    // Arabic (zero, one, two, few, many, other)
    case 'ar':
      if (i == 0) return 'zero';
      if (i == 1) return 'one';
      if (i == 2) return 'two';
      final mod100 = i % 100;
      if (mod100 >= 3 && mod100 <= 10) return 'few';
      if (mod100 >= 11 && mod100 <= 99) return 'many';
      return 'other';

    // East Asian languages: no plural variations (always 'other')
    case 'zh':
    case 'ja':
    case 'ko':
    case 'vi':
    case 'th':
      return 'other';

    // Germanic / Romance baseline (en, de, es, it, nl, pt, etc.)
    default:
      if (i == 1) return 'one';
      return 'other';
  }
}

