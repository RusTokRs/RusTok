import '../ast/ast_nodes.dart';

class FtlParseException implements Exception {
  const FtlParseException(this.message, {this.line, this.column});

  final String message;
  final int? line;
  final int? column;

  @override
  String toString() {
    if (line != null && column != null) {
      return 'FtlParseException: $message at line $line, column $column';
    }
    return 'FtlParseException: $message';
  }
}

/// Function names are upper-case identifiers, e.g. `NUMBER`, `DATETIME`.
final RegExp _functionNamePattern = RegExp(r'^[A-Z][A-Z0-9_-]*$');

/// Recursive-descent parser for Project Fluent 1.0 (FTL) syntax.
///
/// Implements multiline patterns with common-indent removal, attributes,
/// placeables, select expressions, terms with named arguments, built-in
/// function calls, comments, and spec-compliant string escapes.
///
/// Error recovery: an invalid entry becomes an [FtlJunk] node and parsing
/// continues with the next entry at column 0, as the Fluent specification requires.
class FtlParser {
  FtlParser(String source) : input = _normalizeSource(source);

  final String input;
  int _pos = 0;
  int _line = 1;
  int _col = 1;

  static FtlResource parse(String source) => FtlParser(source).parseResource();

  static String _normalizeSource(String source) {
    var text = source;
    if (text.startsWith('\uFEFF')) {
      text = text.substring(1); // byte-order mark
    }
    return text.replaceAll('\r\n', '\n').replaceAll('\r', '\n');
  }

  FtlResource parseResource() {
    final entries = <FtlEntry>[];

    while (true) {
      _skipWhitespace();
      if (_isAtEnd()) break;

      final start = _pos;
      try {
        final char = _peek();
        if (char == '#') {
          entries.add(_parseComment());
        } else if (char == '-' && _isIdentifierStart(_peek(1))) {
          entries.add(_parseTerm());
        } else if (_isIdentifierStart(char)) {
          entries.add(_parseMessage());
        } else {
          throw FtlParseException(
            'Expected a message, term or comment',
            line: _line,
            column: _col,
          );
        }
      } on FtlParseException catch (error) {
        _recoverToNextEntry();
        entries.add(FtlJunk(
          content: input.substring(start, _pos).trimRight(),
          message: error.message,
        ));
      }
    }

    return FtlResource(entries: List.unmodifiable(entries));
  }

  // ---------------------------------------------------------------------------
  // Top-level entries
  // ---------------------------------------------------------------------------

  FtlComment _parseComment() {
    final lines = <String>[];
    while (!_isAtEnd() && _peek() == '#') {
      final line = StringBuffer();
      while (!_isAtEnd() && _peek() != '\n') {
        line.write(_advance());
      }
      lines.add(_stripCommentMarker(line.toString()));
      if (_isAtEnd()) break;
      _advance(); // '\n'
      // Continue only while the next line is another comment line.
      _skipInlineWhitespace();
      if (_peek() != '#') break;
    }
    return FtlComment(content: lines.join('\n'));
  }

  static String _stripCommentMarker(String line) {
    var i = 0;
    while (i < line.length && line[i] == '#') {
      i++;
    }
    var rest = line.substring(i);
    if (rest.startsWith(' ')) rest = rest.substring(1);
    return rest;
  }

  FtlTerm _parseTerm() {
    _advance(); // '-'
    final id = _parseIdentifier();
    _skipInlineWhitespace();
    if (!_match('=')) {
      throw FtlParseException('Expected "=" after term -$id', line: _line, column: _col);
    }
    _skipInlineWhitespace();

    final value = _parsePattern(inVariant: false);
    final attributes = _parseAttributes();

    return FtlTerm(
      id: id,
      value: value ?? const FtlPattern(elements: []),
      attributes: List.unmodifiable(attributes),
    );
  }

  FtlMessage _parseMessage() {
    final id = _parseIdentifier();
    _skipInlineWhitespace();
    if (!_match('=')) {
      throw FtlParseException('Expected "=" after message "$id"', line: _line, column: _col);
    }
    _skipInlineWhitespace();

    final value = _parsePattern(inVariant: false);
    final attributes = _parseAttributes();
    if (value == null && attributes.isEmpty) {
      throw FtlParseException(
        'Message "$id" must have a value or at least one attribute',
        line: _line,
        column: _col,
      );
    }

    return FtlMessage(
      id: id,
      value: value,
      attributes: List.unmodifiable(attributes),
    );
  }

  List<FtlAttribute> _parseAttributes() {
    final attributes = <FtlAttribute>[];
    while (true) {
      final dot = _attributeDotIndex();
      if (dot < 0) break;
      _advanceTo(dot + 1);

      final id = _parseIdentifier();
      _skipInlineWhitespace();
      if (!_match('=')) {
        throw FtlParseException('Expected "=" after attribute ".$id"', line: _line, column: _col);
      }
      _skipInlineWhitespace();
      final value = _parsePattern(inVariant: false) ?? const FtlPattern(elements: []);
      attributes.add(FtlAttribute(id: id, value: value));
    }
    return attributes;
  }

  /// Returns the index of the `.` that starts an attribute on the next line,
  /// or -1 when the current position does not start an attribute.
  int _attributeDotIndex() {
    var p = _pos;
    while (p < input.length && (input[p] == ' ' || input[p] == '\t')) {
      p++;
    }
    if (p < input.length && input[p] == '\n') {
      p++;
      while (p < input.length && (input[p] == ' ' || input[p] == '\t')) {
        p++;
      }
    }
    if (p + 1 < input.length && input[p] == '.' && _isIdentifierStart(input[p + 1])) {
      return p;
    }
    return -1;
  }

  // ---------------------------------------------------------------------------
  // Patterns
  // ---------------------------------------------------------------------------

  /// Parses a pattern starting at the current position (inline whitespace already skipped).
  ///
  /// A pattern that starts with a newline must continue on an indented line;
  /// otherwise `null` is returned and the newline is left unconsumed.
  FtlPattern? _parsePattern({required bool inVariant}) {
    if (_isAtEnd()) return null;

    final tokens = <Object>[];
    final text = StringBuffer();

    void flushText() {
      if (text.isNotEmpty) {
        tokens.add(_TextToken(text.toString()));
        text.clear();
      }
    }

    if (_peek() == '\n') {
      final first = _continuationAt(_pos, inVariant: inVariant);
      if (first == null) return null;
      _advanceTo(first.contentPos);
      // Blank lines before the first content line are not part of the value.
      tokens.add(_NewlineToken(first.indent, leading: true));
    }

    while (!_isAtEnd()) {
      final char = _peek();

      if (char == '\n') {
        final next = _continuationAt(_pos, inVariant: inVariant);
        if (next == null) break;
        _advanceTo(next.contentPos);
        if (next.blankLines > 0) {
          text.write('\n' * next.blankLines);
        }
        flushText();
        tokens.add(_NewlineToken(next.indent));
        continue;
      }

      if (char == '{') {
        flushText();
        tokens.add(_parsePlaceable());
        continue;
      }

      if (char == '}') {
        if (inVariant) break;
        throw FtlParseException(
          'Unescaped "}" in pattern; use { "}" } instead',
          line: _line,
          column: _col,
        );
      }

      text.write(_advance());
    }

    flushText();
    return _buildPattern(tokens);
  }

  /// Inspects the line following the newline at [newlinePos].
  ///
  /// Returns `null` when the pattern ends there: at end of input, at a column-0
  /// entry, at an attribute (`.name`), or (in a variant) at a variant key or
  /// closing brace. Otherwise returns the continuation line details.
  _Continuation? _continuationAt(int newlinePos, {required bool inVariant}) {
    var lineStart = newlinePos + 1;
    var blankLines = 0;

    while (true) {
      var q = lineStart;
      while (q < input.length && (input[q] == ' ' || input[q] == '\t')) {
        q++;
      }
      if (q >= input.length) return null;

      if (input[q] == '\n') {
        blankLines++;
        lineStart = q + 1;
        continue;
      }

      final indent = input.substring(lineStart, q);
      if (indent.isEmpty) return null; // column 0: a new entry or junk

      final char = input[q];
      if (inVariant) {
        if (char == '[' || char == '*' || char == '}') return null;
      } else if (char == '.' && q + 1 < input.length && _isIdentifierStart(input[q + 1])) {
        return null; // attribute
      }

      return _Continuation(blankLines: blankLines, indent: indent, contentPos: q);
    }
  }

  /// Builds the final element list, removing the common indentation of all
  /// continuation lines. Leading indentation beyond the common amount is kept.
  FtlPattern _buildPattern(List<Object> tokens) {
    var minIndent = -1;
    for (final token in tokens) {
      if (token is _NewlineToken) {
        if (minIndent < 0 || token.indent.length < minIndent) {
          minIndent = token.indent.length;
        }
      }
    }

    final elements = <FtlPatternElement>[];
    final text = StringBuffer();

    void flush() {
      if (text.isNotEmpty) {
        elements.add(FtlTextElement(text.toString()));
        text.clear();
      }
    }

    for (final token in tokens) {
      if (token is _TextToken) {
        text.write(token.value);
      } else if (token is FtlPlaceable) {
        flush();
        elements.add(token);
      } else if (token is _NewlineToken) {
        if (token.leading) continue;
        text.write('\n');
        text.write(token.indent.substring(minIndent));
      }
    }

    flush();
    return FtlPattern(elements: List.unmodifiable(elements));
  }

  FtlPlaceable _parsePlaceable() {
    _advance(); // '{'
    _skipWhitespace();
    final expression = _parseExpression();
    _skipWhitespace();
    if (!_match('}')) {
      throw FtlParseException('Unclosed placeable "{"', line: _line, column: _col);
    }
    return FtlPlaceable(expression);
  }

  // ---------------------------------------------------------------------------
  // Expressions
  // ---------------------------------------------------------------------------

  /// Parses an inline expression, optionally followed by `->` and select variants.
  FtlExpression _parseExpression() {
    _skipInlineWhitespace();
    final selector = _parseInlineExpression();
    _skipInlineWhitespace();
    if (_peek() == '-' && _peek(1) == '>') {
      return _parseSelectExpression(selector);
    }
    return selector;
  }

  FtlExpression _parseInlineExpression() {
    final char = _peek();

    if (char == '\$') {
      _advance();
      return FtlVariableReference(_parseIdentifier());
    }
    if (char == '"') {
      return _parseStringLiteral();
    }
    if (char == '-' && _isDigit(_peek(1))) {
      return _parseNumberLiteral();
    }
    if (char == '-' && _isIdentifierStart(_peek(1))) {
      return _parseTermReference();
    }
    if (_isDigit(char)) {
      return _parseNumberLiteral();
    }
    if (_isIdentifierStart(char)) {
      return _parseIdentifierExpression();
    }

    throw FtlParseException('Invalid expression start "$char"', line: _line, column: _col);
  }

  FtlStringLiteral _parseStringLiteral() {
    _advance(); // opening quote
    final buffer = StringBuffer();

    while (true) {
      if (_isAtEnd() || _peek() == '\n') {
        throw FtlParseException('Unterminated string literal', line: _line, column: _col);
      }
      final char = _advance();
      if (char == '"') break;

      if (char == '\\') {
        final escape = _peek();
        if (escape == '\\' || escape == '"') {
          _advance();
          buffer.write(escape);
        } else if (escape == 'u' || escape == 'U') {
          _advance();
          final digits = escape == 'u' ? 4 : 6;
          final hex = StringBuffer();
          for (var i = 0; i < digits; i++) {
            if (!_isHexDigit(_peek())) {
              throw FtlParseException('Invalid unicode escape', line: _line, column: _col);
            }
            hex.write(_advance());
          }
          final codePoint = int.parse(hex.toString(), radix: 16);
          if (codePoint > 0x10FFFF) {
            throw FtlParseException('Unicode escape out of range', line: _line, column: _col);
          }
          buffer.writeCharCode(codePoint);
        } else {
          throw FtlParseException(
            'Invalid escape sequence in string literal; only \\\\, \\", \\uXXXX and \\UXXXXXX are allowed',
            line: _line,
            column: _col,
          );
        }
      } else {
        buffer.write(char);
      }
    }

    return FtlStringLiteral(buffer.toString());
  }

  FtlNumberLiteral _parseNumberLiteral() {
    final raw = StringBuffer();
    if (_peek() == '-') {
      raw.write(_advance());
    }
    while (_isDigit(_peek())) {
      raw.write(_advance());
    }
    if (_peek() == '.' && _isDigit(_peek(1))) {
      raw.write(_advance());
      while (_isDigit(_peek())) {
        raw.write(_advance());
      }
    }
    final text = raw.toString();
    final value = num.tryParse(text);
    if (value == null) {
      throw FtlParseException('Invalid number "$text"', line: _line, column: _col);
    }
    return FtlNumberLiteral(value);
  }

  /// `-term`, `-term.attribute` and `-term(name: "literal")`.
  FtlTermReference _parseTermReference() {
    _advance(); // '-'
    final id = _parseIdentifier();

    String? attribute;
    if (_peek() == '.') {
      _advance();
      attribute = _parseIdentifier();
    }

    _skipInlineWhitespace();
    Map<String, FtlExpression>? arguments;
    if (_peek() == '(') {
      final call = _parseCallArguments();
      if (call.positional.isNotEmpty) {
        throw FtlParseException(
          'Term arguments must be named (-term(name: value))',
          line: _line,
          column: _col,
        );
      }
      arguments = call.named;
    }

    return FtlTermReference(id: id, attribute: attribute, arguments: arguments);
  }

  /// `msg`, `msg.attribute`, or a function call `NUMBER(...)`.
  FtlExpression _parseIdentifierExpression() {
    final id = _parseIdentifier();
    _skipInlineWhitespace();

    if (_peek() == '(') {
      if (!_functionNamePattern.hasMatch(id)) {
        throw FtlParseException(
          'Invalid function name "$id"; functions are upper-case',
          line: _line,
          column: _col,
        );
      }
      final call = _parseCallArguments();
      return FtlFunctionReference(
        name: id,
        positional: List.unmodifiable(call.positional),
        named: Map.unmodifiable(call.named),
      );
    }

    String? attribute;
    if (_peek() == '.') {
      _advance();
      attribute = _parseIdentifier();
    }
    return FtlMessageReference(id: id, attribute: attribute);
  }

  /// Parses `( positional, name: literal, ... )`. The current character must be `(`.
  _CallArguments _parseCallArguments() {
    _advance(); // '('
    final positional = <FtlExpression>[];
    final named = <String, FtlExpression>{};

    while (true) {
      _skipWhitespace();
      if (_isAtEnd()) {
        throw FtlParseException('Unclosed argument list', line: _line, column: _col);
      }
      if (_peek() == ')') {
        _advance();
        break;
      }

      if (_isNamedArgumentAhead()) {
        final name = _parseIdentifier();
        _skipInlineWhitespace();
        _advance(); // ':'
        _skipWhitespace();
        final value = _parseInlineExpression();
        if (value is! FtlStringLiteral && value is! FtlNumberLiteral) {
          throw FtlParseException(
            'Named argument "$name" must be a string or number literal',
            line: _line,
            column: _col,
          );
        }
        if (named.containsKey(name)) {
          throw FtlParseException('Duplicate named argument "$name"', line: _line, column: _col);
        }
        named[name] = value;
      } else {
        if (named.isNotEmpty) {
          throw FtlParseException(
            'Positional argument cannot follow a named argument',
            line: _line,
            column: _col,
          );
        }
        positional.add(_parseInlineExpression());
      }

      _skipWhitespace();
      if (_peek() == ',') {
        _advance();
        continue;
      }
      if (_peek() == ')') {
        _advance();
        break;
      }
      throw FtlParseException('Expected "," or ")" in argument list', line: _line, column: _col);
    }

    return _CallArguments(positional: positional, named: named);
  }

  /// Looks ahead for `identifier :` without consuming input.
  bool _isNamedArgumentAhead() {
    var p = _pos;
    if (p >= input.length || !_isIdentifierStart(input[p])) return false;
    while (p < input.length && _isIdentifierPart(input[p])) {
      p++;
    }
    while (p < input.length && (input[p] == ' ' || input[p] == '\t')) {
      p++;
    }
    return p < input.length && input[p] == ':';
  }

  FtlSelectExpression _parseSelectExpression(FtlExpression selector) {
    _advance(); // '-'
    _advance(); // '>'

    final variants = <FtlSelectVariant>[];
    while (true) {
      _skipWhitespace();
      if (_isAtEnd()) break;
      final char = _peek();
      if (char != '[' && char != '*') break;

      var isDefault = false;
      if (char == '*') {
        _advance();
        isDefault = true;
      }
      if (!_match('[')) {
        throw FtlParseException('Expected "[" to start a variant key', line: _line, column: _col);
      }
      _skipInlineWhitespace();

      final keyBuffer = StringBuffer();
      while (!_isAtEnd() && _peek() != ']' && _peek() != '\n') {
        keyBuffer.write(_advance());
      }
      if (!_match(']')) {
        throw FtlParseException('Unclosed variant key', line: _line, column: _col);
      }
      final key = keyBuffer.toString().trim();
      if (key.isEmpty) {
        throw FtlParseException('Empty variant key', line: _line, column: _col);
      }

      _skipInlineWhitespace();
      final value = _parsePattern(inVariant: true) ?? const FtlPattern(elements: []);
      variants.add(FtlSelectVariant(key: key, value: value, isDefault: isDefault));
    }

    if (variants.isEmpty) {
      throw FtlParseException(
        'Select expression must contain at least one variant',
        line: _line,
        column: _col,
      );
    }
    final defaults = variants.where((v) => v.isDefault).length;
    if (defaults != 1) {
      throw FtlParseException(
        'Select expression must have exactly one default variant (marked with *)',
        line: _line,
        column: _col,
      );
    }

    return FtlSelectExpression(
      selector: selector,
      variants: List.unmodifiable(variants),
    );
  }

  String _parseIdentifier() {
    if (!_isIdentifierStart(_peek())) {
      throw FtlParseException('Expected identifier', line: _line, column: _col);
    }
    final buffer = StringBuffer();
    while (!_isAtEnd() && _isIdentifierPart(_peek())) {
      buffer.write(_advance());
    }
    return buffer.toString();
  }

  // ---------------------------------------------------------------------------
  // Low-level helpers
  // ---------------------------------------------------------------------------

  /// Skips to the next line that starts an entry at column 0.
  void _recoverToNextEntry() {
    _skipToNextLine();
    while (!_isAtEnd()) {
      final char = _peek();
      if (_isIdentifierStart(char) || char == '-' || char == '#') return;
      _skipToNextLine();
    }
  }

  void _skipToNextLine() {
    while (!_isAtEnd() && _peek() != '\n') {
      _advance();
    }
    if (!_isAtEnd() && _peek() == '\n') {
      _advance();
    }
  }

  void _skipInlineWhitespace() {
    while (!_isAtEnd() && (_peek() == ' ' || _peek() == '\t')) {
      _advance();
    }
  }

  void _skipWhitespace() {
    while (!_isAtEnd() && (_peek() == ' ' || _peek() == '\t' || _peek() == '\n')) {
      _advance();
    }
  }

  String _peek([int offset = 0]) {
    final idx = _pos + offset;
    if (idx >= input.length) return '';
    return input[idx];
  }

  bool _match(String char) {
    if (_peek() == char) {
      _advance();
      return true;
    }
    return false;
  }

  String _advance() {
    final char = input[_pos++];
    if (char == '\n') {
      _line++;
      _col = 1;
    } else {
      _col++;
    }
    return char;
  }

  void _advanceTo(int target) {
    while (_pos < target) {
      _advance();
    }
  }

  bool _isAtEnd() => _pos >= input.length;

  static bool _isAsciiLetter(String char) {
    if (char.isEmpty) return false;
    final code = char.codeUnitAt(0);
    return (code >= 0x41 && code <= 0x5A) || (code >= 0x61 && code <= 0x7A);
  }

  static bool _isDigit(String char) {
    if (char.isEmpty) return false;
    final code = char.codeUnitAt(0);
    return code >= 0x30 && code <= 0x39;
  }

  static bool _isHexDigit(String char) {
    if (char.isEmpty) return false;
    final code = char.codeUnitAt(0);
    return (code >= 0x30 && code <= 0x39) ||
        (code >= 0x41 && code <= 0x46) ||
        (code >= 0x61 && code <= 0x66);
  }

  static bool _isIdentifierStart(String char) => _isAsciiLetter(char);

  static bool _isIdentifierPart(String char) =>
      _isAsciiLetter(char) || _isDigit(char) || char == '-' || char == '_';
}

/// Literal text collected while parsing a pattern.
class _TextToken {
  const _TextToken(this.value);

  final String value;
}

/// A line break inside a pattern, together with the indentation of the
/// following line. The indentation is trimmed by the common indent later.
class _NewlineToken {
  const _NewlineToken(this.indent, {this.leading = false});

  final String indent;

  /// True for the break that separates `key =` from a value on the next line.
  final bool leading;
}

/// Details of a continuation line inside a multiline pattern.
class _Continuation {
  const _Continuation({
    required this.blankLines,
    required this.indent,
    required this.contentPos,
  });

  final int blankLines;
  final String indent;
  final int contentPos;
}

/// Parsed arguments of a function or term call.
class _CallArguments {
  const _CallArguments({required this.positional, required this.named});

  final List<FtlExpression> positional;
  final Map<String, FtlExpression> named;
}
