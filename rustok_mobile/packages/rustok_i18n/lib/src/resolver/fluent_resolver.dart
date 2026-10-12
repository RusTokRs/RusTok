import '../ast/ast_nodes.dart';
import '../parser/ftl_parser.dart';
import '../format/currency.dart';
import '../format/datetime_format.dart';
import '../format/number_symbols.dart';
import 'cldr_plurals.dart';

/// Unicode BiDi isolation characters.
const String bidiFsi = '\u2068'; // First Strong Isolate
const String bidiPdi = '\u2069'; // Pop Directional Isolate

/// A Project Fluent bundle representing messages and terms for a single locale.
class FluentBundle {
  FluentBundle({
    required this.locale,
    this.useIsolating = true,
  });

  final String locale;
  final bool useIsolating;

  final Map<String, FtlMessage> _messages = {};
  final Map<String, FtlTerm> _terms = {};
  final List<String> _diagnostics = [];

  /// Problems found while loading resources: junk entries and duplicate IDs.
  ///
  /// Empty when the loaded sources are fully valid.
  List<String> get diagnostics => List.unmodifiable(_diagnostics);

  /// Adds parsed entries. When an ID is defined more than once, the first
  /// definition is kept and the duplicate is reported in [diagnostics].
  void addResource(FtlResource resource) {
    for (final entry in resource.entries) {
      if (entry is FtlMessage) {
        if (_messages.containsKey(entry.id)) {
          _diagnostics.add('Duplicate message "${entry.id}" ignored; first definition wins');
        } else {
          _messages[entry.id] = entry;
        }
      } else if (entry is FtlTerm) {
        if (_terms.containsKey(entry.id)) {
          _diagnostics.add('Duplicate term "-${entry.id}" ignored; first definition wins');
        } else {
          _terms[entry.id] = entry;
        }
      } else if (entry is FtlJunk) {
        final reason = entry.message ?? 'unparseable entry';
        final firstLine = entry.content.split('\n').first;
        _diagnostics.add('Junk ($reason): $firstLine');
      }
    }
  }

  void addSource(String source) {
    addResource(FtlParser.parse(source));
  }

  bool hasMessage(String id) => _messages.containsKey(id);

  bool hasTerm(String id) => _terms.containsKey(id);

  FtlMessage? getMessage(String id) => _messages[id];

  FtlTerm? getTerm(String id) => _terms[id];

  Iterable<String> get messageIds => _messages.keys;

  /// Formats a message by its ID, with optional arguments and attribute name.
  String? format(
    String id, {
    Map<String, dynamic>? args,
    String? attribute,
  }) {
    final message = _messages[id];
    if (message == null) return null;

    final FtlPattern? pattern = attribute != null
        ? message.findAttribute(attribute)?.value
        : message.value;
    if (pattern == null) return null;

    final suffix = attribute != null ? '.$attribute' : '';
    final resolver = _FluentResolver(
      bundle: this,
      args: args ?? const <String, dynamic>{},
      activeEntries: <String>{'msg:$id$suffix'},
    );
    return resolver.resolvePattern(pattern);
  }

  /// Formats a term by its ID (without leading '-').
  String? formatTerm(
    String id, {
    Map<String, dynamic>? args,
    String? attribute,
  }) {
    final term = _terms[id];
    if (term == null) return null;

    final FtlPattern? pattern = attribute != null
        ? term.findAttribute(attribute)?.value
        : term.value;
    if (pattern == null) return null;

    final suffix = attribute != null ? '.$attribute' : '';
    final resolver = _FluentResolver(
      bundle: this,
      args: args ?? const <String, dynamic>{},
      activeEntries: <String>{'term:$id$suffix'},
    );
    return resolver.resolvePattern(pattern);
  }
}

/// Formats [value] the way `NUMBER()` and `fluent-bundle` do: with the locale
/// separators, digit grouping and fraction digits of [numberSymbolsFor].
///
/// Rounding follows `Intl.NumberFormat`: the value is written in its shortest
/// decimal form (Dart's `toString` for doubles) and rounded half away from zero
/// on those digits, so `2.675` with `maximumFractionDigits: 2` is `2.68` and
/// `999999.9999` becomes `1,000,000`. Trailing zeros are dropped down to
/// [minimumFractionDigits], an integer keeps full precision, `-0.0` keeps its
/// sign (`-0`), and values whose shortest form uses an exponent (`1e+21`,
/// `1e-7`) are written out in full before grouping. Non-finite values print the
/// locale's `NaN` and infinity symbols.
///
/// [pattern] is a CLDR number pattern such as `#,##0 %` (see
/// [NumberSymbols.percentPattern]). It supplies the literal text around the
/// number and the sign layout of its optional `;`-separated negative
/// subpattern; digits, separators and grouping sizes always come from the
/// locale symbols. When [pattern] is omitted the number is printed without
/// affixes.
String formatFluentNumber(
  num value, {
  int minimumFractionDigits = 0,
  int maximumFractionDigits = 3,
  int minimumIntegerDigits = 1,
  bool useGrouping = true,
  String locale = 'en',
  String? pattern,
}) {
  final symbols = numberSymbolsFor(locale);
  final minimum = _clampDigits(minimumFractionDigits);
  var maximum = _clampDigits(maximumFractionDigits);
  // `Intl.NumberFormat` raises the maximum when only a minimum is given, so
  // `%` with `minimumFractionDigits: 2` prints `35.00%`, not `35%`.
  if (maximum < minimum) maximum = minimum;

  if (value is double && !value.isFinite) {
    if (value.isNaN) return symbols.nan;
    return '${symbols.minusSign}${symbols.infinity}';
  }

  final negative = value.isNegative;
  final shortest = _shortestDecimalParts(value);
  var integerPart = shortest.$1;
  var fractionPart = shortest.$2;

  final rounded = _roundFractionPart(fractionPart, maximum);
  fractionPart = rounded.$1;
  if (rounded.$2) integerPart = _incrementDigits(integerPart);

  var fractionEnd = fractionPart.length;
  while (fractionEnd > minimum && fractionPart.codeUnitAt(fractionEnd - 1) == 0x30) {
    fractionEnd--;
  }
  fractionPart = fractionPart.substring(0, fractionEnd).padRight(minimum, '0');

  final minInt = _clampDigits(minimumIntegerDigits);
  if (integerPart.length < minInt) {
    integerPart = integerPart.padLeft(minInt, '0');
  }

  final body = StringBuffer();
  body.write(useGrouping ? groupDigits(integerPart, symbols) : integerPart);
  if (fractionPart.isNotEmpty) {
    body.write(symbols.decimal);
    body.write(fractionPart);
  }
  final text = localizeDigits(body.toString(), symbols);

  if (pattern == null) {
    return '${negative ? symbols.minusSign : ''}$text';
  }
  final affixes = _patternAffixes(
    pattern.replaceAll('%', symbols.percentSign),
    symbols,
    negative: negative,
  );
  final sign = negative && !affixes.$3 ? symbols.minusSign : '';
  return '$sign${affixes.$1}$text${affixes.$2}';
}

/// Splits a number pattern into the literal text before and after the number.
///
/// The pattern's first subpattern is used for positive values and the optional
/// second one — after a `;` — for negative values, so `% #,#0;% -#,#0` (Welsh
/// `blo`) puts the sign after the percent sign and `1.234,50-` (Faroese) puts
/// it at the very end. A `-` written in a pattern becomes the locale's
/// [NumberSymbols.minusSign].
///
/// [placeholder] is a pattern character that stands for text supplied by the
/// caller: the currency patterns use CLDR's `\u00a4` for the currency symbol,
/// which is replaced with [replacement] after the layout characters are gone.
///
/// The result is `(prefix, suffix, patternCarriesSign)`, where the last element
/// tells whether the sign came from the pattern itself, in which case the
/// caller must not add a sign of its own.
(String, String, bool) _patternAffixes(
  String pattern,
  NumberSymbols symbols, {
  required bool negative,
  String placeholder = '',
  String replacement = '',
}) {
  final parts = pattern.split(';');
  final useNegative = negative && parts.length > 1;
  final chosen = useNegative ? parts[1] : parts[0];
  var firstDigit = -1;
  for (var i = 0; i < chosen.length; i++) {
    final unit = chosen.codeUnitAt(i);
    if (unit == 0x30 || unit == 0x23) {
      // '0' or '#'
      firstDigit = i;
      break;
    }
  }
  if (firstDigit < 0) return ('', '', useNegative);

  final placeholderUnit = placeholder.isEmpty ? -1 : placeholder.codeUnitAt(0);
  String literal(String text) {
    final buffer = StringBuffer();
    for (final unit in text.codeUnits) {
      if (unit == 0x30 || unit == 0x23 || unit == 0x2C || unit == 0x2E) {
        continue; // 0 # , .
      }
      if (unit == 0x2D) {
        buffer.write(symbols.minusSign);
      } else if (unit == placeholderUnit) {
        buffer.write(replacement);
      } else {
        buffer.writeCharCode(unit);
      }
    }
    return buffer.toString();
  }

  return (
    literal(chosen.substring(0, firstDigit)),
    literal(chosen.substring(firstDigit)),
    useNegative,
  );
}

/// Integer and fraction digits of the shortest decimal form of [value], without
/// its sign, with exponential forms expanded into plain digits.
(String, String) _shortestDecimalParts(num value) {
  final text = value.abs().toString();
  final exponentIndex = text.indexOf('e');
  if (exponentIndex < 0) {
    final dot = text.indexOf('.');
    if (dot < 0) return (text, '');
    return (text.substring(0, dot), text.substring(dot + 1));
  }
  final mantissa = text.substring(0, exponentIndex);
  final exponent = int.parse(text.substring(exponentIndex + 1));
  final dot = mantissa.indexOf('.');
  final digits = mantissa.replaceAll('.', '');
  final point = (dot < 0 ? mantissa.length : dot) + exponent;
  if (point <= 0) return ('0', '${'0' * -point}$digits');
  if (point >= digits.length) {
    return ('$digits${'0' * (point - digits.length)}', '');
  }
  return (digits.substring(0, point), digits.substring(point));
}

/// Rounds [fraction] to [maximum] digits, half away from zero. The flag in the
/// result asks the caller to carry into the integer part.
(String, bool) _roundFractionPart(String fraction, int maximum) {
  if (fraction.length <= maximum) return (fraction, false);
  final kept = fraction.substring(0, maximum).codeUnits.toList();
  if (fraction.codeUnitAt(maximum) < 0x35) {
    // below '5'
    return (String.fromCharCodes(kept), false);
  }
  for (var i = kept.length - 1; i >= 0; i--) {
    if (kept[i] != 0x39) {
      // not '9'
      kept[i]++;
      return (String.fromCharCodes(kept), false);
    }
    kept[i] = 0x30;
  }
  return (String.fromCharCodes(kept), true);
}

/// Adds one to a non-negative digit string, growing it when every digit was 9.
String _incrementDigits(String digits) {
  final units = digits.codeUnits.toList();
  for (var i = units.length - 1; i >= 0; i--) {
    if (units[i] != 0x39) {
      units[i]++;
      return String.fromCharCodes(units);
    }
    units[i] = 0x30;
  }
  return '1${String.fromCharCodes(units)}';
}

int _clampDigits(int digits) {
  if (digits < 0) return 0;
  if (digits > 20) return 20;
  return digits;
}

/// Formats [value] as money in [currency] (ISO 4217 code) for [locale].
///
/// The layout comes from the locale's CLDR currency pattern — `¤#,##0.00` for
/// English (`$1,234.50`), `#,##0.00\u00A0¤` for Russian (`1\u00A0234,50\u00A0€`) — and
/// the fraction digits from the currency itself (0 for JPY, 3 for KWD) unless
/// [minimumFractionDigits] or [maximumFractionDigits] say otherwise.
///
/// [display] is `symbol` (the default) or `code` for the ISO code; any other
/// value falls back to the symbol. An alphabetic symbol is separated from the
/// amount by a no-break space, as CLDR `currencySpacing` requires, so `CHF` is
/// `CHF 3.00` while `$` stays `$3.00`.
String formatFluentCurrency(
  num value, {
  required String currency,
  required String locale,
  String display = 'symbol',
  int? minimumFractionDigits,
  int? maximumFractionDigits,
  bool useGrouping = true,
}) {
  final code = currency.toUpperCase();
  final symbols = numberSymbolsFor(locale);
  final defaults = currencyFractionDigits(code);
  final minimum = minimumFractionDigits ?? defaults;
  var maximum =
      maximumFractionDigits ?? (defaults > minimum ? defaults : minimum);
  if (maximum < minimum) maximum = minimum;

  final currencyText = display == 'code' ? code : currencySymbol(code);
  final affixes = _patternAffixes(
    symbols.currencyPattern,
    symbols,
    negative: value.isNegative,
    placeholder: '¤',
    replacement: currencyText,
  );
  var prefix = affixes.$1;
  var suffix = affixes.$2;
  if (isAlphabeticSymbol(currencyText)) {
    if (prefix == currencyText) prefix = '$currencyText\u00A0';
    if (suffix == currencyText) suffix = '\u00A0$currencyText';
  }

  // The sign is only added when the pattern did not place one itself.
  final sign = value.isNegative && !affixes.$3 ? symbols.minusSign : '';
  final amount = formatFluentNumber(
    value.abs(),
    minimumFractionDigits: minimum,
    maximumFractionDigits: maximum,
    useGrouping: useGrouping,
    locale: locale,
  );
  return '$sign$prefix$amount$suffix';
}

/// Display form of a variable, a number literal or a term argument.
///
/// Integral doubles print without a fraction (`3.0` -> `3`), and exponential
/// forms are written out in full (`1e+21`, `1e-7`), which is what the reference
/// runtime does for a bare number (`to_string` in `fluent-bundle`). No grouping
/// and no locale symbols are applied: a bare `{$n}` is not a `NUMBER()` call.
String _formatValue(Object value) {
  if (value is int) return value.toString();
  if (value is double) {
    if (value.isNaN) return 'NaN';
    if (value.isInfinite) return value.isNegative ? '-Infinity' : 'Infinity';
    final parts = _shortestDecimalParts(value);
    final fraction =
        parts.$2.contains(RegExp(r'[1-9]')) ? '.${parts.$2}' : '';
    return '${value.isNegative ? '-' : ''}${parts.$1}$fraction';
  }
  return value.toString();
}

class _FluentResolver {
  _FluentResolver({
    required this.bundle,
    required this.args,
    required Set<String> activeEntries,
  }) : _activeEntries = activeEntries;

  final FluentBundle bundle;

  /// Variables visible to the pattern being resolved. A referenced message
  /// shares the caller's scope, while a term only sees its own named arguments.
  final Map<String, dynamic> args;
  final Set<String> _activeEntries;

  static const int _maxDepth = 100;

  String resolvePattern(FtlPattern pattern, {int depth = 0}) {
    if (depth > _maxDepth) return '{???}';

    // Isolation follows `fluent-bundle` (the Rust runtime this package mirrors):
    // a pattern with more than one element isolates every placeable except a
    // message reference, a term reference and a string literal.
    final isolate = bundle.useIsolating && pattern.elements.length > 1;

    final buffer = StringBuffer();
    for (final element in pattern.elements) {
      if (element is FtlTextElement) {
        buffer.write(element.value);
      } else if (element is FtlPlaceable) {
        final resolved = resolveExpression(element.expression, depth: depth + 1);
        if (isolate && _needsIsolation(element.expression)) {
          buffer.write('$bidiFsi$resolved$bidiPdi');
        } else {
          buffer.write(resolved);
        }
      }
    }
    return buffer.toString();
  }

  static bool _needsIsolation(FtlExpression expression) =>
      expression is! FtlMessageReference &&
      expression is! FtlTermReference &&
      expression is! FtlStringLiteral;

  String resolveExpression(FtlExpression expr, {int depth = 0}) {
    if (depth > _maxDepth) return '{???}';

    if (expr is FtlPlaceableExpression) {
      // A nested placeable is transparent: it resolves to the value of the
      // expression it wraps.
      return resolveExpression(expr.expression, depth: depth + 1);
    }

    if (expr is FtlStringLiteral) {
      return expr.value;
    }

    if (expr is FtlNumberLiteral) {
      return _formatValue(expr.value);
    }

    if (expr is FtlVariableReference) {
      final val = args[expr.id];
      if (val == null) return '{\$${expr.id}}';
      return _formatValue(val as Object);
    }

    if (expr is FtlTermReference) {
      return _resolveTerm(expr, depth: depth);
    }

    if (expr is FtlMessageReference) {
      return _resolveMessage(expr, depth: depth);
    }

    if (expr is FtlFunctionReference) {
      return _resolveFunction(expr, depth: depth);
    }

    if (expr is FtlSelectExpression) {
      return _resolveSelect(expr, depth: depth + 1);
    }

    return '{???}';
  }

  String _resolveTerm(FtlTermReference expr, {required int depth}) {
    final suffix = expr.attribute != null ? '.${expr.attribute}' : '';
    final entryKey = 'term:${expr.id}$suffix';
    if (_activeEntries.contains(entryKey)) return '{???}';

    final term = bundle.getTerm(expr.id);
    if (term == null) return '{-${expr.id}}';
    final pattern = expr.attribute != null
        ? term.findAttribute(expr.attribute!)?.value
        : term.value;
    if (pattern == null) return '{-${expr.id}$suffix}';

    // Terms get their own scope: only their named arguments are visible.
    // Positional arguments (`{-term(1)}`) are accepted by the syntax but never
    // passed to the term, matching `fluent-bundle`.
    final termArgs = <String, dynamic>{};
    final namedArgs = expr.arguments;
    if (namedArgs != null) {
      namedArgs.forEach((name, value) {
        termArgs[name] = _literalValue(value);
      });
    }

    _activeEntries.add(entryKey);
    try {
      return _FluentResolver(
        bundle: bundle,
        args: termArgs,
        activeEntries: _activeEntries,
      ).resolvePattern(pattern, depth: depth + 1);
    } finally {
      _activeEntries.remove(entryKey);
    }
  }

  String _resolveMessage(FtlMessageReference expr, {required int depth}) {
    final suffix = expr.attribute != null ? '.${expr.attribute}' : '';
    final entryKey = 'msg:${expr.id}$suffix';
    if (_activeEntries.contains(entryKey)) return '{???}';

    final message = bundle.getMessage(expr.id);
    if (message == null) return '{${expr.id}}';
    final pattern = expr.attribute != null
        ? message.findAttribute(expr.attribute!)?.value
        : message.value;
    if (pattern == null) return '{${expr.id}$suffix}';

    // A message reference is resolved in the caller's scope: the variables of
    // the enclosing pattern stay visible, exactly as in the reference
    // `FluentBundle` resolvers. (Terms, by contrast, get their own scope.)
    _activeEntries.add(entryKey);
    try {
      return _FluentResolver(
        bundle: bundle,
        args: args,
        activeEntries: _activeEntries,
      ).resolvePattern(pattern, depth: depth + 1);
    } finally {
      _activeEntries.remove(entryKey);
    }
  }

  String _resolveFunction(FtlFunctionReference expr, {required int depth}) {
    if (expr.name == 'NUMBER' && expr.positional.isNotEmpty) {
      final number = _numericValue(_rawValue(expr.positional.first));
      if (number != null) return _formatNumberFunction(expr, number);
    }
    if (expr.name == 'DATETIME' && expr.positional.isNotEmpty) {
      final date = _dateValue(_rawValue(expr.positional.first));
      if (date != null) return _formatDateFunction(expr, date);
    }

    // Unknown or non-applicable calls fall back to the first positional argument.
    if (expr.positional.isEmpty) return '{${expr.name}()}';
    return resolveExpression(expr.positional.first, depth: depth + 1);
  }

  String _formatNumberFunction(FtlFunctionReference expr, num number) {
    final style = _stringOption(expr.named['style']);
    final code = _stringOption(expr.named['currency']);
    if (style == 'currency' && code != null && isValidCurrencyCode(code)) {
      return formatFluentCurrency(
        number,
        currency: code,
        locale: bundle.locale,
        display: _stringOption(expr.named['currencyDisplay']) ?? 'symbol',
        minimumFractionDigits:
            _optionalDigits(expr.named['minimumFractionDigits']),
        maximumFractionDigits:
            _optionalDigits(expr.named['maximumFractionDigits']),
        useGrouping: _boolOption(expr.named['useGrouping']) ?? true,
      );
    }

    final percent = style == 'percent';
    final symbols = numberSymbolsFor(bundle.locale);
    // `NUMBER()` without options formats with the locale's decimal pattern; the
    // percent style uses the percent pattern, which carries the sign layout and
    // the default fraction digits (`#,##0 %`, `%#,##0`, `#,##,##0%`).
    final defaults = percent
        ? patternFractionDigits(symbols.percentPattern)
        : (symbols.minimumFractionDigits, symbols.maximumFractionDigits);
    final minimum =
        _digitsOption(expr.named['minimumFractionDigits'], defaults.$1);
    final maximumRaw =
        _digitsOption(expr.named['maximumFractionDigits'], defaults.$2);
    final maximum = maximumRaw < minimum ? minimum : maximumRaw;
    return formatFluentNumber(
      percent ? number * 100 : number,
      minimumFractionDigits: minimum,
      maximumFractionDigits: maximum,
      minimumIntegerDigits:
          _digitsOption(expr.named['minimumIntegerDigits'], 1),
      useGrouping: _boolOption(expr.named['useGrouping']) ?? true,
      locale: bundle.locale,
      pattern: percent ? symbols.percentPattern : null,
    );
  }

  String _formatDateFunction(FtlFunctionReference expr, DateTime date) {
    final value = _stringOption(expr.named['timeZone']) == 'UTC'
        ? date.toUtc()
        : date;
    return formatFluentDateTime(
      value,
      locale: bundle.locale,
      dateStyle: _stringOption(expr.named['dateStyle']),
      timeStyle: _stringOption(expr.named['timeStyle']),
      weekday: _stringOption(expr.named['weekday']),
      year: _stringOption(expr.named['year']),
      month: _stringOption(expr.named['month']),
      day: _stringOption(expr.named['day']),
      hour: _stringOption(expr.named['hour']),
      minute: _stringOption(expr.named['minute']),
      second: _stringOption(expr.named['second']),
      hour12: _boolOption(expr.named['hour12']),
    );
  }

  /// Numeric value of a raw argument: numbers, or strings holding a number.
  num? _numericValue(Object? raw) {
    if (raw is num) return raw;
    if (raw is String) return num.tryParse(raw.trim());
    return null;
  }

  /// Date value of a raw argument: a [DateTime], an ISO-8601 string, or epoch
  /// milliseconds (interpreted as UTC).
  DateTime? _dateValue(Object? raw) {
    if (raw is DateTime) return raw;
    if (raw is num) {
      return DateTime.fromMillisecondsSinceEpoch(raw.toInt(), isUtc: true);
    }
    if (raw is String) return DateTime.tryParse(raw.trim());
    return null;
  }

  String? _stringOption(FtlExpression? option) {
    return option is FtlStringLiteral ? option.value : null;
  }

  /// Boolean option written as the string `"true"`/`"false"` or as a number.
  bool? _boolOption(FtlExpression? option) {
    if (option is FtlStringLiteral) {
      if (option.value == 'true') return true;
      if (option.value == 'false') return false;
    }
    if (option is FtlNumberLiteral) return option.value != 0;
    return null;
  }

  int? _optionalDigits(FtlExpression? option) {
    return option is FtlNumberLiteral ? _clampDigits(option.value.toInt()) : null;
  }

  int _digitsOption(FtlExpression? option, int fallback) {
    if (option is FtlNumberLiteral) {
      return _clampDigits(option.value.toInt());
    }
    return fallback;
  }

  /// Raw (unformatted) value of an argument expression, used by built-in functions.
  Object? _rawValue(FtlExpression expr) {
    if (expr is FtlPlaceableExpression) return _rawValue(expr.expression);
    if (expr is FtlNumberLiteral) return expr.value;
    if (expr is FtlStringLiteral) return expr.value;
    if (expr is FtlVariableReference) return args[expr.id] as Object?;
    return null;
  }

  Object _literalValue(FtlExpression expr) {
    if (expr is FtlNumberLiteral) return expr.value;
    if (expr is FtlStringLiteral) return expr.value;
    return '';
  }

  /// Numeric value of a selector that is a variable or `NUMBER()` call, if any.
  num? _selectorNumber(FtlExpression selector) {
    if (selector is FtlVariableReference) {
      return _numericValue(args[selector.id]);
    }
    if (selector is FtlFunctionReference &&
        selector.name == 'NUMBER' &&
        selector.positional.isNotEmpty) {
      return _numericValue(_rawValue(selector.positional.first));
    }
    return null;
  }

  String _resolveSelect(FtlSelectExpression expr, {required int depth}) {
    final selectorValue = resolveExpression(expr.selector, depth: depth);
    final cleanSelector =
        selectorValue.replaceAll(bidiFsi, '').replaceAll(bidiPdi, '').trim();

    // 1. Exact match on non-numeric keys (case-sensitive, as the spec requires).
    for (final variant in expr.variants) {
      if (variant.key == cleanSelector) {
        return resolvePattern(variant.value, depth: depth + 1);
      }
    }

    // Plural selection uses the raw number, not the display string: `1,234`
    // from NUMBER() must still select by its numeric value.
    final numericValue = _selectorNumber(expr.selector) ?? num.tryParse(cleanSelector);
    if (numericValue != null) {
      // 2a. Exact numeric key (e.g. [0]) takes precedence over plural categories.
      for (final variant in expr.variants) {
        final variantNum = num.tryParse(variant.key);
        if (variantNum != null && variantNum == numericValue) {
          return resolvePattern(variant.value, depth: depth + 1);
        }
      }

      // 2b. CLDR plural category (e.g. [one], [few], [many]).
      final category = resolveCldrPluralCategory(bundle.locale, numericValue);
      for (final variant in expr.variants) {
        if (variant.key == category) {
          return resolvePattern(variant.value, depth: depth + 1);
        }
      }
    }

    // 3. Default variant.
    return resolvePattern(expr.defaultVariant.value, depth: depth + 1);
  }
}
