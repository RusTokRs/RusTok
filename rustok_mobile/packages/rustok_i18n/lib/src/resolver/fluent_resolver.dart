import '../ast/ast_nodes.dart';
import '../parser/ftl_parser.dart';
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

/// Formats [value] for [locale] with fraction digits, grouping and sign.
///
/// Digits beyond [minimumFractionDigits] are dropped when they are zero.
/// Separators come from [numberSymbolsFor]; unknown locales use `1,234.5`.
String formatFluentNumber(
  num value, {
  int minimumFractionDigits = 0,
  int maximumFractionDigits = 3,
  int minimumIntegerDigits = 1,
  bool useGrouping = true,
  String locale = 'en',
}) {
  if (value is double && !value.isFinite) return value.toString();

  final maximum = _clampDigits(maximumFractionDigits);
  final minimum = _clampDigits(minimumFractionDigits) > maximum
      ? maximum
      : _clampDigits(minimumFractionDigits);

  final text = value.abs().toStringAsFixed(maximum);
  final dot = text.indexOf('.');
  var integerPart = dot >= 0 ? text.substring(0, dot) : text;
  var fractionPart = dot >= 0 ? text.substring(dot + 1) : '';

  var fractionEnd = fractionPart.length;
  while (fractionEnd > minimum &&
      fractionPart.codeUnitAt(fractionEnd - 1) == 0x30) {
    fractionEnd--;
  }
  fractionPart = fractionPart.substring(0, fractionEnd);

  final isZero = !(integerPart + fractionPart).contains(RegExp(r'[1-9]'));
  final negative = value < 0 && !isZero;

  final minInt = _clampDigits(minimumIntegerDigits);
  if (integerPart.length < minInt) {
    integerPart = integerPart.padLeft(minInt, '0');
  }

  final symbols = numberSymbolsFor(locale);
  final grouped = useGrouping ? groupDigits(integerPart, symbols) : integerPart;

  final buffer = StringBuffer();
  if (negative) buffer.write('-');
  buffer.write(grouped);
  if (fractionPart.isNotEmpty) {
    buffer.write(symbols.decimal);
    buffer.write(fractionPart);
  }
  return buffer.toString();
}

int _clampDigits(int digits) {
  if (digits < 0) return 0;
  if (digits > 20) return 20;
  return digits;
}

/// Display form of a variable or number literal: integral doubles print without
/// a fraction (`3.0` -> `3`), everything else uses its default string form.
String _formatValue(Object value) {
  if (value is double && value.isFinite && value == value.truncateToDouble()) {
    return value.toStringAsFixed(0);
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

  /// Variables visible to the pattern being resolved. Messages receive none
  /// from their callers; terms only see their own named arguments.
  final Map<String, dynamic> args;
  final Set<String> _activeEntries;

  static const int _maxDepth = 100;

  String resolvePattern(FtlPattern pattern, {int depth = 0}) {
    if (depth > _maxDepth) return '{???}';

    final buffer = StringBuffer();
    for (final element in pattern.elements) {
      if (element is FtlTextElement) {
        buffer.write(element.value);
      } else if (element is FtlPlaceable) {
        final resolved = resolveExpression(element.expression, depth: depth + 1);
        if (bundle.useIsolating && !resolved.contains(bidiFsi)) {
          buffer.write('$bidiFsi$resolved$bidiPdi');
        } else {
          buffer.write(resolved);
        }
      }
    }
    return buffer.toString();
  }

  String resolveExpression(FtlExpression expr, {int depth = 0}) {
    if (depth > _maxDepth) return '{???}';

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

    // Messages referenced from a placeable do not inherit the caller's variables.
    _activeEntries.add(entryKey);
    try {
      return _FluentResolver(
        bundle: bundle,
        args: const <String, dynamic>{},
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
    final percent = _stringOption(expr.named['style']) == 'percent';
    final minimum = _digitsOption(expr.named['minimumFractionDigits'], 0);
    final maximumRaw = _digitsOption(
      expr.named['maximumFractionDigits'],
      percent ? 0 : 3,
    );
    final maximum = maximumRaw < minimum ? minimum : maximumRaw;
    final text = formatFluentNumber(
      percent ? number * 100 : number,
      minimumFractionDigits: minimum,
      maximumFractionDigits: maximum,
      minimumIntegerDigits:
          _digitsOption(expr.named['minimumIntegerDigits'], 1),
      useGrouping: _boolOption(expr.named['useGrouping']) ?? true,
      locale: bundle.locale,
    );
    return percent ? '$text%' : text;
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

  int _digitsOption(FtlExpression? option, int fallback) {
    if (option is FtlNumberLiteral) {
      return _clampDigits(option.value.toInt());
    }
    return fallback;
  }

  /// Raw (unformatted) value of an argument expression, used by built-in functions.
  Object? _rawValue(FtlExpression expr) {
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

    final numericValue = num.tryParse(cleanSelector);
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
