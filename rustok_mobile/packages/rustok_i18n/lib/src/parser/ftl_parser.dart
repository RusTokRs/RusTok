import '../ast/ast_nodes.dart';

/// Maximum combined nesting of placeables and inline expressions.
/// Excessive nesting becomes junk; subsequent resource entries still parse.
const int maxFtlNestingDepth = 100;

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
  int _nestingDepth = 0;

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
    // A `#` comment that is followed immediately by a message or a term is a
    // *translator comment*: the reference attaches it to that entry and does not
    // add it to the resource separately. The comment is held back until the
    // next entry is known to be a message or a term, so that a following junk
    // entry leaves the comment standalone.
    FtlComment? pendingComment;

    while (true) {
      _skipBlankLines();
      if (_isAtEnd()) break;

      final start = _pos;
      late FtlEntry entry;
      try {
        final char = _peek();
        if (char == '#') {
          entry = _parseComment();
        } else if (char == '-' && _isIdentifierStart(_peek(1))) {
          entry = _parseTerm();
        } else if (_isIdentifierStart(char)) {
          entry = _parseMessage();
        } else {
          throw FtlParseException(
            'Expected a message, term or comment at the start of the line',
            line: _line,
            column: _col,
          );
        }
      } on FtlParseException catch (error) {
        final errorPos = _pos;
        final junkEnd = _nextEntryStart(start, errorPos);
        if (junkEnd < errorPos) {
          // The line that failed starts a new entry, so the junk ends before
          // it. The resume position is on the same line as the error, which
          // means only the column has to be rewound.
          _col -= errorPos - junkEnd;
          _pos = junkEnd;
        } else {
          _advanceTo(junkEnd);
        }
        entry = FtlJunk(
          // Junk keeps the source as it was written, including the line ends
          // and blank lines that separate it from the next entry.
          content: input.substring(start, junkEnd),
          message: error.message,
        );
      }

      if (entry is FtlComment && entry.level == 0 && !_isAtEnd() && _isAtContentLine()) {
        // The comment may belong to the entry that follows it.
        pendingComment = entry;
        continue;
      }

      if (pendingComment != null) {
        if (entry is FtlMessage || entry is FtlTerm) {
          entry = _withComment(entry, pendingComment.content);
        } else {
          // The stashed comment does not belong to anything.
          entries.add(pendingComment);
        }
        pendingComment = null;
      }
      entries.add(entry);
    }

    if (pendingComment != null) entries.add(pendingComment);

    return FtlResource(entries: List.unmodifiable(entries));
  }

  /// Returns [entry] with [comment] attached as its translator comment.
  static FtlEntry _withComment(FtlEntry entry, String comment) {
    if (entry is FtlMessage) {
      return FtlMessage(
        id: entry.id,
        value: entry.value,
        attributes: entry.attributes,
        comment: comment,
      );
    }
    if (entry is FtlTerm) {
      return FtlTerm(
        id: entry.id,
        value: entry.value,
        attributes: entry.attributes,
        comment: comment,
      );
    }
    return entry;
  }

  /// True when the cursor stands at a line that has content, i.e. the current
  /// line is neither blank nor the end of the input.
  bool _isAtContentLine() {
    var p = _pos;
    while (p < input.length && input[p] == ' ') {
      p++;
    }
    return p < input.length && input[p] != '\n';
  }

  // ---------------------------------------------------------------------------
  // Top-level entries
  // ---------------------------------------------------------------------------

  /// Parses adjacent comment lines of the same level (`#`, `##`, `###`).
  ///
  /// The level marker is followed by a single space (or the end of the line),
  /// and every line of the block must start at column 0. A line with a
  /// different marker level — or a marker that is not followed by a space —
  /// starts a new comment entry, so it never swallows a valid block.
  FtlComment _parseComment() {
    final lines = <String>[];
    String? marker;
    var level = 0;

    while (true) {
      if (_isAtEnd() || _peek() != '#') break;

      final savePos = _pos;
      final saveLine = _line;
      final saveCol = _col;

      var hashes = 0;
      while (hashes < 3 && _peek() == '#') {
        _advance();
        hashes++;
      }
      final candidate = '#' * hashes;
      final afterMarker = _peek();
      // The marker must be followed by a space or a line end. A marker that
      // ends the file without either (`#` on the last line, `#tooClose`) is
      // junk in the reference, so the empty string counts as malformed too.
      final malformedMarker = afterMarker != ' ' && afterMarker != '\n';
      if (marker != null && (candidate != marker || malformedMarker)) {
        // Only a line that repeats the exact same marker continues this
        // comment; a different level — or a marker without the required space —
        // starts a new entry, which reports its own error.
        _restore(savePos, saveLine, saveCol);
        break;
      }
      if (malformedMarker) {
        _restore(savePos, saveLine, saveCol);
        throw FtlParseException(
          'A comment marker must be followed by a space, a line end '
          'or the end of the file',
          line: _line,
          column: _col,
        );
      }
      marker = candidate;
      level = hashes - 1;

      if (_peek() == ' ') _advance(); // the single space after the marker
      final line = StringBuffer();
      while (!_isAtEnd() && _peek() != '\n') {
        line.write(_advance());
      }
      lines.add(line.toString());
      if (_isAtEnd()) break;
      _advance(); // '\n'
    }

    return FtlComment(content: lines.join('\n'), level: level);
  }

  FtlTerm _parseTerm() {
    _advance(); // '-'
    final id = _parseIdentifier();
    _skipInlineWhitespace();
    if (!_match('=')) {
      throw FtlParseException('Expected "=" after term -$id', line: _line, column: _col);
    }
    _skipInlineWhitespace();

    final value = _parsePattern();
    if (value == null) {
      throw FtlParseException('Term -$id must have a value', line: _line, column: _col);
    }
    final attributes = _parseAttributes();

    return FtlTerm(
      id: id,
      value: value,
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

    final value = _parsePattern();
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
      final value = _parsePattern();
      if (value == null) {
        throw FtlParseException(
          'Attribute ".$id" must have a value',
          line: _line,
          column: _col,
        );
      }
      attributes.add(FtlAttribute(id: id, value: value));
    }
    return attributes;
  }

  /// Returns the index of the `.` that starts an attribute, or -1 when the
  /// current position does not start one.
  ///
  /// The reference looks for `blank? "."` after the value, where `blank` is
  /// spaces and line ends, so a `.` that follows a value — on a later line as
  /// well as directly after the value — always starts an attribute. Text after
  /// a value on the same line is consumed by the value itself, so in practice
  /// the dot begins a new line. A dot that is not followed by an identifier is
  /// still an attribute start, which makes the malformed line junk for the whole
  /// entry, exactly as in the reference.
  int _attributeDotIndex() {
    var p = _pos;
    while (p < input.length && (input[p] == ' ' || input[p] == '\n')) {
      p++;
    }
    if (p < input.length && input[p] == '.') return p;
    return -1;
  }

  // ---------------------------------------------------------------------------
  // Patterns
  // ---------------------------------------------------------------------------

  /// Parses a pattern starting at the current position (inline whitespace already skipped).
  ///
  /// A pattern that starts with a newline must continue on a following line
  /// (indented, or a block placeable at column 0); otherwise `null` is returned
  /// and the newline is left unconsumed.
  FtlPattern? _parsePattern() {
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
      final first = _continuationAt(_pos);
      if (first == null) return null;
      _advanceTo(first.contentPos);
      // Blank lines before the first content line are not part of the value.
      tokens.add(_NewlineToken(first.indent, leading: true));
    }

    while (!_isAtEnd()) {
      final char = _peek();

      if (char == '\n') {
        final next = _continuationAt(_pos);
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
  /// Returns `null` when the pattern ends there. A line continues the pattern
  /// when its first non-blank character is `{` (a block placeable, which may
  /// stand at column 0) or when it is indented (with spaces, per
  /// `blank_inline`) and does not start with `}`, `.`, `[` or `*` — those four
  /// characters always end a pattern, and any other character at column 0
  /// starts a new entry. A tab is never indentation.
  _Continuation? _continuationAt(int newlinePos) {
    var lineStart = newlinePos + 1;
    var blankLines = 0;

    while (true) {
      var q = lineStart;
      while (q < input.length && input[q] == ' ') {
        q++;
      }
      if (q >= input.length) return null;

      if (input[q] == '\n') {
        blankLines++;
        lineStart = q + 1;
        continue;
      }

      final indent = input.substring(lineStart, q);
      final char = input[q];
      if (char != '{') {
        if (indent.isEmpty) return null; // column 0: a new entry, a term or junk
        if (char == '}' || char == '.' || char == '[' || char == '*') return null;
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
        if (token.leading) {
          // The line break that separates `key =` from a block pattern is not
          // part of the value, but an indent that is larger than the common
          // indent is: the common indent is measured over every continuation
          // line, so one less indented line keeps the excess of the first one.
          text.write(token.indent.substring(minIndent));
          continue;
        }
        text.write('\n');
        text.write(token.indent.substring(minIndent));
      }
    }

    flush();

    // Trailing blank_inline and line ends are not part of a pattern: the
    // reference implementation trims them from the last text element and drops
    // the element entirely when nothing is left. Tabs are ordinary characters
    // and survive the trim.
    if (elements.isNotEmpty && elements.last is FtlTextElement) {
      final last = elements.removeLast() as FtlTextElement;
      final trimmed = last.value.replaceFirst(RegExp(r'[ \n\r]+$'), '');
      if (trimmed.isNotEmpty) {
        elements.add(FtlTextElement(trimmed));
      }
    }

    return FtlPattern(elements: List.unmodifiable(elements));
  }

  void _enterNesting() {
    if (_nestingDepth >= maxFtlNestingDepth) {
      throw FtlParseException('Expression nesting limit exceeded',
          line: _line, column: _col);
    }
    _nestingDepth++;
  }

  FtlPlaceable _parsePlaceable() {
    return FtlPlaceable(_parsePlaceableExpression());
  }

  /// Parses `"{" blank? expression blank? "}"` and returns the inner expression.
  FtlExpression _parsePlaceableExpression() {
    _enterNesting();
    try {
      return _parsePlaceableExpressionBody();
    } finally {
      _nestingDepth--;
    }
  }

  FtlExpression _parsePlaceableExpressionBody() {
    _advance(); // '{'
    _skipWhitespace();
    final expression = _parseExpression();
    _skipWhitespace();
    if (!_match('}')) {
      throw FtlParseException('Unclosed placeable "{"', line: _line, column: _col);
    }
    return expression;
  }

  // ---------------------------------------------------------------------------
  // Expressions
  // ---------------------------------------------------------------------------

  /// Parses an inline expression, optionally followed by `->` and select variants.
  ///
  /// Enforces the spec's callee rules (E0016–E0019, E0029): only variables,
  /// literals, functions and term attributes may act as selectors, a term
  /// attribute is never a valid value on its own, and the `->` of a select must
  /// be followed by a line end.
  FtlExpression _parseExpression() {
    _skipInlineWhitespace();
    final selector = _parseInlineExpression();
    _skipWhitespace();
    if (_peek() == '-' && _peek(1) == '>') {
      if (selector is FtlMessageReference) {
        throw FtlParseException(
          'Message references cannot be used as selectors or parameterized',
          line: _line,
          column: _col,
        );
      }
      if (selector is FtlTermReference && selector.attribute == null) {
        throw FtlParseException(
          'Term values may not be used as selectors',
          line: _line,
          column: _col,
        );
      }
      if (selector is FtlPlaceableExpression) {
        throw FtlParseException(
          'Placeables may not be used as selectors',
          line: _line,
          column: _col,
        );
      }
      _advance(); // '-'
      _advance(); // '>'
      // Select expressions are written as a block: after `->` only spaces may
      // follow before the line end.
      _skipInlineWhitespace();
      if (!_isAtEnd() && _peek() != '\n') {
        throw FtlParseException(
          'Expected a line end after "->"',
          line: _line,
          column: _col,
        );
      }
      return _parseSelectExpression(selector);
    }
    if (selector is FtlTermReference && selector.attribute != null) {
      throw FtlParseException(
        'Term attributes may not be used in placeables',
        line: _line,
        column: _col,
      );
    }
    return selector;
  }

  FtlExpression _parseInlineExpression() {
    _enterNesting();
    try {
      return _parseInlineExpressionBody();
    } finally {
      _nestingDepth--;
    }
  }

  FtlExpression _parseInlineExpressionBody() {
    final char = _peek();

    if (char == '{') {
      // A nested placeable is an inline expression: `{ { $count } }`,
      // `{NUMBER({$count})}`. The wrapper is kept in the tree; the resolver
      // resolves it to the value of the wrapped expression.
      return FtlPlaceableExpression(_parsePlaceableExpression());
    }
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
          // Each escape denotes a scalar, not half of a UTF-16 pair.
          // Fluent replaces escaped surrogates individually with U+FFFD.
          buffer.writeCharCode(
            codePoint >= 0xD800 && codePoint <= 0xDFFF ? 0xFFFD : codePoint,
          );
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

    final callIndex = _indexAfterBlank();
    Map<String, FtlExpression>? arguments;
    var positional = const <FtlExpression>[];
    if (callIndex < input.length && input[callIndex] == '(') {
      _advanceTo(callIndex);
      final call = _parseCallArguments();
      // The grammar accepts positional arguments on a term reference even
      // though the runtime never passes them to the term (`fluent-bundle`
      // discards them). They are kept in the tree instead of rejecting the
      // entry, so that the parser accepts what the reference accepts.
      positional = List.unmodifiable(call.positional);
      arguments = call.named.isEmpty ? null : call.named;
    }

    return FtlTermReference(
      id: id,
      attribute: attribute,
      arguments: arguments,
      positionalArguments: positional,
    );
  }

  /// `msg`, `msg.attribute`, or a function call `NUMBER(...)`.
  ///
  /// `CallArguments ::= blank? "(" ...` — the blanks before `(` are only part of
  /// the call when a call actually follows, so the lookahead does not consume
  /// them (a following `.attribute` must still be adjacent to the identifier).
  FtlExpression _parseIdentifierExpression() {
    final id = _parseIdentifier();
    final callIndex = _indexAfterBlank();

    if (callIndex < input.length && input[callIndex] == '(') {
      if (!_functionNamePattern.hasMatch(id)) {
        throw FtlParseException(
          'Invalid function name "$id"; functions are upper-case',
          line: _line,
          column: _col,
        );
      }
      _advanceTo(callIndex);
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
        final savePos = _pos;
        final saveLine = _line;
        final saveCol = _col;
        final name = _parseIdentifier();
        // The reference decides whether this is a named argument by looking at
        // the character after the blank without consuming the blank, so a `)`
        // that follows the identifier ends the list instead of being skipped.
        _skipWhitespace();
        if (_peek() != ':') {
          _restore(savePos, saveLine, saveCol);
          _addPositional(positional, named);
        } else {
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
        }
      } else {
        _addPositional(positional, named);
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

  /// Adds a positional argument, rejecting it when a named argument was already
  /// seen — the reference parses the argument first and reports the error after.
  void _addPositional(List<FtlExpression> positional, Map<String, FtlExpression> named) {
    if (named.isNotEmpty) {
      throw FtlParseException(
        'Positional argument cannot follow a named argument',
        line: _line,
        column: _col,
      );
    }
    positional.add(_parseInlineExpression());
  }

  /// Looks ahead for `identifier blank? ":"` without consuming input.
  bool _isNamedArgumentAhead() {
    var p = _pos;
    if (p >= input.length || !_isIdentifierStart(input[p])) return false;
    while (p < input.length && _isIdentifierPart(input[p])) {
      p++;
    }
    while (p < input.length && (input[p] == ' ' || input[p] == '\n')) {
      p++;
    }
    return p < input.length && input[p] == ':';
  }

  /// Parses the variants of a select expression; the `->` is already consumed
  /// and is known to be followed by a line end.
  FtlSelectExpression _parseSelectExpression(FtlExpression selector) {
    final variants = <FtlSelectVariant>[];
    while (true) {
      _skipWhitespace();
      if (_isAtEnd()) break;
      final char = _peek();
      if (char != '[' && char != '*') break;
      // Syntax 0.4 compatibility, as in the reference `is_variant_start`: a
      // line that opens with `[[` does not start a variant.
      if (char == '[' && _peek(1) == '[') break;

      var isDefault = false;
      if (char == '*') {
        _advance();
        isDefault = true;
      }
      if (!_match('[')) {
        throw FtlParseException('Expected "[" to start a variant key', line: _line, column: _col);
      }
      // `blank` inside the brackets: `*[key]`, `*[  key  ]` and even
      // `*[\n  key\n]` are the same key.
      _skipWhitespace();
      final key = _parseVariantKey();
      _skipWhitespace();
      if (!_match(']')) {
        throw FtlParseException(
          'Expected "]" after the variant key',
          line: _line,
          column: _col,
        );
      }

      _skipInlineWhitespace();
      final value = _parsePattern();
      if (value == null) {
        throw FtlParseException('Variant "[$key]" must have a value', line: _line, column: _col);
      }
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

  /// Parses a variant key — an identifier or a number literal — and returns it
  /// as written, so that keys such as `[01]` keep their text.
  String _parseVariantKey() {
    final char = _peek();
    if (_isDigit(char) || char == '-') {
      final start = _pos;
      if (_peek() == '-') {
        _advance();
        if (!_isDigit(_peek())) {
          throw FtlParseException(
            'Invalid variant key; keys are identifiers or numbers',
            line: _line,
            column: _col,
          );
        }
      }
      while (_isDigit(_peek())) {
        _advance();
      }
      if (_peek() == '.' && _isDigit(_peek(1))) {
        _advance();
        while (_isDigit(_peek())) {
          _advance();
        }
      }
      return input.substring(start, _pos);
    }
    if (!_isIdentifierStart(char)) {
      throw FtlParseException(
        'Invalid variant key; keys are identifiers or numbers',
        line: _line,
        column: _col,
      );
    }
    return _parseIdentifier();
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

  /// Index at which the junk starting at [entryStart] ends and the next entry
  /// begins — the boundary the reference `skip_to_next_entry_start` computes.
  ///
  /// The reference rewinds to the line end before the error and then walks the
  /// following lines until one starts like an entry: column 0 with a letter,
  /// `-` or `#`. Lines that do not look like an entry — including lines starting
  /// with a digit, `/` or `[` — stay inside the junk, together with their line
  /// ends and blank lines. When the error is on the junk's first line there is
  /// nothing to rewind to and the remainder of that line is skipped first.
  int _nextEntryStart(int entryStart, int errorPos) {
    // The search includes the error position, as in `@fluent/syntax`, where
    // `lastIndexOf(EOL, index)` finds the line end the error sits on.
    final previousLineEnd = input.lastIndexOf('\n', errorPos);
    var p = entryStart < previousLineEnd
        ? previousLineEnd + 1 // the failing line may itself start an entry
        : _lineEndAfter(errorPos); // skip the rest of the failing line
    while (p < input.length) {
      final char = input[p];
      if (_isIdentifierStart(char) || char == '-' || char == '#') return p;
      p = _lineEndAfter(p);
    }
    return input.length;
  }

  /// Index of the first character after the line end at or after [pos].
  int _lineEndAfter(int pos) {
    final newline = input.indexOf('\n', pos);
    return newline < 0 ? input.length : newline + 1;
  }

  /// Skips blank blocks — whitespace-only lines and blank lines — plus trailing
  /// blanks at the end of the input.
  ///
  /// The indentation of the next line that has content is deliberately left
  /// unconsumed: entries and comments only start at column 0, so a mis-indented
  /// line is reported as junk instead of silently starting an entry.
  void _skipBlankLines() {
    while (!_isAtEnd()) {
      var p = _pos;
      while (p < input.length && input[p] == ' ') {
        p++;
      }
      if (p >= input.length) {
        _advanceTo(p);
        return;
      }
      if (input[p] == '\n') {
        _advanceTo(p + 1);
        continue;
      }
      return;
    }
  }

  /// Restores a previously saved scanner position.
  void _restore(int pos, int line, int col) {
    _pos = pos;
    _line = line;
    _col = col;
  }

  /// Skips `blank_inline` — spaces only.
  ///
  /// The grammar defines `blank_inline ::= "\u0020"+`, so a tab is an ordinary
  /// character in FTL and is never treated as layout (see the reference
  /// `tab.ftl` fixtures, where a tab after `=` belongs to the value while a tab
  /// before `=` or as indentation is an error).
  void _skipInlineWhitespace() {
    while (!_isAtEnd() && _peek() == ' ') {
      _advance();
    }
  }

  /// Skips `blank` — spaces and line ends, but never tabs.
  void _skipWhitespace() {
    while (!_isAtEnd() && (_peek() == ' ' || _peek() == '\n')) {
      _advance();
    }
  }

  /// Index of the first character after `blank` starting at the cursor.
  ///
  /// Used for the non-committing lookahead of `CallArguments`.
  int _indexAfterBlank() {
    var p = _pos;
    while (p < input.length && (input[p] == ' ' || input[p] == '\n')) {
      p++;
    }
    return p;
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
