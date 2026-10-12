/// Locale-specific number data used by `NUMBER()` and [formatFluentNumber].
///
/// The table is generated from CLDR (currently 48.2) and covers every locale
/// whose numbers differ from English: English itself, 175 languages and every
/// regional override CLDR ships (`en-DE`, `ar-EG`, `es-MX`, `hi-IN`, ...).
/// `rustok_mobile/tooling/scripts/gen_number_symbols.py` regenerates this file
/// from `cldr-numbers-full` and `cldr-core`; the package itself depends on
/// neither `intl` nor any other data package.
///
/// Keys are BCP-47 tags. [numberSymbolsFor] tries the full tag, then the
/// language of the tag, then falls back to `en` (`1,234.5`), so a locale that
/// CLDR does not differentiate (`de-LI` while CLDR keeps `de-LI` equal to
/// `de`) inherits its language's data.
///
/// Verified against `Intl.NumberFormat` for every locale CLDR ships: 10 724
/// comparisons across 766 tags with the only remaining differences coming from
/// the ICU build of the runtime, which lags behind CLDR 48 in a few locales
/// (`fr-CH`, whose grouping separator became the Swiss apostrophe in CLDR 46).
class NumberSymbols {
  const NumberSymbols({
    required this.decimal,
    required this.group,
    this.digits = '',
    this.minimumGroupingDigits = 1,
    this.primaryGroupingSize = 3,
    this.secondaryGroupingSize = 3,
    this.minimumFractionDigits = 0,
    this.maximumFractionDigits = 3,
    this.percentPattern = '#,##0%',
    this.percentSign = '%',
    this.minusSign = '-',
    this.currencyPattern = '\u00a4#,##0.00',
    this.infinity = '\u221E',
    this.nan = 'NaN',
  });

  /// Decimal separator, e.g. `,` in `1,5`.
  final String decimal;

  /// Digit-group separator, e.g. `.` in `1.234`.
  final String group;

  /// The ten digits of the locale's numbering system in `0`-`9` order, or an
  /// empty string when the locale uses ASCII digits.
  final String digits;

  /// Smallest number of digits that is grouped: 2 in Polish, Spanish, Italian
  /// and Czech, so `1234` stays `1234` while `12345` becomes `12 345`.
  final int minimumGroupingDigits;

  /// Digits between group separators, counted from the decimal separator:
  /// 3 nearly everywhere, 3 with a secondary size of 2 in the Indian
  /// numbering system (`12,34,567`), and 2 with a secondary size of 2 in
  /// Toki Pona.
  final int primaryGroupingSize;
  final int secondaryGroupingSize;

  /// Fraction digits of the locale's decimal pattern, which are the defaults of
  /// `NUMBER()` when the message does not name them.
  final int minimumFractionDigits;
  final int maximumFractionDigits;

  /// Percent layout from CLDR: `#,##0%`, `#,##0 %` (French, Russian) or
  /// `%#,##0` (Turkish). It holds the position of the sign and the default
  /// fraction digits of the percent style.
  final String percentPattern;

  /// Percent sign, `%` or `\u066A` in Arabic.
  final String percentSign;

  /// Minus sign: `-`, `\u2212` (the true minus of Estonian, Basque or
  /// Persian) or `\u061C-` in Arabic. It is used for every negative value and
  /// replaces the `-` written in the CLDR patterns.
  final String minusSign;

  /// Currency layout from CLDR, where `\u00a4` marks the currency:
  /// `\u00a4#,##0.00` is `$1,234.50`, `#,##0.00\u00a0\u00a4` is
  /// `1\u00a0234,50\u00a0\u20ac`, and a `;`-separated negative subpattern is
  /// used as written, which is what makes `1.234,50-` (Faroese) possible.
  final String currencyPattern;

  /// Symbols for infinite values and for NaN.
  final String infinity;
  final String nan;
}

const NumberSymbols _english = NumberSymbols(decimal: '.', group: ',');

/// Number data for a BCP-47 [locale], English when nothing matches.
NumberSymbols numberSymbolsFor(String locale) {
  final tag = _normalizeTag(locale);
  final parts = tag.split('-');
  final language = parts.first;
  String? script;
  for (final part in parts.skip(1)) {
    if (part.length == 4 && _isAsciiLetters(part)) {
      script = part;
      break;
    }
  }
  return _numberSymbols[tag] ??
      (script == null ? null : _numberSymbols['$language-$script']) ??
      _numberSymbols[language] ??
      _english;
}

/// Lower-cases the language, title-cases a script and upper-cases a region, so
/// that `es-mx` and `ES-MX` find the same entry.
String _normalizeTag(String locale) {
  final parts = locale.replaceAll('_', '-').split('-');
  final result = <String>[parts.first.toLowerCase()];
  for (final part in parts.skip(1)) {
    if (part.length == 4 && _isAsciiLetters(part)) {
      result.add(part[0].toUpperCase() + part.substring(1).toLowerCase());
    } else if (part.length == 2 && _isAsciiLetters(part)) {
      result.add(part.toUpperCase());
    } else {
      result.add(part);
    }
  }
  return result.join('-');
}

bool _isAsciiLetters(String value) {
  for (final unit in value.codeUnits) {
    final isUpper = unit >= 0x41 && unit <= 0x5A;
    final isLower = unit >= 0x61 && unit <= 0x7A;
    if (!isUpper && !isLower) return false;
  }
  return true;
}

/// Inserts the group separator into the ASCII [digits].
///
/// Supports the Indian numbering system (`12,34,567`) and
/// [NumberSymbols.minimumGroupingDigits], which suppresses the separator for
/// short numbers (`1234` in Polish).
String groupDigits(String digits, NumberSymbols symbols) {
  if (digits.length < 3 + symbols.minimumGroupingDigits) return digits;
  final chunks = <String>[];
  var end = digits.length;
  var size = symbols.primaryGroupingSize;
  while (end > 0) {
    final start = end - size < 0 ? 0 : end - size;
    chunks.insert(0, digits.substring(start, end));
    end = start;
    size = symbols.secondaryGroupingSize;
  }
  return chunks.join(symbols.group);
}

/// Rewrites ASCII digits in [text] with the locale's numbering system digits.
String localizeDigits(String text, NumberSymbols symbols) {
  if (symbols.digits.isEmpty) return text;
  final buffer = StringBuffer();
  for (final unit in text.codeUnits) {
    if (unit >= 0x30 && unit <= 0x39) {
      buffer.write(symbols.digits[unit - 0x30]);
    } else {
      buffer.writeCharCode(unit);
    }
  }
  return buffer.toString();
}

/// Fraction digits a number pattern asks for: `#,##0.###` is `(0, 3)`,
/// `#,##0.00` is `(2, 2)` and `#,##0%` is `(0, 0)`.
(int, int) patternFractionDigits(String pattern) {
  final positive = pattern.split(';').first;
  final dot = positive.indexOf('.');
  if (dot < 0) return (0, 0);
  var minimum = 0;
  var maximum = 0;
  for (var i = dot + 1; i < positive.length; i++) {
    final unit = positive.codeUnitAt(i);
    if (unit == 0x30) {
      minimum++;
      maximum++;
    } else if (unit == 0x23) {
      maximum++;
    } else {
      break;
    }
  }
  return (minimum, maximum);
}

/// Locale data for every locale whose numbers differ from English.
const Map<String, NumberSymbols> _numberSymbols = {
  'aa': NumberSymbols(decimal: '.', group: ',', currencyPattern: '¤\u00A0#,##0.00'),
  'ab': NumberSymbols(decimal: '.', group: ',', currencyPattern: '¤\u00A0#,##0.00'),
  'af': NumberSymbols(decimal: ',', group: '\u00A0'),
  'agq': NumberSymbols(decimal: ',', group: '\u00A0', currencyPattern: '#,##0.00¤'),
  'am': NumberSymbols(decimal: '.', group: ',', nan: 'በቁጥር\u00A0ሊገለጽ\u00A0የማይችል'),
  'an': NumberSymbols(decimal: '.', group: ',', currencyPattern: '¤\u00A0#,##0.00'),
  'ann': NumberSymbols(decimal: '.', group: ',', currencyPattern: '¤\u00A0#,##0.00'),
  'apc': NumberSymbols(decimal: '.', group: ',', currencyPattern: '¤\u00A0#,##0.00'),
  'ar': NumberSymbols(
    decimal: '.',
    group: ',',
    percentSign: '\u200E%\u200E',
    minusSign: '\u200E-',
    currencyPattern: '\u200F#,##0.00\u00A0¤;\u200F-#,##0.00\u00A0¤',
    nan: 'ليس\u00A0رقمًا',
  ),
  'ar-BH': NumberSymbols(
    decimal: '٫',
    group: '٬',
    digits: '٠١٢٣٤٥٦٧٨٩',
    percentSign: '٪\u061C',
    minusSign: '\u061C-',
    currencyPattern: '\u200F#,##0.00\u00A0¤',
    nan: 'ليس\u00A0رقمًا',
  ),
  'ar-DJ': NumberSymbols(
    decimal: '٫',
    group: '٬',
    digits: '٠١٢٣٤٥٦٧٨٩',
    percentSign: '٪\u061C',
    minusSign: '\u061C-',
    currencyPattern: '\u200F#,##0.00\u00A0¤',
    nan: 'ليس\u00A0رقمًا',
  ),
  'ar-DZ': NumberSymbols(
    decimal: ',',
    group: '.',
    percentSign: '\u200E%\u200E',
    minusSign: '\u200E-',
    currencyPattern: '\u200F#,##0.00\u00A0¤;\u200F-#,##0.00\u00A0¤',
    nan: 'ليس\u00A0رقمًا',
  ),
  'ar-EG': NumberSymbols(
    decimal: '٫',
    group: '٬',
    digits: '٠١٢٣٤٥٦٧٨٩',
    percentSign: '٪\u061C',
    minusSign: '\u061C-',
    currencyPattern: '\u200F#,##0.00\u00A0¤',
    nan: 'ليس\u00A0رقمًا',
  ),
  'ar-ER': NumberSymbols(
    decimal: '٫',
    group: '٬',
    digits: '٠١٢٣٤٥٦٧٨٩',
    percentSign: '٪\u061C',
    minusSign: '\u061C-',
    currencyPattern: '\u200F#,##0.00\u00A0¤',
    nan: 'ليس\u00A0رقمًا',
  ),
  'ar-IL': NumberSymbols(
    decimal: '٫',
    group: '٬',
    digits: '٠١٢٣٤٥٦٧٨٩',
    percentSign: '٪\u061C',
    minusSign: '\u061C-',
    currencyPattern: '\u200F#,##0.00\u00A0¤',
    nan: 'ليس\u00A0رقمًا',
  ),
  'ar-IQ': NumberSymbols(
    decimal: '٫',
    group: '٬',
    digits: '٠١٢٣٤٥٦٧٨٩',
    percentSign: '٪\u061C',
    minusSign: '\u061C-',
    currencyPattern: '\u200F#,##0.00\u00A0¤',
    nan: 'ليس\u00A0رقمًا',
  ),
  'ar-JO': NumberSymbols(
    decimal: '٫',
    group: '٬',
    digits: '٠١٢٣٤٥٦٧٨٩',
    percentSign: '٪\u061C',
    minusSign: '\u061C-',
    currencyPattern: '\u200F#,##0.00\u00A0¤',
    nan: 'ليس\u00A0رقمًا',
  ),
  'ar-KM': NumberSymbols(
    decimal: '٫',
    group: '٬',
    digits: '٠١٢٣٤٥٦٧٨٩',
    percentSign: '٪\u061C',
    minusSign: '\u061C-',
    currencyPattern: '\u200F#,##0.00\u00A0¤',
    nan: 'ليس\u00A0رقمًا',
  ),
  'ar-KW': NumberSymbols(
    decimal: '٫',
    group: '٬',
    digits: '٠١٢٣٤٥٦٧٨٩',
    percentSign: '٪\u061C',
    minusSign: '\u061C-',
    currencyPattern: '\u200F#,##0.00\u00A0¤',
    nan: 'ليس\u00A0رقمًا',
  ),
  'ar-LB': NumberSymbols(
    decimal: '٫',
    group: '٬',
    digits: '٠١٢٣٤٥٦٧٨٩',
    percentSign: '٪\u061C',
    minusSign: '\u061C-',
    currencyPattern: '\u200F#,##0.00\u00A0¤',
    nan: 'ليس\u00A0رقمًا',
  ),
  'ar-LY': NumberSymbols(
    decimal: ',',
    group: '.',
    percentSign: '\u200E%\u200E',
    minusSign: '\u200E-',
    currencyPattern: '\u200F#,##0.00\u00A0¤;\u200F-#,##0.00\u00A0¤',
    nan: 'ليس\u00A0رقمًا',
  ),
  'ar-MA': NumberSymbols(
    decimal: ',',
    group: '.',
    percentSign: '\u200E%\u200E',
    minusSign: '\u200E-',
    currencyPattern: '\u200F#,##0.00\u00A0¤;\u200F-#,##0.00\u00A0¤',
    nan: 'ليس\u00A0رقمًا',
  ),
  'ar-MR': NumberSymbols(
    decimal: '٫',
    group: '٬',
    digits: '٠١٢٣٤٥٦٧٨٩',
    percentSign: '٪\u061C',
    minusSign: '\u061C-',
    currencyPattern: '\u200F#,##0.00\u00A0¤',
    nan: 'ليس\u00A0رقمًا',
  ),
  'ar-OM': NumberSymbols(
    decimal: '٫',
    group: '٬',
    digits: '٠١٢٣٤٥٦٧٨٩',
    percentSign: '٪\u061C',
    minusSign: '\u061C-',
    currencyPattern: '\u200F#,##0.00\u00A0¤',
    nan: 'ليس\u00A0رقمًا',
  ),
  'ar-PS': NumberSymbols(
    decimal: '٫',
    group: '٬',
    digits: '٠١٢٣٤٥٦٧٨٩',
    percentSign: '٪\u061C',
    minusSign: '\u061C-',
    currencyPattern: '\u200F#,##0.00\u00A0¤',
    nan: 'ليس\u00A0رقمًا',
  ),
  'ar-QA': NumberSymbols(
    decimal: '٫',
    group: '٬',
    digits: '٠١٢٣٤٥٦٧٨٩',
    percentSign: '٪\u061C',
    minusSign: '\u061C-',
    currencyPattern: '\u200F#,##0.00\u00A0¤',
    nan: 'ليس\u00A0رقمًا',
  ),
  'ar-SA': NumberSymbols(
    decimal: '٫',
    group: '٬',
    digits: '٠١٢٣٤٥٦٧٨٩',
    percentSign: '٪\u061C',
    minusSign: '\u061C-',
    currencyPattern: '\u200F#,##0.00\u00A0¤',
    nan: 'ليس\u00A0رقمًا',
  ),
  'ar-SD': NumberSymbols(
    decimal: '٫',
    group: '٬',
    digits: '٠١٢٣٤٥٦٧٨٩',
    percentSign: '٪\u061C',
    minusSign: '\u061C-',
    currencyPattern: '\u200F#,##0.00\u00A0¤',
    nan: 'ليس\u00A0رقمًا',
  ),
  'ar-SO': NumberSymbols(
    decimal: '٫',
    group: '٬',
    digits: '٠١٢٣٤٥٦٧٨٩',
    percentSign: '٪\u061C',
    minusSign: '\u061C-',
    currencyPattern: '\u200F#,##0.00\u00A0¤',
    nan: 'ليس\u00A0رقمًا',
  ),
  'ar-SS': NumberSymbols(
    decimal: '٫',
    group: '٬',
    digits: '٠١٢٣٤٥٦٧٨٩',
    percentSign: '٪\u061C',
    minusSign: '\u061C-',
    currencyPattern: '\u200F#,##0.00\u00A0¤',
    nan: 'ليس\u00A0رقمًا',
  ),
  'ar-SY': NumberSymbols(
    decimal: '٫',
    group: '٬',
    digits: '٠١٢٣٤٥٦٧٨٩',
    percentSign: '٪\u061C',
    minusSign: '\u061C-',
    currencyPattern: '\u200F#,##0.00\u00A0¤',
    nan: 'ليس\u00A0رقمًا',
  ),
  'ar-TD': NumberSymbols(
    decimal: '٫',
    group: '٬',
    digits: '٠١٢٣٤٥٦٧٨٩',
    percentSign: '٪\u061C',
    minusSign: '\u061C-',
    currencyPattern: '\u200F#,##0.00\u00A0¤',
    nan: 'ليس\u00A0رقمًا',
  ),
  'ar-TN': NumberSymbols(
    decimal: ',',
    group: '.',
    percentSign: '\u200E%\u200E',
    minusSign: '\u200E-',
    currencyPattern: '\u200F#,##0.00\u00A0¤;\u200F-#,##0.00\u00A0¤',
    nan: 'ليس\u00A0رقمًا',
  ),
  'ar-YE': NumberSymbols(
    decimal: '٫',
    group: '٬',
    digits: '٠١٢٣٤٥٦٧٨٩',
    percentSign: '٪\u061C',
    minusSign: '\u061C-',
    currencyPattern: '\u200F#,##0.00\u00A0¤',
    nan: 'ليس\u00A0رقمًا',
  ),
  'arn': NumberSymbols(decimal: '.', group: ',', currencyPattern: '¤\u00A0#,##0.00'),
  'as': NumberSymbols(
    decimal: '.',
    group: ',',
    digits: '০১২৩৪৫৬৭৮৯',
    secondaryGroupingSize: 2,
    percentPattern: '#,##,##0%',
    currencyPattern: '¤\u00A0#,##,##0.00',
  ),
  'asa': NumberSymbols(decimal: '.', group: ',', currencyPattern: '#,##0.00\u00A0¤'),
  'ast': NumberSymbols(
    decimal: ',',
    group: '.',
    currencyPattern: '#,##0.00\u00A0¤',
    nan: 'ND',
  ),
  'az': NumberSymbols(decimal: ',', group: '.', currencyPattern: '#,##0.00\u00A0¤'),
  'az-Arab': NumberSymbols(
    decimal: '٫',
    group: '٬',
    digits: '۰۱۲۳۴۵۶۷۸۹',
    percentSign: '٪',
    minusSign: '\u200E-\u200E',
    currencyPattern: '¤\u00A0#,##0.00',
  ),
  'az-Arab-IQ': NumberSymbols(
    decimal: '٫',
    group: '٬',
    digits: '۰۱۲۳۴۵۶۷۸۹',
    percentSign: '٪',
    minusSign: '\u200E-\u200E',
    currencyPattern: '¤\u00A0#,##0.00',
  ),
  'az-Arab-TR': NumberSymbols(
    decimal: '٫',
    group: '٬',
    digits: '۰۱۲۳۴۵۶۷۸۹',
    percentSign: '٪',
    minusSign: '\u200E-\u200E',
    currencyPattern: '¤\u00A0#,##0.00',
  ),
  'ba': NumberSymbols(
    decimal: '.',
    group: '\u00A0',
    currencyPattern: '#,##0.00\u00A0¤',
  ),
  'bal': NumberSymbols(decimal: '.', group: ',', currencyPattern: '¤\u00A0#,##0.00'),
  'bas': NumberSymbols(
    decimal: ',',
    group: '\u00A0',
    percentPattern: '#,##0\u00A0%',
    currencyPattern: '#,##0.00\u00A0¤',
  ),
  'be': NumberSymbols(
    decimal: ',',
    group: '\u00A0',
    minimumGroupingDigits: 2,
    percentPattern: '#,##0\u00A0%',
    currencyPattern: '#,##0.00\u00A0¤',
  ),
  'bew': NumberSymbols(decimal: '.', group: ',', currencyPattern: '¤\u00A0#,##0.00'),
  'bez': NumberSymbols(decimal: '.', group: ',', currencyPattern: '#,##0.00¤'),
  'bg': NumberSymbols(
    decimal: ',',
    group: '\u00A0',
    minimumGroupingDigits: 2,
    currencyPattern: '#,##0.00\u00A0¤',
  ),
  'bgc': NumberSymbols(
    decimal: '.',
    group: ',',
    digits: '०१२३४५६७८९',
    currencyPattern: '¤\u00A0#,##0.00',
  ),
  'bgn': NumberSymbols(
    decimal: '٫',
    group: '٬',
    digits: '۰۱۲۳۴۵۶۷۸۹',
    percentSign: '٪',
    minusSign: '\u200E-\u200E',
    currencyPattern: '¤\u00A0#,##0.00',
  ),
  'bho': NumberSymbols(decimal: '.', group: ',', digits: '०१२३४५६७८९'),
  'blo': NumberSymbols(
    decimal: ',',
    group: '\u00A0',
    percentPattern: '%\u00A0#,#0;%\u00A0-#,#0',
    currencyPattern: '¤\u00A0#,##0.00;¤\u00A0-#,##0.00',
  ),
  'blt': NumberSymbols(decimal: '.', group: ',', currencyPattern: '¤\u00A0#,##0.00'),
  'bm-Nkoo': NumberSymbols(
    decimal: '.',
    group: ',',
    currencyPattern: '¤\u00A0#,##0.00',
  ),
  'bn': NumberSymbols(
    decimal: '.',
    group: ',',
    digits: '০১২৩৪৫৬৭৮৯',
    secondaryGroupingSize: 2,
    currencyPattern: '#,##,##0.00¤',
  ),
  'bn-IN': NumberSymbols(
    decimal: '.',
    group: ',',
    digits: '০১২৩৪৫৬৭৮৯',
    secondaryGroupingSize: 2,
    currencyPattern: '¤#,##,##0.00',
  ),
  'bo': NumberSymbols(decimal: '.', group: ',', currencyPattern: '¤\u00A0#,##0.00'),
  'bqi': NumberSymbols(decimal: '.', group: ',', currencyPattern: '¤\u00A0#,##0.00'),
  'br': NumberSymbols(
    decimal: ',',
    group: '\u00A0',
    percentPattern: '#,##0\u00A0%',
    currencyPattern: '#,##0.00\u00A0¤',
  ),
  'brx': NumberSymbols(
    decimal: '.',
    group: ',',
    secondaryGroupingSize: 2,
    percentPattern: '#,##,##0%',
    currencyPattern: '¤\u00A0#,##,##0.00',
  ),
  'bs': NumberSymbols(decimal: ',', group: '.', currencyPattern: '#,##0.00\u00A0¤'),
  'bss': NumberSymbols(decimal: '.', group: ',', currencyPattern: '¤\u00A0#,##0.00'),
  'bua': NumberSymbols(
    decimal: ',',
    group: '\u00A0',
    percentPattern: '#,##0\u00A0%',
    currencyPattern: '#,##0.00\u00A0¤',
  ),
  'byn': NumberSymbols(decimal: '.', group: ',', currencyPattern: '¤\u00A0#,##0.00'),
  'ca': NumberSymbols(
    decimal: ',',
    group: '.',
    percentPattern: '#,##0\u00A0%',
    currencyPattern: '#,##0.00\u00A0¤',
  ),
  'cad': NumberSymbols(decimal: '.', group: ',', currencyPattern: '¤\u00A0#,##0.00'),
  'cch': NumberSymbols(decimal: '.', group: ',', currencyPattern: '¤\u00A0#,##0.00'),
  'ccp': NumberSymbols(
    decimal: '.',
    group: ',',
    digits: '𑄶𑄷𑄸𑄹𑄺𑄻𑄼𑄽𑄾𑄿',
    secondaryGroupingSize: 2,
    currencyPattern: '#,##,##0.00¤',
  ),
  'ce': NumberSymbols(
    decimal: '.',
    group: ',',
    percentPattern: '#,##0\u00A0%',
    currencyPattern: '#,##0.00\u00A0¤',
    nan: 'Терхьаш\u00A0дац',
  ),
  'cho': NumberSymbols(decimal: '.', group: ',', currencyPattern: '¤\u00A0#,##0.00'),
  'cic': NumberSymbols(decimal: '.', group: ',', currencyPattern: '¤\u00A0#,##0.00'),
  'ckb': NumberSymbols(
    decimal: '٫',
    group: '٬',
    digits: '٠١٢٣٤٥٦٧٨٩',
    percentPattern: '#,##0\u00A0%',
    percentSign: '٪',
    minusSign: '\u200F-',
    currencyPattern: '#,##0.00\u00A0¤',
  ),
  'co': NumberSymbols(decimal: '.', group: ',', currencyPattern: '¤\u00A0#,##0.00'),
  'cop': NumberSymbols(decimal: '.', group: ',', currencyPattern: '¤\u00A0#,##0.00'),
  'cs': NumberSymbols(
    decimal: ',',
    group: '\u00A0',
    percentPattern: '#,##0\u00A0%',
    currencyPattern: '#,##0.00\u00A0¤',
  ),
  'csw': NumberSymbols(decimal: '.', group: ',', currencyPattern: '¤\u00A0#,##0.00'),
  'cu': NumberSymbols(decimal: '.', group: ',', currencyPattern: '¤\u00A0#,##0.00'),
  'cv': NumberSymbols(
    decimal: ',',
    group: '\u00A0',
    percentPattern: '#,##0\u00A0%',
    currencyPattern: '#,##0.00\u00A0¤',
    nan: 'хисеп\u00A0мар',
  ),
  'da': NumberSymbols(
    decimal: ',',
    group: '.',
    percentPattern: '#,##0\u00A0%',
    currencyPattern: '#,##0.00\u00A0¤',
  ),
  'de': NumberSymbols(
    decimal: ',',
    group: '.',
    percentPattern: '#,##0\u00A0%',
    currencyPattern: '#,##0.00\u00A0¤',
  ),
  'de-AT': NumberSymbols(
    decimal: ',',
    group: '\u00A0',
    percentPattern: '#,##0\u00A0%',
    currencyPattern: '¤\u00A0#,##0.00',
  ),
  'de-CH': NumberSymbols(
    decimal: '.',
    group: '\'',
    currencyPattern: '¤\u00A0#,##0.00;¤-#,##0.00',
  ),
  'de-LI': NumberSymbols(decimal: '.', group: '\'', currencyPattern: '¤\u00A0#,##0.00'),
  'dje': NumberSymbols(decimal: '.', group: '\u00A0', currencyPattern: '#,##0.00¤'),
  'dsb': NumberSymbols(
    decimal: ',',
    group: '.',
    percentPattern: '#,##0\u00A0%',
    currencyPattern: '#,##0.00\u00A0¤',
  ),
  'dua': NumberSymbols(
    decimal: ',',
    group: '\u00A0',
    percentPattern: '#,##0\u00A0%',
    currencyPattern: '#,##0.00\u00A0¤',
  ),
  'dv': NumberSymbols(decimal: '.', group: ',', currencyPattern: '¤\u00A0#,##0.00'),
  'dyo': NumberSymbols(
    decimal: ',',
    group: '\u00A0',
    currencyPattern: '#,##0.00\u00A0¤',
  ),
  'dz': NumberSymbols(
    decimal: '.',
    group: ',',
    digits: '༠༡༢༣༤༥༦༧༨༩',
    secondaryGroupingSize: 2,
    percentPattern: '#,##,##0\u00A0%',
    currencyPattern: '¤#,##,##0.00',
    infinity: 'གྲངས་མེད',
    nan: 'ཨང་མད',
  ),
  'ee': NumberSymbols(decimal: '.', group: ',', minimumGroupingDigits: 3, nan: 'mnn'),
  'el': NumberSymbols(decimal: ',', group: '.', currencyPattern: '#,##0.00\u00A0¤'),
  'en-150': NumberSymbols(decimal: '.', group: ',', currencyPattern: '#,##0.00\u00A0¤'),
  'en-AT': NumberSymbols(
    decimal: ',',
    group: '.',
    percentPattern: '#,##0\u00A0%',
    currencyPattern: '¤\u00A0#,##0.00',
  ),
  'en-BE': NumberSymbols(decimal: ',', group: '.', currencyPattern: '#,##0.00\u00A0¤'),
  'en-CH': NumberSymbols(
    decimal: '.',
    group: '\'',
    currencyPattern: '¤\u00A0#,##0.00;¤-#,##0.00',
  ),
  'en-CZ': NumberSymbols(
    decimal: ',',
    group: '\u00A0',
    currencyPattern: '#,##0.00\u00A0¤',
  ),
  'en-DE': NumberSymbols(
    decimal: ',',
    group: '.',
    percentPattern: '#,##0\u00A0%',
    currencyPattern: '#,##0.00\u00A0¤',
  ),
  'en-DK': NumberSymbols(
    decimal: ',',
    group: '.',
    percentPattern: '#,##0\u00A0%',
    currencyPattern: '#,##0.00\u00A0¤',
  ),
  'en-Dsrt': NumberSymbols(
    decimal: '.',
    group: ',',
    currencyPattern: '¤\u00A0#,##0.00',
  ),
  'en-EE': NumberSymbols(
    decimal: ',',
    group: '\u00A0',
    currencyPattern: '#,##0.00\u00A0¤',
  ),
  'en-ES': NumberSymbols(decimal: ',', group: '.', currencyPattern: '#,##0.00\u00A0¤'),
  'en-FI': NumberSymbols(
    decimal: ',',
    group: '\u00A0',
    percentPattern: '#,##0\u00A0%',
    currencyPattern: '#,##0.00\u00A0¤',
  ),
  'en-FR': NumberSymbols(
    decimal: ',',
    group: '\u202F',
    currencyPattern: '#,##0.00\u00A0¤',
  ),
  'en-GE': NumberSymbols(
    decimal: ',',
    group: '\u202F',
    currencyPattern: '#,##0.00\u00A0¤',
  ),
  'en-HU': NumberSymbols(
    decimal: ',',
    group: '\u00A0',
    currencyPattern: '#,##0.00\u00A0¤',
  ),
  'en-ID': NumberSymbols(decimal: ',', group: '.'),
  'en-IN': NumberSymbols(
    decimal: '.',
    group: ',',
    secondaryGroupingSize: 2,
    percentPattern: '#,##,##0%',
    currencyPattern: '¤#,##,##0.00',
  ),
  'en-IT': NumberSymbols(decimal: ',', group: '.', currencyPattern: '#,##0.00\u00A0¤'),
  'en-LT': NumberSymbols(
    decimal: ',',
    group: '\u00A0',
    currencyPattern: '#,##0.00\u00A0¤',
  ),
  'en-LV': NumberSymbols(
    decimal: ',',
    group: '\u00A0',
    currencyPattern: '#,##0.00\u00A0¤',
  ),
  'en-MV': NumberSymbols(decimal: '.', group: ',', currencyPattern: '¤\u00A0#,##0.00'),
  'en-NL': NumberSymbols(
    decimal: ',',
    group: '.',
    currencyPattern: '¤\u00A0#,##0.00;¤\u00A0-#,##0.00',
  ),
  'en-NO': NumberSymbols(
    decimal: ',',
    group: '\u00A0',
    percentPattern: '#,##0\u00A0%',
    currencyPattern: '#,##0.00\u00A0¤',
  ),
  'en-PL': NumberSymbols(decimal: ',', group: '.', currencyPattern: '#,##0.00\u00A0¤'),
  'en-PT': NumberSymbols(
    decimal: ',',
    group: '\u00A0',
    currencyPattern: '#,##0.00\u00A0¤',
  ),
  'en-RO': NumberSymbols(decimal: ',', group: '.', currencyPattern: '#,##0.00\u00A0¤'),
  'en-SE': NumberSymbols(
    decimal: ',',
    group: '\u00A0',
    percentPattern: '#,##0\u00A0%',
    currencyPattern: '#,##0.00\u00A0¤',
  ),
  'en-Shaw': NumberSymbols(
    decimal: '.',
    group: ',',
    currencyPattern: '¤\u00A0#,##0.00',
  ),
  'en-SI': NumberSymbols(decimal: ',', group: '.', currencyPattern: '#,##0.00\u00A0¤'),
  'en-SK': NumberSymbols(
    decimal: ',',
    group: '\u00A0',
    currencyPattern: '#,##0.00\u00A0¤',
  ),
  'en-UA': NumberSymbols(
    decimal: ',',
    group: '\u00A0',
    currencyPattern: '#,##0.00\u00A0¤',
  ),
  'en-ZA': NumberSymbols(decimal: ',', group: '\u00A0'),
  'eo': NumberSymbols(
    decimal: ',',
    group: '\u202F',
    currencyPattern: '#,##0.00\u202F¤',
  ),
  'es': NumberSymbols(
    decimal: ',',
    group: '.',
    minimumGroupingDigits: 2,
    percentPattern: '#,##0\u00A0%',
    currencyPattern: '#,##0.00\u00A0¤',
  ),
  'es-419': NumberSymbols(decimal: '.', group: ','),
  'es-AR': NumberSymbols(decimal: ',', group: '.', currencyPattern: '¤\u00A0#,##0.00'),
  'es-BO': NumberSymbols(decimal: ',', group: '.'),
  'es-BR': NumberSymbols(decimal: '.', group: ','),
  'es-BZ': NumberSymbols(decimal: '.', group: ','),
  'es-CL': NumberSymbols(
    decimal: ',',
    group: '.',
    currencyPattern: '¤#,##0.00;¤-#,##0.00',
  ),
  'es-CO': NumberSymbols(decimal: ',', group: '.', currencyPattern: '¤\u00A0#,##0.00'),
  'es-CR': NumberSymbols(decimal: ',', group: '\u00A0'),
  'es-CU': NumberSymbols(decimal: '.', group: ','),
  'es-DO': NumberSymbols(decimal: '.', group: ','),
  'es-EC': NumberSymbols(
    decimal: ',',
    group: '.',
    currencyPattern: '¤#,##0.00;¤-#,##0.00',
  ),
  'es-GQ': NumberSymbols(
    decimal: ',',
    group: '.',
    minimumGroupingDigits: 2,
    percentPattern: '#,##0\u00A0%',
  ),
  'es-GT': NumberSymbols(decimal: '.', group: ','),
  'es-HN': NumberSymbols(decimal: '.', group: ','),
  'es-MX': NumberSymbols(decimal: '.', group: ','),
  'es-NI': NumberSymbols(decimal: '.', group: ','),
  'es-PA': NumberSymbols(decimal: '.', group: ','),
  'es-PE': NumberSymbols(decimal: '.', group: ',', currencyPattern: '¤\u00A0#,##0.00'),
  'es-PR': NumberSymbols(decimal: '.', group: ','),
  'es-PY': NumberSymbols(
    decimal: ',',
    group: '.',
    currencyPattern: '¤\u00A0#,##0.00;¤\u00A0-#,##0.00',
  ),
  'es-SV': NumberSymbols(decimal: '.', group: ','),
  'es-US': NumberSymbols(decimal: '.', group: ','),
  'es-UY': NumberSymbols(decimal: ',', group: '.', currencyPattern: '¤\u00A0#,##0.00'),
  'es-VE': NumberSymbols(
    decimal: ',',
    group: '.',
    currencyPattern: '¤#,##0.00;¤-#,##0.00',
  ),
  'et': NumberSymbols(
    decimal: ',',
    group: '\u00A0',
    minimumGroupingDigits: 2,
    minusSign: '−',
    currencyPattern: '#,##0.00\u00A0¤',
  ),
  'eu': NumberSymbols(
    decimal: ',',
    group: '.',
    percentPattern: '%\u00A0#,##0',
    minusSign: '−',
    currencyPattern: '#,##0.00\u00A0¤',
  ),
  'ewo': NumberSymbols(
    decimal: ',',
    group: '\u00A0',
    currencyPattern: '#,##0.00\u00A0¤',
  ),
  'fa': NumberSymbols(
    decimal: '٫',
    group: '٬',
    digits: '۰۱۲۳۴۵۶۷۸۹',
    percentSign: '٪',
    minusSign: '\u200E−',
    currencyPattern: '\u200E¤#,##0.00',
    nan: 'ناعدد',
  ),
  'fa-AF': NumberSymbols(
    decimal: '٫',
    group: '٬',
    digits: '۰۱۲۳۴۵۶۷۸۹',
    percentSign: '٪',
    minusSign: '\u200E−',
    currencyPattern: '¤\u00A0#,##0.00',
    nan: 'ناعدد',
  ),
  'ff': NumberSymbols(
    decimal: ',',
    group: '\u00A0',
    currencyPattern: '#,##0.00\u00A0¤',
  ),
  'ff-Adlm': NumberSymbols(
    decimal: '.',
    group: '⹁',
    digits: '𞥐𞥑𞥒𞥓𞥔𞥕𞥖𞥗𞥘𞥙',
    currencyPattern: '¤\u00A0#,##0.00',
  ),
  'ff-Adlm-BF': NumberSymbols(
    decimal: '.',
    group: '⹁',
    digits: '𞥐𞥑𞥒𞥓𞥔𞥕𞥖𞥗𞥘𞥙',
    currencyPattern: '¤\u00A0#,##0.00',
  ),
  'ff-Adlm-CM': NumberSymbols(
    decimal: '.',
    group: '⹁',
    digits: '𞥐𞥑𞥒𞥓𞥔𞥕𞥖𞥗𞥘𞥙',
    currencyPattern: '¤\u00A0#,##0.00',
  ),
  'ff-Adlm-GH': NumberSymbols(
    decimal: '.',
    group: '⹁',
    digits: '𞥐𞥑𞥒𞥓𞥔𞥕𞥖𞥗𞥘𞥙',
    currencyPattern: '¤\u00A0#,##0.00',
  ),
  'ff-Adlm-GM': NumberSymbols(
    decimal: '.',
    group: '⹁',
    digits: '𞥐𞥑𞥒𞥓𞥔𞥕𞥖𞥗𞥘𞥙',
    currencyPattern: '¤\u00A0#,##0.00',
  ),
  'ff-Adlm-GW': NumberSymbols(
    decimal: '.',
    group: '⹁',
    digits: '𞥐𞥑𞥒𞥓𞥔𞥕𞥖𞥗𞥘𞥙',
    currencyPattern: '¤\u00A0#,##0.00',
  ),
  'ff-Adlm-LR': NumberSymbols(
    decimal: '.',
    group: '⹁',
    digits: '𞥐𞥑𞥒𞥓𞥔𞥕𞥖𞥗𞥘𞥙',
    currencyPattern: '¤\u00A0#,##0.00',
  ),
  'ff-Adlm-MR': NumberSymbols(
    decimal: '.',
    group: '⹁',
    digits: '𞥐𞥑𞥒𞥓𞥔𞥕𞥖𞥗𞥘𞥙',
    currencyPattern: '¤\u00A0#,##0.00',
  ),
  'ff-Adlm-NE': NumberSymbols(
    decimal: '.',
    group: '⹁',
    digits: '𞥐𞥑𞥒𞥓𞥔𞥕𞥖𞥗𞥘𞥙',
    currencyPattern: '¤\u00A0#,##0.00',
  ),
  'ff-Adlm-NG': NumberSymbols(
    decimal: '.',
    group: '⹁',
    digits: '𞥐𞥑𞥒𞥓𞥔𞥕𞥖𞥗𞥘𞥙',
    currencyPattern: '¤\u00A0#,##0.00',
  ),
  'ff-Adlm-SL': NumberSymbols(
    decimal: '.',
    group: '⹁',
    digits: '𞥐𞥑𞥒𞥓𞥔𞥕𞥖𞥗𞥘𞥙',
    currencyPattern: '¤\u00A0#,##0.00',
  ),
  'ff-Adlm-SN': NumberSymbols(
    decimal: '.',
    group: '⹁',
    digits: '𞥐𞥑𞥒𞥓𞥔𞥕𞥖𞥗𞥘𞥙',
    currencyPattern: '¤\u00A0#,##0.00',
  ),
  'fi': NumberSymbols(
    decimal: ',',
    group: '\u00A0',
    percentPattern: '#,##0\u00A0%',
    minusSign: '−',
    currencyPattern: '#,##0.00\u00A0¤',
    nan: 'epäluku',
  ),
  'fo': NumberSymbols(
    decimal: ',',
    group: '.',
    percentPattern: '#,##0\u00A0%',
    minusSign: '−',
    currencyPattern: '#,##0.00\u00A0¤',
  ),
  'fr': NumberSymbols(
    decimal: ',',
    group: '\u202F',
    percentPattern: '#,##0\u00A0%',
    currencyPattern: '#,##0.00\u00A0¤',
  ),
  'fr-CA': NumberSymbols(
    decimal: ',',
    group: '\u00A0',
    percentPattern: '#,##0\u00A0%',
    currencyPattern: '#,##0.00\u00A0¤',
  ),
  'fr-CH': NumberSymbols(decimal: ',', group: '\'', currencyPattern: '#,##0.00\u00A0¤'),
  'fr-LU': NumberSymbols(
    decimal: ',',
    group: '.',
    percentPattern: '#,##0\u00A0%',
    currencyPattern: '#,##0.00\u00A0¤',
  ),
  'fr-MA': NumberSymbols(
    decimal: ',',
    group: '.',
    percentPattern: '#,##0\u00A0%',
    currencyPattern: '#,##0.00\u00A0¤',
  ),
  'frr': NumberSymbols(decimal: '.', group: ',', currencyPattern: '¤\u00A0#,##0.00'),
  'fur': NumberSymbols(decimal: ',', group: '.', currencyPattern: '¤\u00A0#,##0.00'),
  'fy': NumberSymbols(
    decimal: ',',
    group: '.',
    currencyPattern: '¤\u00A0#,##0.00;¤\u00A0#,##0.00-',
  ),
  'ga': NumberSymbols(decimal: '.', group: ',', nan: 'Nuimh'),
  'gaa': NumberSymbols(decimal: '.', group: ',', currencyPattern: '¤\u00A0#,##0.00'),
  'gez': NumberSymbols(decimal: '.', group: ',', currencyPattern: '¤\u00A0#,##0.00'),
  'gl': NumberSymbols(
    decimal: ',',
    group: '.',
    percentPattern: '#,##0\u00A0%',
    currencyPattern: '#,##0.00\u00A0¤',
  ),
  'gn': NumberSymbols(decimal: '.', group: ',', currencyPattern: '¤\u00A0#,##0.00'),
  'gsw': NumberSymbols(
    decimal: '.',
    group: '\'',
    percentPattern: '#,##0\u00A0%',
    minusSign: '−',
    currencyPattern: '#,##0.00\u00A0¤',
  ),
  'gu': NumberSymbols(
    decimal: '.',
    group: ',',
    secondaryGroupingSize: 2,
    percentPattern: '#,##,##0%',
    currencyPattern: '¤#,##,##0.00',
  ),
  'ha': NumberSymbols(decimal: '.', group: ',', currencyPattern: '¤\u00A0#,##0.00'),
  'he': NumberSymbols(
    decimal: '.',
    group: ',',
    minusSign: '\u200E-',
    currencyPattern: '\u200F#,##0.00\u00A0\u200F¤;\u200F-#,##0.00\u00A0\u200F¤',
  ),
  'hi': NumberSymbols(
    decimal: '.',
    group: ',',
    secondaryGroupingSize: 2,
    percentPattern: '#,##,##0%',
    currencyPattern: '¤#,##,##0.00',
  ),
  'hnj': NumberSymbols(
    decimal: '.',
    group: ',',
    digits: '𞅀𞅁𞅂𞅃𞅄𞅅𞅆𞅇𞅈𞅉',
    currencyPattern: '¤\u00A0#,##0.00',
  ),
  'hr': NumberSymbols(
    decimal: ',',
    group: '.',
    percentPattern: '#,##0\u00A0%',
    minusSign: '−',
    currencyPattern: '#,##0.00\u00A0¤',
  ),
  'hsb': NumberSymbols(
    decimal: ',',
    group: '.',
    percentPattern: '#,##0\u00A0%',
    currencyPattern: '#,##0.00\u00A0¤',
  ),
  'ht': NumberSymbols(
    decimal: ',',
    group: '\u00A0',
    percentPattern: '#,##0\u00A0%',
    currencyPattern: '#,##0.00\u00A0¤',
  ),
  'hu': NumberSymbols(
    decimal: ',',
    group: '\u00A0',
    minimumGroupingDigits: 2,
    currencyPattern: '#,##0.00\u00A0¤',
  ),
  'hy': NumberSymbols(
    decimal: ',',
    group: '\u00A0',
    minimumGroupingDigits: 2,
    currencyPattern: '#,##0.00\u00A0¤',
    nan: 'ՈչԹ',
  ),
  'ia': NumberSymbols(
    decimal: ',',
    group: '.',
    minimumGroupingDigits: 2,
    currencyPattern: '¤\u00A0#,##0.00',
  ),
  'id': NumberSymbols(decimal: ',', group: '.'),
  'ie': NumberSymbols(
    decimal: ',',
    group: '\u00A0',
    minimumGroupingDigits: 2,
    currencyPattern: '¤\u00A0#,##0.00;¤\u00A0-#,##0.00',
  ),
  'ii': NumberSymbols(decimal: '.', group: ',', currencyPattern: '¤\u00A0#,##0.00'),
  'io': NumberSymbols(decimal: '.', group: ',', currencyPattern: '¤\u00A0#,##0.00'),
  'is': NumberSymbols(decimal: ',', group: '.', currencyPattern: '#,##0.00\u00A0¤'),
  'it': NumberSymbols(
    decimal: ',',
    group: '.',
    minimumGroupingDigits: 2,
    currencyPattern: '#,##0.00\u00A0¤',
  ),
  'it-CH': NumberSymbols(
    decimal: '.',
    group: '\'',
    minimumGroupingDigits: 2,
    currencyPattern: '¤\u00A0#,##0.00;¤-#,##0.00',
  ),
  'iu': NumberSymbols(decimal: '.', group: ',', currencyPattern: '¤\u00A0#,##0.00'),
  'jbo': NumberSymbols(decimal: '.', group: ',', currencyPattern: '¤\u00A0#,##0.00'),
  'jgo': NumberSymbols(decimal: ',', group: '.', currencyPattern: '¤\u00A0#,##0.00'),
  'jv': NumberSymbols(decimal: ',', group: '.', currencyPattern: '¤\u00A0#,##0.00'),
  'ka': NumberSymbols(
    decimal: ',',
    group: '\u00A0',
    minimumGroupingDigits: 2,
    currencyPattern: '#,##0.00\u00A0¤',
    nan: 'არ\u00A0არის\u00A0რიცხვი',
  ),
  'kaa': NumberSymbols(decimal: '.', group: ',', currencyPattern: '¤\u00A0#,##0.00'),
  'kab': NumberSymbols(decimal: ',', group: '\u00A0', currencyPattern: '#,##0.00¤'),
  'kaj': NumberSymbols(decimal: '.', group: ',', currencyPattern: '¤\u00A0#,##0.00'),
  'kcg': NumberSymbols(decimal: '.', group: ',', currencyPattern: '¤\u00A0#,##0.00'),
  'kea': NumberSymbols(
    decimal: ',',
    group: '\u00A0',
    currencyPattern: '#,##0.00\u00A0¤',
  ),
  'kek': NumberSymbols(decimal: '.', group: ',', currencyPattern: '¤\u00A0#,##0.00'),
  'ken': NumberSymbols(decimal: '.', group: ',', currencyPattern: '¤\u00A0#,##0.00'),
  'kgp': NumberSymbols(decimal: ',', group: '.', currencyPattern: '¤\u00A0#,##0.00'),
  'khq': NumberSymbols(decimal: '.', group: '\u00A0', currencyPattern: '#,##0.00¤'),
  'kk': NumberSymbols(
    decimal: ',',
    group: '\u00A0',
    currencyPattern: '#,##0.00\u00A0¤',
    nan: 'сан\u00A0емес',
  ),
  'kk-Arab': NumberSymbols(
    decimal: '.',
    group: ',',
    currencyPattern: '¤\u00A0#,##0.00',
    nan: 'سان\u00A0ەمەس',
  ),
  'kkj': NumberSymbols(decimal: ',', group: '.', currencyPattern: '¤\u00A0#,##0.00'),
  'kl': NumberSymbols(
    decimal: ',',
    group: '.',
    percentPattern: '#,##0\u00A0%',
    currencyPattern: '¤#,##0.00;¤-#,##0.00',
  ),
  'km': NumberSymbols(decimal: '.', group: ',', currencyPattern: '#,##0.00¤'),
  'kok': NumberSymbols(
    decimal: '.',
    group: ',',
    secondaryGroupingSize: 2,
    currencyPattern: '¤#,##,##0.00',
  ),
  'kpe': NumberSymbols(decimal: '.', group: ',', currencyPattern: '¤\u00A0#,##0.00'),
  'ks': NumberSymbols(
    decimal: '٫',
    group: '٬',
    digits: '۰۱۲۳۴۵۶۷۸۹',
    percentSign: '٪',
    minusSign: '\u200E-\u200E',
  ),
  'ks-Deva': NumberSymbols(
    decimal: '.',
    group: ',',
    currencyPattern: '¤\u00A0#,##0.00',
  ),
  'ksb': NumberSymbols(decimal: '.', group: ',', currencyPattern: '#,##0.00¤'),
  'ksf': NumberSymbols(
    decimal: ',',
    group: '\u00A0',
    currencyPattern: '#,##0.00\u00A0¤',
  ),
  'ksh': NumberSymbols(
    decimal: ',',
    group: '\u00A0',
    percentPattern: '#,##0\u00A0%',
    minusSign: '−',
    currencyPattern: '#,##0.00\u00A0¤',
  ),
  'ku': NumberSymbols(
    decimal: ',',
    group: '.',
    percentPattern: '%#,##0',
    currencyPattern: '#,##0.00\u00A0¤',
  ),
  'ku-Arab': NumberSymbols(
    decimal: '.',
    group: ',',
    currencyPattern: '¤\u00A0#,##0.00',
  ),
  'ku-Arab-IR': NumberSymbols(
    decimal: '.',
    group: ',',
    currencyPattern: '¤\u00A0#,##0.00',
  ),
  'kxv': NumberSymbols(
    decimal: '.',
    group: ',',
    secondaryGroupingSize: 2,
    currencyPattern: '¤#,##,##0.00',
  ),
  'ky': NumberSymbols(
    decimal: ',',
    group: '\u00A0',
    currencyPattern: '#,##0.00\u00A0¤',
    nan: 'сан\u00A0эмес',
  ),
  'la': NumberSymbols(decimal: '.', group: ',', currencyPattern: '¤\u00A0#,##0.00'),
  'lag': NumberSymbols(decimal: '.', group: ',', currencyPattern: '¤\u00A0#,##0.00'),
  'lb': NumberSymbols(
    decimal: ',',
    group: '.',
    percentPattern: '#,##0\u00A0%',
    currencyPattern: '#,##0.00\u00A0¤',
  ),
  'lg': NumberSymbols(decimal: '.', group: ',', currencyPattern: '#,##0.00¤'),
  'lij': NumberSymbols(decimal: ',', group: '.', currencyPattern: '#,##0.00\u00A0¤'),
  'lkt': NumberSymbols(decimal: '.', group: ',', currencyPattern: '¤\u00A0#,##0.00'),
  'lld': NumberSymbols(decimal: ',', group: '.', currencyPattern: '#,##0.00\u00A0¤'),
  'lmo': NumberSymbols(decimal: ',', group: '\'', currencyPattern: '¤\u00A0#,##0.00'),
  'ln': NumberSymbols(decimal: ',', group: '.', currencyPattern: '#,##0.00\u00A0¤'),
  'lo': NumberSymbols(
    decimal: ',',
    group: '.',
    currencyPattern: '¤#,##0.00;¤-#,##0.00',
    nan: 'ບໍ່\u200Bແມ່ນ\u200Bໂຕ\u200Bເລກ',
  ),
  'lrc': NumberSymbols(
    decimal: '٫',
    group: '٬',
    digits: '۰۱۲۳۴۵۶۷۸۹',
    percentSign: '٪',
    minusSign: '\u200E-\u200E',
    currencyPattern: '¤\u00A0#,##0.00',
  ),
  'lt': NumberSymbols(
    decimal: ',',
    group: '\u00A0',
    percentPattern: '#,##0\u00A0%',
    minusSign: '−',
    currencyPattern: '#,##0.00\u00A0¤',
  ),
  'ltg': NumberSymbols(decimal: '.', group: ',', currencyPattern: '¤\u00A0#,##0.00'),
  'lu': NumberSymbols(decimal: ',', group: '.', currencyPattern: '#,##0.00¤'),
  'luo': NumberSymbols(decimal: '.', group: ',', currencyPattern: '#,##0.00¤'),
  'luy': NumberSymbols(
    decimal: '.',
    group: ',',
    currencyPattern: '¤#,##0.00;¤-\u00A0#,##0.00',
  ),
  'lv': NumberSymbols(
    decimal: ',',
    group: '\u00A0',
    minimumGroupingDigits: 2,
    currencyPattern: '#,##0.00\u00A0¤',
    nan: 'NS',
  ),
  'lzz': NumberSymbols(decimal: '.', group: ',', currencyPattern: '¤\u00A0#,##0.00'),
  'mai': NumberSymbols(decimal: '.', group: ',', currencyPattern: '¤\u00A0#,##0.00'),
  'mdf': NumberSymbols(decimal: '.', group: ',', currencyPattern: '¤\u00A0#,##0.00'),
  'mfe': NumberSymbols(
    decimal: '.',
    group: '\u00A0',
    currencyPattern: '¤\u00A0#,##0.00',
  ),
  'mg': NumberSymbols(decimal: '.', group: ',', currencyPattern: '¤\u00A0#,##0.00'),
  'mgh': NumberSymbols(decimal: ',', group: '.', currencyPattern: '¤\u00A0#,##0.00'),
  'mgo': NumberSymbols(decimal: '.', group: ',', currencyPattern: '¤\u00A0#,##0.00'),
  'mhn': NumberSymbols(decimal: '.', group: ',', currencyPattern: '¤\u00A0#,##0.00'),
  'mi': NumberSymbols(decimal: '.', group: ',', currencyPattern: '¤\u00A0#,##0.00'),
  'mic': NumberSymbols(decimal: '.', group: ',', currencyPattern: '¤\u00A0#,##0.00'),
  'mk': NumberSymbols(
    decimal: ',',
    group: '.',
    percentPattern: '#,##0\u00A0%',
    currencyPattern: '#,##0.00\u00A0¤',
  ),
  'ml': NumberSymbols(decimal: '.', group: ',', secondaryGroupingSize: 2),
  'mn': NumberSymbols(decimal: '.', group: ',', currencyPattern: '¤\u00A0#,##0.00'),
  'mn-Mong-MN': NumberSymbols(decimal: '.', group: ','),
  'mni': NumberSymbols(
    decimal: '.',
    group: ',',
    digits: '০১২৩৪৫৬৭৮৯',
    currencyPattern: '¤\u00A0#,##0.00',
  ),
  'mni-Mtei': NumberSymbols(
    decimal: '.',
    group: ',',
    digits: '꯰꯱꯲꯳꯴꯵꯶꯷꯸꯹',
    currencyPattern: '¤\u00A0#,##0.00',
  ),
  'moh': NumberSymbols(decimal: '.', group: ',', currencyPattern: '¤\u00A0#,##0.00'),
  'mr': NumberSymbols(
    decimal: '.',
    group: ',',
    digits: '०१२३४५६७८९',
    secondaryGroupingSize: 2,
  ),
  'ms-Arab-BN': NumberSymbols(
    decimal: ',',
    group: '.',
    currencyPattern: '¤\u00A0#,##0.00',
  ),
  'ms-BN': NumberSymbols(decimal: ',', group: '.', currencyPattern: '¤\u00A0#,##0.00'),
  'ms-ID': NumberSymbols(decimal: ',', group: '.'),
  'mua': NumberSymbols(decimal: ',', group: '.'),
  'mus': NumberSymbols(decimal: '.', group: ',', currencyPattern: '¤\u00A0#,##0.00'),
  'mww': NumberSymbols(
    decimal: '.',
    group: ',',
    digits: '𞅀𞅁𞅂𞅃𞅄𞅅𞅆𞅇𞅈𞅉',
    currencyPattern: '¤\u00A0#,##0.00',
  ),
  'my': NumberSymbols(
    decimal: '.',
    group: ',',
    digits: '၀၁၂၃၄၅၆၇၈၉',
    currencyPattern: '#,##0.00\u00A0¤',
    nan: 'ဂဏန်းမဟုတ်သော',
  ),
  'myv': NumberSymbols(decimal: '.', group: ',', currencyPattern: '¤\u00A0#,##0.00'),
  'mzn': NumberSymbols(
    decimal: '٫',
    group: '٬',
    digits: '۰۱۲۳۴۵۶۷۸۹',
    percentSign: '٪',
    minusSign: '\u200E-\u200E',
    currencyPattern: '¤\u00A0#,##0.00',
  ),
  'nb': NumberSymbols(
    decimal: ',',
    group: '\u00A0',
    percentPattern: '#,##0\u00A0%',
    minusSign: '−',
    currencyPattern: '#,##0.00\u00A0¤;-#,##0.00\u00A0¤',
  ),
  'nds': NumberSymbols(
    decimal: ',',
    group: '.',
    percentPattern: '#,##0\u00A0%',
    currencyPattern: '#,##0.00\u00A0¤',
  ),
  'ne': NumberSymbols(
    decimal: '.',
    group: ',',
    digits: '०१२३४५६७८९',
    secondaryGroupingSize: 2,
    percentPattern: '#,##,##0%',
    currencyPattern: '¤\u00A0#,##,##0.00',
  ),
  'nl': NumberSymbols(
    decimal: ',',
    group: '.',
    currencyPattern: '¤\u00A0#,##0.00;¤\u00A0-#,##0.00',
  ),
  'nmg': NumberSymbols(
    decimal: ',',
    group: '\u00A0',
    currencyPattern: '#,##0.00\u00A0¤',
  ),
  'nn': NumberSymbols(
    decimal: ',',
    group: '\u00A0',
    percentPattern: '#,##0\u00A0%',
    minusSign: '−',
    currencyPattern: '#,##0.00\u00A0¤;-#,##0.00\u00A0¤',
  ),
  'nnh': NumberSymbols(decimal: ',', group: '.', currencyPattern: '¤\u00A0#,##0.00'),
  'no': NumberSymbols(
    decimal: ',',
    group: '\u00A0',
    percentPattern: '#,##0\u00A0%',
    minusSign: '−',
    currencyPattern: '#,##0.00\u00A0¤;-#,##0.00\u00A0¤',
  ),
  'nqo': NumberSymbols(
    decimal: '.',
    group: '،',
    digits: '߀߁߂߃߄߅߆߇߈߉',
    currencyPattern: '¤\u00A0#,##0.00',
  ),
  'nr': NumberSymbols(decimal: ',', group: '\u00A0'),
  'nso': NumberSymbols(decimal: '.', group: ',', currencyPattern: '¤\u00A0#,##0.00'),
  'nv': NumberSymbols(decimal: '.', group: ',', currencyPattern: '¤\u00A0#,##0.00'),
  'ny': NumberSymbols(decimal: '.', group: ',', currencyPattern: '¤\u00A0#,##0.00'),
  'oc': NumberSymbols(
    decimal: ',',
    group: '\u00A0',
    percentPattern: '#,##0\u202F%',
    currencyPattern: '#,##0.00¤',
  ),
  'oka': NumberSymbols(decimal: '.', group: ',', currencyPattern: '¤\u00A0#,##0.00'),
  'or': NumberSymbols(decimal: '.', group: ',', secondaryGroupingSize: 2),
  'os': NumberSymbols(
    decimal: ',',
    group: '\u00A0',
    currencyPattern: '¤\u00A0#,##0.00',
    nan: 'НН',
  ),
  'osa': NumberSymbols(decimal: '.', group: ',', currencyPattern: '¤\u00A0#,##0.00'),
  'pa': NumberSymbols(
    decimal: '.',
    group: ',',
    secondaryGroupingSize: 2,
    percentPattern: '#,##,##0%',
    currencyPattern: '¤#,##,##0.00',
  ),
  'pa-Arab': NumberSymbols(
    decimal: '٫',
    group: '٬',
    digits: '۰۱۲۳۴۵۶۷۸۹',
    percentSign: '٪',
    minusSign: '\u200E-\u200E',
    currencyPattern: '¤\u00A0#,##0.00',
  ),
  'pap': NumberSymbols(decimal: '.', group: ',', currencyPattern: '¤\u00A0#,##0.00'),
  'pi': NumberSymbols(decimal: '.', group: ',', currencyPattern: '¤\u00A0#,##0.00'),
  'pis': NumberSymbols(decimal: '.', group: ',', currencyPattern: '¤\u00A0#,##0.00'),
  'pl': NumberSymbols(
    decimal: ',',
    group: '\u00A0',
    minimumGroupingDigits: 2,
    currencyPattern: '#,##0.00\u00A0¤',
  ),
  'pms': NumberSymbols(decimal: ',', group: '.', currencyPattern: '#,##0.00\u00A0¤'),
  'prg': NumberSymbols(
    decimal: ',',
    group: '\u00A0',
    currencyPattern: '#,##0.00\u00A0¤',
  ),
  'ps': NumberSymbols(
    decimal: '٫',
    group: '٬',
    digits: '۰۱۲۳۴۵۶۷۸۹',
    percentSign: '٪',
    minusSign: '\u200E-\u200E',
    currencyPattern: '¤\u00A0#,##0.00',
  ),
  'pt': NumberSymbols(decimal: ',', group: '.', currencyPattern: '¤\u00A0#,##0.00'),
  'pt-AO': NumberSymbols(
    decimal: ',',
    group: '\u00A0',
    currencyPattern: '#,##0.00\u00A0¤',
  ),
  'pt-CH': NumberSymbols(
    decimal: ',',
    group: '\u00A0',
    minimumGroupingDigits: 2,
    currencyPattern: '#,##0.00\u00A0¤',
  ),
  'pt-CV': NumberSymbols(
    decimal: ',',
    group: '\u00A0',
    minimumGroupingDigits: 2,
    currencyPattern: '#,##0.00\u00A0¤',
  ),
  'pt-GQ': NumberSymbols(
    decimal: ',',
    group: '\u00A0',
    minimumGroupingDigits: 2,
    currencyPattern: '#,##0.00\u00A0¤',
  ),
  'pt-GW': NumberSymbols(
    decimal: ',',
    group: '\u00A0',
    minimumGroupingDigits: 2,
    currencyPattern: '#,##0.00\u00A0¤',
  ),
  'pt-LU': NumberSymbols(
    decimal: ',',
    group: '\u00A0',
    minimumGroupingDigits: 2,
    currencyPattern: '#,##0.00\u00A0¤',
  ),
  'pt-MO': NumberSymbols(
    decimal: ',',
    group: '\u00A0',
    minimumGroupingDigits: 2,
    currencyPattern: '#,##0.00\u00A0¤',
  ),
  'pt-MZ': NumberSymbols(
    decimal: ',',
    group: '\u00A0',
    minimumGroupingDigits: 2,
    currencyPattern: '#,##0.00\u00A0¤',
  ),
  'pt-PT': NumberSymbols(
    decimal: ',',
    group: '\u00A0',
    minimumGroupingDigits: 2,
    currencyPattern: '#,##0.00\u00A0¤',
  ),
  'pt-ST': NumberSymbols(
    decimal: ',',
    group: '\u00A0',
    minimumGroupingDigits: 2,
    currencyPattern: '#,##0.00\u00A0¤',
  ),
  'pt-TL': NumberSymbols(
    decimal: ',',
    group: '\u00A0',
    minimumGroupingDigits: 2,
    currencyPattern: '#,##0.00\u00A0¤',
  ),
  'qu': NumberSymbols(
    decimal: '.',
    group: ',',
    percentPattern: '#,##0\u00A0%',
    currencyPattern: '¤\u00A0#,##0.00',
  ),
  'qu-BO': NumberSymbols(
    decimal: ',',
    group: '.',
    percentPattern: '#,##0\u00A0%',
    currencyPattern: '¤\u00A0#,##0.00',
  ),
  'quc': NumberSymbols(decimal: '.', group: ',', currencyPattern: '¤\u00A0#,##0.00'),
  'raj': NumberSymbols(
    decimal: '.',
    group: ',',
    digits: '०१२३४५६७८९',
    currencyPattern: '¤\u00A0#,##0.00',
  ),
  'rhg': NumberSymbols(decimal: '.', group: ',', currencyPattern: '¤\u00A0#,##0.00'),
  'rif': NumberSymbols(decimal: '.', group: ',', currencyPattern: '¤\u00A0#,##0.00'),
  'rm': NumberSymbols(decimal: ',', group: '\u202F'),
  'rn': NumberSymbols(
    decimal: ',',
    group: '.',
    percentPattern: '#,##0\u00A0%',
    currencyPattern: '#,##0.00¤',
  ),
  'ro': NumberSymbols(
    decimal: ',',
    group: '.',
    percentPattern: '#,##0\u00A0%',
    currencyPattern: '#,##0.00\u00A0¤',
  ),
  'ru': NumberSymbols(
    decimal: ',',
    group: '\u00A0',
    percentPattern: '#,##0\u00A0%',
    currencyPattern: '#,##0.00\u00A0¤',
    nan: 'не\u00A0число',
  ),
  'ru-UA': NumberSymbols(
    decimal: ',',
    group: '\u00A0',
    minimumGroupingDigits: 2,
    percentPattern: '#,##0\u00A0%',
    currencyPattern: '#,##0.00\u00A0¤',
    nan: 'не\u00A0число',
  ),
  'rw': NumberSymbols(decimal: ',', group: '.', currencyPattern: '¤\u00A0#,##0.00'),
  'rwk': NumberSymbols(decimal: '.', group: ',', currencyPattern: '#,##0.00¤'),
  'sa': NumberSymbols(
    decimal: '.',
    group: ',',
    digits: '०१२३४५६७८९',
    secondaryGroupingSize: 2,
    percentPattern: '#,##,##0%',
    currencyPattern: '¤#,##,##0.00',
  ),
  'sah': NumberSymbols(
    decimal: ',',
    group: '\u00A0',
    currencyPattern: '#,##0.00\u00A0¤',
    nan: 'чыыһыла\u00A0буотах',
  ),
  'sat': NumberSymbols(
    decimal: '.',
    group: ',',
    digits: '᱐᱑᱒᱓᱔᱕᱖᱗᱘᱙',
    currencyPattern: '¤\u00A0#,##0.00',
  ),
  'sat-Deva': NumberSymbols(
    decimal: '.',
    group: ',',
    digits: '०१२३४५६७८९',
    currencyPattern: '¤\u00A0#,##0.00',
  ),
  'sbp': NumberSymbols(decimal: '.', group: ',', currencyPattern: '#,##0.00¤'),
  'sc': NumberSymbols(decimal: ',', group: '.', currencyPattern: '#,##0.00\u00A0¤'),
  'scn': NumberSymbols(decimal: ',', group: '.', currencyPattern: '¤\u00A0#,##0.00'),
  'sd': NumberSymbols(
    decimal: '.',
    group: '٬',
    digits: '٠١٢٣٤٥٦٧٨٩',
    percentSign: '٪\u061C',
    minusSign: '\u061C-',
    currencyPattern: '#,##0.00\u00A0¤',
  ),
  'sd-Deva': NumberSymbols(
    decimal: '.',
    group: ',',
    currencyPattern: '¤\u00A0#,##0.00',
  ),
  'sdh': NumberSymbols(
    decimal: '٫',
    group: '٬',
    digits: '٠١٢٣٤٥٦٧٨٩',
    percentSign: '٪\u061C',
    minusSign: '\u061C-',
    currencyPattern: '#,##0.00\u00A0¤',
  ),
  'se': NumberSymbols(
    decimal: ',',
    group: '\u00A0',
    percentPattern: '#,##0\u00A0%',
    minusSign: '−',
    currencyPattern: '#,##0.00\u00A0¤',
  ),
  'seh': NumberSymbols(decimal: ',', group: '.', currencyPattern: '#,##0.00¤'),
  'ses': NumberSymbols(decimal: '.', group: '\u00A0', currencyPattern: '#,##0.00¤'),
  'sg': NumberSymbols(
    decimal: ',',
    group: '.',
    currencyPattern: '¤#,##0.00;¤-#,##0.00',
  ),
  'sgs': NumberSymbols(decimal: '.', group: ',', currencyPattern: '¤\u00A0#,##0.00'),
  'shi': NumberSymbols(decimal: ',', group: '\u00A0', currencyPattern: '#,##0.00¤'),
  'shn': NumberSymbols(decimal: '.', group: ',', currencyPattern: '¤\u00A0#,##0.00'),
  'sid': NumberSymbols(decimal: '.', group: ',', currencyPattern: '¤\u00A0#,##0.00'),
  'sk': NumberSymbols(
    decimal: ',',
    group: '\u00A0',
    percentPattern: '#,##0\u00A0%',
    currencyPattern: '#,##0.00\u00A0¤',
  ),
  'skr': NumberSymbols(decimal: '.', group: ',', currencyPattern: '¤\u00A0#,##0.00'),
  'sl': NumberSymbols(
    decimal: ',',
    group: '.',
    minimumGroupingDigits: 2,
    percentPattern: '#,##0\u00A0%',
    minusSign: '−',
    currencyPattern: '#,##0.00\u00A0¤',
  ),
  'sma': NumberSymbols(decimal: '.', group: ',', currencyPattern: '¤\u00A0#,##0.00'),
  'smj': NumberSymbols(decimal: '.', group: ',', currencyPattern: '¤\u00A0#,##0.00'),
  'smn': NumberSymbols(
    decimal: ',',
    group: '\u00A0',
    percentPattern: '#,##0\u00A0%',
    currencyPattern: '#,##0.00\u00A0¤',
    nan: 'epiloho',
  ),
  'sms': NumberSymbols(decimal: '.', group: ',', currencyPattern: '¤\u00A0#,##0.00'),
  'so': NumberSymbols(decimal: '.', group: ',', nan: 'MaL'),
  'sq': NumberSymbols(
    decimal: ',',
    group: '\u00A0',
    minimumGroupingDigits: 2,
    currencyPattern: '#,##0.00\u00A0¤',
  ),
  'sr': NumberSymbols(decimal: ',', group: '.', currencyPattern: '#,##0.00\u00A0¤'),
  'ss': NumberSymbols(decimal: ',', group: '\u00A0'),
  'ssy': NumberSymbols(decimal: '.', group: ',', currencyPattern: '¤\u00A0#,##0.00'),
  'su': NumberSymbols(decimal: ',', group: '.'),
  'suz': NumberSymbols(decimal: '.', group: ',', currencyPattern: '¤\u00A0#,##0.00'),
  'sv': NumberSymbols(
    decimal: ',',
    group: '\u00A0',
    percentPattern: '#,##0\u00A0%',
    minusSign: '−',
    currencyPattern: '#,##0.00\u00A0¤',
  ),
  'sw': NumberSymbols(decimal: '.', group: ',', currencyPattern: '¤\u00A0#,##0.00'),
  'sw-CD': NumberSymbols(decimal: ',', group: '.', currencyPattern: '¤\u00A0#,##0.00'),
  'syr': NumberSymbols(
    decimal: '.',
    group: ',',
    currencyPattern: '¤\u00A0#,##0.00',
    nan: 'ܠܝܬ\u00A0ܡܢܝܢܐ',
  ),
  'szl': NumberSymbols(
    decimal: ',',
    group: '\u00A0',
    currencyPattern: '#,##0.00\u00A0¤',
  ),
  'ta': NumberSymbols(
    decimal: '.',
    group: ',',
    secondaryGroupingSize: 2,
    percentPattern: '#,##,##0%',
    currencyPattern: '¤#,##,##0.00',
  ),
  'ta-MY': NumberSymbols(decimal: '.', group: ',', currencyPattern: '¤\u00A0#,##0.00'),
  'ta-SG': NumberSymbols(decimal: '.', group: ',', currencyPattern: '¤\u00A0#,##0.00'),
  'te': NumberSymbols(
    decimal: '.',
    group: ',',
    secondaryGroupingSize: 2,
    currencyPattern: '¤#,##,##0.00',
  ),
  'tg': NumberSymbols(
    decimal: ',',
    group: '\u00A0',
    currencyPattern: '#,##0.00\u00A0¤',
  ),
  'tig': NumberSymbols(decimal: '.', group: ',', currencyPattern: '¤\u00A0#,##0.00'),
  'tk': NumberSymbols(
    decimal: ',',
    group: '\u00A0',
    percentPattern: '#,##0\u00A0%',
    currencyPattern: '#,##0.00\u00A0¤',
    nan: 'san\u00A0däl',
  ),
  'tn': NumberSymbols(decimal: '.', group: '\''),
  'to': NumberSymbols(
    decimal: '.',
    group: ',',
    currencyPattern: '¤\u00A0#,##0.00',
    nan: 'TF',
  ),
  'tok': NumberSymbols(
    decimal: ',',
    group: '\u00A0',
    primaryGroupingSize: 2,
    secondaryGroupingSize: 2,
    currencyPattern: '¤#,#0.00',
  ),
  'tpi': NumberSymbols(decimal: '.', group: ',', currencyPattern: '#,##0.00\u00A0¤'),
  'tr': NumberSymbols(decimal: ',', group: '.', percentPattern: '%#,##0'),
  'trv': NumberSymbols(decimal: '.', group: ',', currencyPattern: '¤\u00A0#,##0.00'),
  'trw': NumberSymbols(decimal: '.', group: ',', currencyPattern: '¤\u00A0#,##0.00'),
  'ts': NumberSymbols(
    decimal: ',',
    group: '\u00A0',
    currencyPattern: '¤\u00A0#,##0.00',
  ),
  'tt': NumberSymbols(
    decimal: ',',
    group: '\u00A0',
    percentPattern: '#,##0\u00A0%',
    currencyPattern: '#,##0.00\u00A0¤',
  ),
  'twq': NumberSymbols(decimal: '.', group: '\u00A0', currencyPattern: '#,##0.00¤'),
  'tyv': NumberSymbols(
    decimal: '.',
    group: '\u00A0',
    currencyPattern: '¤\u00A0#,##0.00',
  ),
  'tzm': NumberSymbols(
    decimal: ',',
    group: '\u00A0',
    currencyPattern: '#,##0.00\u00A0¤',
  ),
  'uk': NumberSymbols(
    decimal: ',',
    group: '\u00A0',
    currencyPattern: '#,##0.00\u00A0¤',
  ),
  'und': NumberSymbols(decimal: '.', group: ',', currencyPattern: '¤\u00A0#,##0.00'),
  'ur': NumberSymbols(decimal: '.', group: ',', minusSign: '\u200E-'),
  'ur-IN': NumberSymbols(
    decimal: '٫',
    group: '٬',
    digits: '۰۱۲۳۴۵۶۷۸۹',
    minusSign: '\u200E-\u200E',
  ),
  'uz': NumberSymbols(
    decimal: ',',
    group: '\u00A0',
    currencyPattern: '#,##0.00\u00A0¤',
    nan: 'son\u00A0emas',
  ),
  'uz-Arab': NumberSymbols(
    decimal: '٫',
    group: '٬',
    digits: '۰۱۲۳۴۵۶۷۸۹',
    percentSign: '٪',
    minusSign: '\u200E-\u200E',
    currencyPattern: '¤\u00A0#,##0.00',
  ),
  'uz-Cyrl': NumberSymbols(
    decimal: ',',
    group: '\u00A0',
    currencyPattern: '#,##0.00\u00A0¤',
    nan: 'ҳақиқий\u00A0сон\u00A0эмас',
  ),
  've': NumberSymbols(decimal: ',', group: '\u00A0'),
  'vec': NumberSymbols(
    decimal: ',',
    group: '\u202F',
    currencyPattern: '#,##0.00\u202F¤',
  ),
  'vi': NumberSymbols(decimal: ',', group: '.', currencyPattern: '#,##0.00\u00A0¤'),
  'vmw': NumberSymbols(decimal: ',', group: '.', currencyPattern: '#,##0.00\u00A0¤'),
  'vo': NumberSymbols(decimal: '.', group: ',', currencyPattern: '¤\u00A0#,##0.00'),
  'wa': NumberSymbols(decimal: '.', group: ',', currencyPattern: '¤\u00A0#,##0.00'),
  'wae': NumberSymbols(decimal: ',', group: '\'', currencyPattern: '¤\u00A0#,##0.00'),
  'wal': NumberSymbols(decimal: '.', group: ',', currencyPattern: '¤\u00A0#,##0.00'),
  'wbp': NumberSymbols(decimal: '.', group: ',', currencyPattern: '¤\u00A0#,##0.00'),
  'wo': NumberSymbols(decimal: ',', group: '.', currencyPattern: '¤\u00A0#,##0.00'),
  'xh': NumberSymbols(decimal: '.', group: '\u00A0'),
  'xnr': NumberSymbols(
    decimal: '.',
    group: ',',
    secondaryGroupingSize: 2,
    percentPattern: '#,##,##0%',
    currencyPattern: '¤#,##,##0.00',
  ),
  'xog': NumberSymbols(decimal: '.', group: ',', currencyPattern: '#,##0.00\u00A0¤'),
  'yav': NumberSymbols(
    decimal: ',',
    group: '\u00A0',
    currencyPattern: '#,##0.00\u00A0¤',
  ),
  'yi': NumberSymbols(decimal: '.', group: ',', currencyPattern: '¤\u00A0#,##0.00'),
  'yrl': NumberSymbols(decimal: ',', group: '.', currencyPattern: '¤\u00A0#,##0.00'),
  'yue': NumberSymbols(decimal: '.', group: ',', nan: '非數值'),
  'yue-Hans': NumberSymbols(decimal: '.', group: ',', nan: '非数值'),
  'za': NumberSymbols(decimal: '.', group: ',', currencyPattern: '¤\u00A0#,##0.00'),
  'zgh': NumberSymbols(
    decimal: ',',
    group: '\u00A0',
    percentPattern: '#,##0\u00A0%',
    currencyPattern: '#,##0.00¤',
  ),
  'zh-Hant': NumberSymbols(decimal: '.', group: ',', nan: '非數值'),
  'zh-Hant-HK': NumberSymbols(decimal: '.', group: ',', nan: '非數值'),
  'zh-Hant-MO': NumberSymbols(decimal: '.', group: ',', nan: '非數值'),
  'zh-Hant-MY': NumberSymbols(decimal: '.', group: ',', nan: '非數值'),
  'zh-Latn': NumberSymbols(
    decimal: '.',
    group: ',',
    currencyPattern: '¤\u00A0#,##0.00',
  ),
};
