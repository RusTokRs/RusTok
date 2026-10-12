/// Locale-specific number symbols used by `NUMBER()` and `formatFluentNumber`.
///
/// This is a small, hand-maintained subset of CLDR data, so the package does not
/// need `intl`. Unknown languages fall back to `1,234.5` (English style).
class NumberSymbols {
  const NumberSymbols(
    this.decimal,
    this.group, {
    this.minimumGroupingDigits = 1,
  });

  /// Decimal separator.
  final String decimal;

  /// Digit-group separator.
  final String group;

  /// Smallest number of leading digits that is grouped. CLDR uses 2 for
  /// Polish and Spanish, so `1234` stays `1234` while `12345` becomes `12 345`.
  final int minimumGroupingDigits;
}

const String _nbsp = '\u00A0';
const String _narrowNbsp = '\u202F';

const NumberSymbols _dotDecimal = NumberSymbols('.', ',');
const NumberSymbols _commaDecimal = NumberSymbols(',', '.');
const NumberSymbols _commaDecimalNbsp = NumberSymbols(',', _nbsp);
const NumberSymbols _commaDecimalNarrowNbsp = NumberSymbols(',', _narrowNbsp);

/// Symbols for a BCP-47 [locale] (only the language subtag is used).
NumberSymbols numberSymbolsFor(String locale) {
  final language = locale.split(RegExp(r'[-_]')).first.toLowerCase();
  switch (language) {
    case 'de':
    case 'nl':
    case 'it':
    case 'id':
    case 'tr':
    case 'da':
    case 'el':
    case 'pt':
    case 'ro':
    case 'hr':
    case 'sr':
    case 'bs':
    case 'sl':
      return _commaDecimal;
    case 'es':
      return const NumberSymbols(',', '.', minimumGroupingDigits: 2);
    case 'pl':
      return const NumberSymbols(',', _nbsp, minimumGroupingDigits: 2);
    case 'ru':
    case 'uk':
    case 'be':
    case 'bg':
    case 'cs':
    case 'sk':
    case 'fi':
    case 'sv':
    case 'nb':
    case 'no':
    case 'lt':
    case 'lv':
      return _commaDecimalNbsp;
    case 'fr':
      return _commaDecimalNarrowNbsp;
    default:
      return _dotDecimal;
  }
}

/// Inserts [NumberSymbols.group] between groups of three digits.
///
/// [digits] must contain only ASCII digits.
String groupDigits(String digits, NumberSymbols symbols) {
  if (digits.length < 3 + symbols.minimumGroupingDigits) return digits;
  final buffer = StringBuffer();
  final head = digits.length % 3;
  if (head > 0) buffer.write(digits.substring(0, head));
  for (var i = head; i < digits.length; i += 3) {
    if (buffer.isNotEmpty) buffer.write(symbols.group);
    buffer.write(digits.substring(i, i + 3));
  }
  return buffer.toString();
}
