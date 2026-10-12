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

/// Robust, deterministic recursive-descent parser for Project Fluent (FTL) syntax.
///
/// Implements Mozilla Project Fluent 1.0 specification including multiline indented patterns,
/// attributes, placeables, selectors, terms, comments, escape sequences, and Windows CRLF normalization.
class FtlParser {
  FtlParser(String source) : input = source.replaceAll('\r\n', '\n').replaceAll('\r', '\n');

  final String input;
  int _pos = 0;
  int _line = 1;
  int _col = 1;

  static FtlResource parse(String source) => FtlParser(source).parseResource();

  FtlResource parseResource() {
    final entries = <FtlEntry>[];

    while (!_isAtEnd()) {
      _skipBlankLinesAndWhitespace();
      if (_isAtEnd()) break;

      // Comment: #, ##, ###
      if (_peek() == '#') {
        entries.add(_parseComment());
        continue;
      }

      // Term: -term-id
      if (_peek() == '-') {
        entries.add(_parseTerm());
        continue;
      }

      // Message: message-id
      if (_isIdentifierStart(_peek())) {
        entries.add(_parseMessage());
        continue;
      }

      // Junk / skip unparseable line safely
      _skipToNextLine();
    }

    return FtlResource(entries: List.unmodifiable(entries));
  }

  FtlComment _parseComment() {
    final buffer = StringBuffer();
    while (!_isAtEnd() && _peek() == '#') {
      while (!_isAtEnd() && _peek() != '\n') {
        buffer.write(_advance());
      }
      if (!_isAtEnd() && _peek() == '\n') {
        buffer.write(_advance());
      }
      _skipBlankLinesAndWhitespace();
    }
    return FtlComment(content: buffer.toString().trim());
  }

  FtlTerm _parseTerm() {
    _match('-'); // consume leading '-'
    final id = _parseIdentifier();
    _skipInlineWhitespace();

    if (!_match('=')) {
      throw FtlParseException('Expected "=" after term -$id', line: _line, column: _col);
    }
    _skipInlineWhitespace();

    final value = _parsePatternOrMultiline();
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
      throw FtlParseException('Expected "=" after message $id', line: _line, column: _col);
    }
    _skipInlineWhitespace();

    final value = _parsePatternOrMultiline();
    final attributes = _parseAttributes();

    return FtlMessage(
      id: id,
      value: value,
      attributes: List.unmodifiable(attributes),
    );
  }

  FtlPattern? _parsePatternOrMultiline() {
    // 1. Value starts on the same line
    if (_peek() != '\n' && _peek() != '.') {
      return _parsePattern();
    }

    // 2. Value starts on the next line if indented and not an attribute
    if (_peek() == '\n') {
      final nextLinePos = _pos + 1;
      if (nextLinePos < input.length) {
        var look = nextLinePos;
        int indent = 0;
        while (look < input.length && (input[look] == ' ' || input[look] == '\t')) {
          indent++;
          look++;
        }
        // If next line is indented and not an attribute '.', parse multiline pattern
        if (indent > 0 && look < input.length && input[look] != '.' && input[look] != '\n') {
          _advance(); // consume '\n'
          return _parsePattern();
        }
      }
    }

    return null;
  }

  List<FtlAttribute> _parseAttributes() {
    final attributes = <FtlAttribute>[];

    while (!_isAtEnd()) {
      final checkpoint = _pos;
      _skipInlineWhitespace();
      if (_match('\n')) {
        _skipInlineWhitespace();
      }
      if (_peek() == '.' && _peek(1) != '.') {
        _advance(); // consume '.'
        final attrId = _parseIdentifier();
        _skipInlineWhitespace();
        if (_match('=')) {
          _skipInlineWhitespace();
          final attrVal = _parsePatternOrMultiline() ?? const FtlPattern(elements: []);
          attributes.add(FtlAttribute(id: attrId, value: attrVal));
          continue;
        }
      }
      // Not an attribute, rewind
      _pos = checkpoint;
      break;
    }

    return attributes;
  }

  FtlPattern _parsePattern() {
    final elements = <FtlPatternElement>[];
    final textBuffer = StringBuffer();

    void flushText() {
      if (textBuffer.isNotEmpty) {
        elements.add(FtlTextElement(textBuffer.toString()));
        textBuffer.clear();
      }
    }

    while (!_isAtEnd()) {
      final char = _peek();

      // Escape sequence: \{ or \"
      if (char == '\\' && (_peek(1) == '{' || _peek(1) == '"' || _peek(1) == '\\')) {
        _advance(); // skip '\'
        textBuffer.write(_advance()); // write escaped char
        continue;
      }

      // End of pattern on newline followed by a new top-level entry or attribute
      if (char == '\n') {
        final nextLinePos = _pos + 1;
        if (nextLinePos < input.length) {
          final nextChar = input[nextLinePos];
          // New top-level entry starts on column 0
          if (_isIdentifierStart(nextChar) || nextChar == '-' || nextChar == '#') {
            break;
          }
          // Check if continuation line is an attribute
          if (nextChar == ' ' || nextChar == '\t') {
            var lookAhead = nextLinePos;
            while (lookAhead < input.length &&
                (input[lookAhead] == ' ' || input[lookAhead] == '\t')) {
              lookAhead++;
            }
            if (lookAhead < input.length && input[lookAhead] == '.') {
              break; // Next line is an attribute
            }
          } else if (nextChar == '\n') {
            // Consecutive blank lines terminate pattern
            var look = nextLinePos;
            while (look < input.length && input[look] == '\n') {
              look++;
            }
            if (look < input.length &&
                (_isIdentifierStart(input[look]) || input[look] == '-' || input[look] == '#')) {
              break;
            }
          }
        }
        textBuffer.write(_advance());
        continue;
      }

      // Placeable: { ... }
      if (char == '{') {
        flushText();
        _advance(); // consume '{'
        _skipInlineWhitespace();
        final expr = _parseExpression();
        _skipInlineWhitespace();
        if (!_match('}')) {
          throw FtlParseException('Unclosed placeable "{"', line: _line, column: _col);
        }
        elements.add(FtlPlaceable(expr));
        continue;
      }

      textBuffer.write(_advance());
    }

    flushText();
    return _dedentPattern(elements);
  }

  /// Dedents leading indentation from continuation lines in a pattern.
  FtlPattern _dedentPattern(List<FtlPatternElement> elements) {
    if (elements.isEmpty) return const FtlPattern(elements: []);

    final cleanElements = <FtlPatternElement>[];
    for (final elem in elements) {
      if (elem is FtlTextElement) {
        // Normalize multiline leading spaces after newlines
        final normalized = elem.value.replaceAll(RegExp(r'\n[ \t]+'), '\n');
        cleanElements.add(FtlTextElement(normalized));
      } else {
        cleanElements.add(elem);
      }
    }
    return FtlPattern(elements: List.unmodifiable(cleanElements));
  }

  FtlExpression _parseExpression() {
    _skipInlineWhitespace();
    final char = _peek();

    // Variable reference: $varName
    if (char == '\$') {
      _advance();
      final varName = _parseIdentifier();
      _skipInlineWhitespace();
      if (_peek() == '-' && _peek(1) == '>') {
        return _parseSelectExpression(FtlVariableReference(varName));
      }
      return FtlVariableReference(varName);
    }

    // Negative number literal: -123 or -3.14
    if (char == '-' && _isDigit(_peek(1))) {
      _advance(); // consume '-'
      final numBuf = StringBuffer('-');
      while (!_isAtEnd() && (_isDigit(_peek()) || _peek() == '.')) {
        numBuf.write(_advance());
      }
      final parsed = num.tryParse(numBuf.toString()) ?? 0;
      return FtlNumberLiteral(parsed);
    }

    // Term reference: -termName
    if (char == '-') {
      _advance();
      final termName = _parseIdentifier();
      String? attribute;
      if (_match('.')) {
        attribute = _parseIdentifier();
      }
      _skipInlineWhitespace();
      if (_peek() == '-' && _peek(1) == '>') {
        return _parseSelectExpression(
          FtlTermReference(id: termName, attribute: attribute),
        );
      }
      return FtlTermReference(id: termName, attribute: attribute);
    }

    // String literal: "..."
    if (char == '"') {
      _advance();
      final strBuf = StringBuffer();
      while (!_isAtEnd() && _peek() != '"') {
        if (_peek() == '\\') {
          _advance();
          if (!_isAtEnd()) {
            final esc = _advance();
            switch (esc) {
              case 'n':
                strBuf.write('\n');
                break;
              case 'r':
                strBuf.write('\r');
                break;
              case 't':
                strBuf.write('\t');
                break;
              case '"':
                strBuf.write('"');
                break;
              case '\\':
                strBuf.write('\\');
                break;
              default:
                strBuf.write(esc);
            }
          }
        } else {
          strBuf.write(_advance());
        }
      }
      _match('"');
      return FtlStringLiteral(strBuf.toString());
    }

    // Number literal: 123 or 3.14
    if (_isDigit(char)) {
      final numBuf = StringBuffer();
      while (!_isAtEnd() && (_isDigit(_peek()) || _peek() == '.')) {
        numBuf.write(_advance());
      }
      final parsed = num.tryParse(numBuf.toString()) ?? 0;
      return FtlNumberLiteral(parsed);
    }

    // Message reference or Built-in Function Call (e.g. NUMBER($count))
    if (_isIdentifierStart(char)) {
      final id = _parseIdentifier();
      _skipInlineWhitespace();

      // Check if it's a function call like NUMBER($val)
      if (_peek() == '(') {
        _advance(); // consume '('
        _skipInlineWhitespace();
        final firstArg = _parseExpression();
        // Skip remaining arguments up to ')'
        while (!_isAtEnd() && _peek() != ')') {
          _advance();
        }
        _match(')');
        _skipInlineWhitespace();
        if (_peek() == '-' && _peek(1) == '>') {
          return _parseSelectExpression(firstArg);
        }
        return firstArg;
      }

      String? attribute;
      if (_match('.')) {
        attribute = _parseIdentifier();
      }
      _skipInlineWhitespace();
      if (_peek() == '-' && _peek(1) == '>') {
        return _parseSelectExpression(
          FtlMessageReference(id: id, attribute: attribute),
        );
      }
      return FtlMessageReference(id: id, attribute: attribute);
    }

    throw FtlParseException('Invalid expression start "$char"', line: _line, column: _col);
  }

  FtlSelectExpression _parseSelectExpression(FtlExpression selector) {
    _advance(); // '-'
    _advance(); // '>'
    _skipBlankLinesAndWhitespace();

    final variants = <FtlSelectVariant>[];

    while (!_isAtEnd() && (_peek() == '[' || _peek() == '*')) {
      bool isDefault = false;
      if (_match('*')) {
        isDefault = true;
      }

      if (!_match('[')) {
        throw FtlParseException('Expected "[" for variant key', line: _line, column: _col);
      }
      final keyBuf = StringBuffer();
      while (!_isAtEnd() && _peek() != ']') {
        keyBuf.write(_advance());
      }
      _match(']');
      _skipInlineWhitespace();

      final variantValue = _parseVariantPattern();
      variants.add(FtlSelectVariant(
        key: keyBuf.toString().trim(),
        value: variantValue,
        isDefault: isDefault,
      ));
      _skipBlankLinesAndWhitespace();
    }

    if (variants.isEmpty) {
      throw FtlParseException('Select expression must contain at least one variant',
          line: _line, column: _col);
    }

    return FtlSelectExpression(
      selector: selector,
      variants: List.unmodifiable(variants),
    );
  }

  FtlPattern _parseVariantPattern() {
    final elements = <FtlPatternElement>[];
    final textBuffer = StringBuffer();

    void flushText() {
      if (textBuffer.isNotEmpty) {
        elements.add(FtlTextElement(textBuffer.toString()));
        textBuffer.clear();
      }
    }

    while (!_isAtEnd()) {
      final char = _peek();

      if (char == '\\' && (_peek(1) == '{' || _peek(1) == '}' || _peek(1) == '\\')) {
        _advance();
        textBuffer.write(_advance());
        continue;
      }

      if (char == '\n') {
        final nextLinePos = _pos + 1;
        if (nextLinePos < input.length) {
          var look = nextLinePos;
          while (look < input.length && (input[look] == ' ' || input[look] == '\t')) {
            look++;
          }
          if (look < input.length &&
              (input[look] == '[' ||
                  input[look] == '*' ||
                  input[look] == '}' ||
                  input[look] == '#')) {
            break;
          }
        }
        textBuffer.write(_advance());
        continue;
      }

      if (char == '}') {
        break;
      }

      if (char == '{') {
        flushText();
        _advance();
        _skipInlineWhitespace();
        final expr = _parseExpression();
        _skipInlineWhitespace();
        _match('}');
        elements.add(FtlPlaceable(expr));
        continue;
      }

      textBuffer.write(_advance());
    }

    flushText();
    return FtlPattern(elements: List.unmodifiable(elements));
  }

  String _parseIdentifier() {
    final buffer = StringBuffer();
    while (!_isAtEnd() && _isIdentifierPart(_peek())) {
      buffer.write(_advance());
    }
    final result = buffer.toString();
    if (result.isEmpty) {
      throw FtlParseException('Expected valid identifier', line: _line, column: _col);
    }
    return result;
  }

  void _skipInlineWhitespace() {
    while (!_isAtEnd() && (_peek() == ' ' || _peek() == '\t')) {
      _advance();
    }
  }

  void _skipBlankLinesAndWhitespace() {
    while (!_isAtEnd() && (_peek() == ' ' || _peek() == '\t' || _peek() == '\n')) {
      _advance();
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

  bool _isAtEnd() => _pos >= input.length;

  bool _isIdentifierStart(String char) =>
      RegExp(r'^[a-zA-Z]$').hasMatch(char);

  bool _isIdentifierPart(String char) =>
      RegExp(r'^[a-zA-Z0-9_-]$').hasMatch(char);

  bool _isDigit(String char) =>
      RegExp(r'^[0-9]$').hasMatch(char);
}
