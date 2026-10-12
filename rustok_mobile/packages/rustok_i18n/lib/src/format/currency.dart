/// Currency data for `NUMBER(..., style: "currency")`.
///
/// A small hand-maintained subset of ISO 4217 and CLDR data. Unknown currencies
/// fall back to their ISO code as the symbol and to two fraction digits.

const Map<String, String> _currencySymbols = {
  'USD': r'$',
  'EUR': '€',
  'GBP': '£',
  'JPY': '¥',
  'CNY': 'CN¥',
  'KRW': '₩',
  'INR': '₹',
  'RUB': '₽',
  'BYN': 'Br',
  'UAH': '₴',
  'KZT': '₸',
  'PLN': 'zł',
  'CHF': 'CHF',
};

const Map<String, int> _currencyDigits = {
  'JPY': 0,
  'KRW': 0,
  'CLP': 0,
  'BHD': 3,
  'KWD': 3,
  'JOD': 3,
};

final RegExp _isoCodePattern = RegExp(r'^[A-Za-z]{3}$');
final RegExp _alphabeticPattern = RegExp(r'^[A-Za-z]+$');

/// Whether [code] has the shape of an ISO 4217 currency code (three letters).
bool isValidCurrencyCode(String code) => _isoCodePattern.hasMatch(code);

/// Symbol for an ISO 4217 [code], or the code itself when no symbol is known.
String currencySymbol(String code) {
  final upper = code.toUpperCase();
  return _currencySymbols[upper] ?? upper;
}

/// Standard number of fraction digits for an ISO 4217 [code].
int currencyFractionDigits(String code) => _currencyDigits[code.toUpperCase()] ?? 2;

/// Whether the currency symbol is written before the amount in [locale].
/// Other locales write it after the amount, separated by a no-break space.
bool currencySymbolPrecedes(String locale) {
  final language = locale.split(RegExp(r'[-_]')).first.toLowerCase();
  switch (language) {
    case 'en':
    case 'ja':
    case 'zh':
    case 'ko':
    case 'th':
    case 'hi':
      return true;
    default:
      return false;
  }
}

/// Whether [symbol] is letters only (for example `CHF`), which needs a space
/// before the amount in prefix position.
bool isAlphabeticSymbol(String symbol) => _alphabeticPattern.hasMatch(symbol);
