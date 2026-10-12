import '../ast/ast_nodes.dart';
import '../parser/ftl_parser.dart';
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

  void addResource(FtlResource resource) {
    for (final entry in resource.entries) {
      if (entry is FtlMessage) {
        _messages[entry.id] = entry;
      } else if (entry is FtlTerm) {
        _terms[entry.id] = entry;
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

    FtlPattern? pattern;
    if (attribute != null) {
      final attr = message.findAttribute(attribute);
      if (attr == null) return null;
      pattern = attr.value;
    } else {
      pattern = message.value;
    }

    if (pattern == null) return null;

    final entryKey = 'msg:$id${attribute != null ? ".$attribute" : ""}';
    final resolver = _FluentResolver(
      bundle: this,
      args: args ?? const {},
      activeEntries: {entryKey},
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

    FtlPattern? pattern;
    if (attribute != null) {
      final attr = term.findAttribute(attribute);
      if (attr == null) return null;
      pattern = attr.value;
    } else {
      pattern = term.value;
    }

    final entryKey = 'term:$id${attribute != null ? ".$attribute" : ""}';
    final resolver = _FluentResolver(
      bundle: this,
      args: args ?? const {},
      activeEntries: {entryKey},
    );
    return resolver.resolvePattern(pattern);
  }
}

class _FluentResolver {
  _FluentResolver({
    required this.bundle,
    required this.args,
    Set<String>? activeEntries,
  }) : _activeEntries = activeEntries ?? <String>{};

  final FluentBundle bundle;
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
      return expr.value.toString();
    }

    if (expr is FtlVariableReference) {
      final val = args[expr.id];
      if (val == null) return '{\$${expr.id}}';
      return val.toString();
    }

    if (expr is FtlTermReference) {
      final entryKey = 'term:${expr.id}${expr.attribute != null ? ".${expr.attribute}" : ""}';
      if (_activeEntries.contains(entryKey) || depth > _maxDepth) {
        return '{???}';
      }
      _activeEntries.add(entryKey);
      try {
        final term = bundle.getTerm(expr.id);
        if (term == null) return '{-${expr.id}}';
        final pattern = expr.attribute != null
            ? term.findAttribute(expr.attribute!)?.value
            : term.value;
        return resolvePattern(pattern, depth: depth + 1);
      } finally {
        _activeEntries.remove(entryKey);
      }
    }

    if (expr is FtlMessageReference) {
      final entryKey = 'msg:${expr.id}${expr.attribute != null ? ".${expr.attribute}" : ""}';
      if (_activeEntries.contains(entryKey) || depth > _maxDepth) {
        return '{???}';
      }
      _activeEntries.add(entryKey);
      try {
        final message = bundle.getMessage(expr.id);
        if (message == null) return '{${expr.id}}';
        final pattern = expr.attribute != null
            ? message.findAttribute(expr.attribute!)?.value
            : message.value;
        if (pattern == null) return '{${expr.id}}';
        return resolvePattern(pattern, depth: depth + 1);
      } finally {
        _activeEntries.remove(entryKey);
      }
    }

    if (expr is FtlSelectExpression) {
      return _resolveSelectExpression(expr, depth: depth + 1);
    }

    return '{???}';
  }

  String _resolveSelectExpression(FtlSelectExpression expr, {required int depth}) {
    final selectorValue = resolveExpression(expr.selector, depth: depth);
    final cleanSelector = selectorValue
        .replaceAll(bidiFsi, '')
        .replaceAll(bidiPdi, '')
        .trim();

    // 1. Exact string match
    for (final variant in expr.variants) {
      if (variant.key == cleanSelector) {
        return resolvePattern(variant.value, depth: depth + 1);
      }
    }

    // 2. Numeric matching: exact numeric key takes precedence over CLDR category
    final numericValue = num.tryParse(cleanSelector);
    if (numericValue != null) {
      // 2a. Exact numeric match (e.g. [0] or [1])
      for (final variant in expr.variants) {
        final variantNum = num.tryParse(variant.key);
        if (variantNum != null && variantNum == numericValue) {
          return resolvePattern(variant.value, depth: depth + 1);
        }
      }

      // 2b. CLDR plural category match (e.g. [one], [few], [many], [other])
      final pluralCategory = resolveCldrPluralCategory(bundle.locale, numericValue);
      for (final variant in expr.variants) {
        if (variant.key.toLowerCase() == pluralCategory) {
          return resolvePattern(variant.value, depth: depth + 1);
        }
      }
    }

    // 3. Case-insensitive string match
    for (final variant in expr.variants) {
      if (variant.key.toLowerCase() == cleanSelector.toLowerCase()) {
        return resolvePattern(variant.value, depth: depth + 1);
      }
    }

    // 4. Fallback to default variant
    return resolvePattern(expr.defaultVariant.value, depth: depth + 1);
  }
}
