/// Currency data for `NUMBER(..., style: "currency")`.
///
/// Two tables taken from CLDR 48: the symbols of the currencies a message is
/// most likely to format, and the fraction digits of every currency CLDR gives
/// a count other than two (0 for JPY and ISK, 3 for KWD, 4 for CLF).
///
/// A currency without a symbol here falls back to its ISO code, which is also
/// what ICU does for a currency the locale has no data for; the layout around
/// the symbol - before or after the amount, and the no-break space between
/// them - comes from the locale's CLDR currency pattern in
/// [NumberSymbols.currencyPattern].

/// Currency symbols, written as CLDR writes the currency in its own locale:
/// `₽` in Russian, `zł` in Polish, `฿` in Thai, `￥` in Japanese, `лв.` in
/// Bulgarian.
///
/// The table is deliberately not locale-dependent: CLDR gives a locale its own
/// symbol for foreign money (`US$` in Armenian, `JP¥` in Basque, `د.إ.` in
/// Arabic), and a table of four hundred locales by three hundred currencies is
/// out of scope for a package that ships no CLDR dependency. A currency that is
/// not here is shown as its ISO code, as ICU does for a currency the locale has
/// no data for.
const Map<String, String> _currencySymbols = {
  'USD': '\$',   'EUR': '€',   'GBP': '£',   'JPY': '￥',   'CNY': '¥',
  'KRW': '₩',   'INR': '₹',   'RUB': '₽',   'UAH': '₴',   'BYN': 'Br',
  'KZT': '₸',   'PLN': 'zł',   'CZK': 'Kč',   'HUF': 'Ft',   'RON': 'RON',
  'BGN': 'лв.',   'TRY': '₺',   'SEK': 'kr',   'NOK': 'kr',   'DKK': 'kr.',
  'ISK': 'kr.',   'CHF': 'CHF',   'THB': '฿',   'VND': '₫',   'IDR': 'Rp',
  'MYR': 'RM',   'SGD': '\$',   'PHP': '₱',   'HKD': 'HK\$',   'TWD': '\$',
  'BRL': 'R\$',   'MXN': '\$',   'ARS': '\$',   'CLP': '\$',   'COP': '\$',
  'PEN': 'S/',   'ZAR': 'R',   'EGP': 'ج.م.\u200F',   'SAR': 'ر.س.\u200F',   'AED': 'د.إ.\u200F',
  'ILS': '₪',   'AUD': '\$',   'CAD': '\$',   'NZD': '\$',   'UZS': 'soʻm',
  'AZN': '₼',   'GEL': '₾',   'AMD': '֏',   'KGS': 'сом',   'TJS': 'сом.',
  'MDL': 'MDL',   'TMT': 'TMT',   'NGN': '₦',   'KES': 'Ksh',   'PKR': 'Rs',
  'BDT': '৳',   'LKR': 'රු.',   'NPR': 'नेरू',   'MAD': 'د.م.\u200F',   'TND': 'د.ت.\u200F',
  'IQD': 'د.ع.\u200F',   'XAF': 'FCFA',   'XOF': 'F\u202FCFA',   'XPF': 'FCFP',
};

/// Fraction digits of every ISO 4217 code whose count is not the usual two
/// (CLDR `supplemental/currencyData.json`).
const Map<String, int> _currencyDigits = {
  'ADP': 0,   'AFN': 0,   'ALL': 0,   'BHD': 3,   'BIF': 0,   'BYR': 0,   'CLF': 4,
  'CLP': 0,   'COP': 0,   'DJF': 0,   'ESP': 0,   'GNF': 0,   'HUF': 0,   'IDR': 0,
  'IQD': 0,   'IRR': 0,   'ISK': 0,   'ITL': 0,   'JOD': 3,   'JPY': 0,   'KMF': 0,
  'KPW': 0,   'KRW': 0,   'KWD': 3,   'LAK': 0,   'LBP': 0,   'LUF': 0,   'LYD': 3,
  'MGA': 0,   'MGF': 0,   'MMK': 0,   'MRO': 0,   'OMR': 3,   'PKR': 0,   'PYG': 0,
  'RWF': 0,   'SLL': 0,   'SOS': 0,   'STD': 0,   'SYP': 0,   'TMM': 0,   'TND': 3,
  'TRL': 0,   'UGX': 0,   'UYI': 0,   'UYW': 4,   'VND': 0,   'VUV': 0,   'XAF': 0,
  'XOF': 0,   'XPF': 0,   'YER': 0,   'ZMK': 0,   'ZWD': 0,
};

final RegExp _isoCodePattern = RegExp(r'^[A-Za-z]{3}$');
final RegExp _alphabeticPattern = RegExp(r'^\p{L}+$', unicode: true);

/// Whether [code] has the shape of an ISO 4217 currency code (three letters).
bool isValidCurrencyCode(String code) => _isoCodePattern.hasMatch(code);

/// Symbol for an ISO 4217 [code], or the code itself when no symbol is known.
String currencySymbol(String code) {
  final upper = code.toUpperCase();
  return _currencySymbols[upper] ?? upper;
}

/// Standard number of fraction digits for an ISO 4217 [code]: 2 unless CLDR
/// says otherwise (0 for `JPY`, `ISK` and `VND`, 3 for `KWD`, 4 for `CLF`).
int currencyFractionDigits(String code) =>
    _currencyDigits[code.toUpperCase()] ?? 2;

/// Whether [symbol] is letters only (for example `CHF` or `USD`).
///
/// Recognizes Unicode letters, not just ASCII (`Kč`, `zł`). Currency layout
/// uses the boundary-based CLDR spacing rule, not this whole-string predicate.
bool isAlphabeticSymbol(String symbol) => _alphabeticPattern.hasMatch(symbol);
