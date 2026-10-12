import 'dart:io';

import 'package:args/args.dart';

import '../ast/ast_nodes.dart';
import '../parser/ftl_parser.dart';
import '../resolver/fluent_resolver.dart';

/// Exit codes returned by [runI18nCli].
const int exitOk = 0;
const int exitFailure = 1;
const int exitUsage = 64;

/// Receives one line of CLI output.
typedef CliOutput = void Function(String line);

/// Runs the `i18n_cli` commands and returns the process exit code.
///
/// Commands:
/// * `check --dir <dir> [--baseline en] [--recursive]`: verifies every locale
///   file has the same message IDs and attributes as the baseline.
/// * `typegen --input <file.ftl> --output <file.dart> [--className AppMessages]`:
///   generates a typed accessor class.
///
/// This library uses `dart:io`, so it is not exported from `package:rustok_i18n`.
/// Import it from `bin/` or tests only.
int runI18nCli(
  List<String> arguments, {
  CliOutput? out,
  CliOutput? err,
}) {
  final write = out ?? _printLine;
  final writeErr = err ?? _printLine;

  final parser = ArgParser()
    ..addCommand('check')
    ..addCommand('typegen');

  parser.commands['check']!
    ..addOption('dir', abbr: 'd', help: 'Directory containing .ftl files', mandatory: true)
    ..addOption('baseline', abbr: 'b', help: 'Baseline locale tag', defaultsTo: 'en')
    ..addFlag('recursive', abbr: 'r', help: 'Search subdirectories recursively', defaultsTo: false);

  parser.commands['typegen']!
    ..addOption('input', abbr: 'i', help: 'Input .ftl file', mandatory: true)
    ..addOption('output', abbr: 'o', help: 'Output Dart file', mandatory: true)
    ..addOption('className', abbr: 'c', help: 'Generated class name', defaultsTo: 'AppMessages');

  if (arguments.isEmpty) {
    write('Usage: dart run rustok_i18n:i18n_cli <command> [options]');
    write(parser.usage);
    return exitUsage;
  }

  final ArgResults results;
  try {
    results = parser.parse(arguments);
  } on Exception catch (e) {
    writeErr('Error: $e');
    write('Usage: dart run rustok_i18n:i18n_cli <command> [options]');
    write(parser.usage);
    return exitUsage;
  }

  final command = results.command;
  if (command == null) {
    writeErr('Unknown command. Available commands: check, typegen');
    return exitUsage;
  }

  if (command.name == 'check') {
    return _runCheck(command, write, writeErr);
  }
  return _runTypegen(command, write, writeErr);
}

void _printLine(String line) => stdout.writeln(line);

/// Locale name of an `.ftl` file: `ru.ftl` -> `ru`.
String _localeOf(File file) {
  final name = file.uri.pathSegments.last;
  return name.endsWith('.ftl') ? name.substring(0, name.length - 4) : name;
}

/// Parses [source] (read from [label]) into a bundle.
///
/// Parser errors are appended to [errors] together with the file label.
FluentBundle _loadBundle(
  String source,
  String locale,
  String label,
  List<String> errors,
) {
  final bundle = FluentBundle(locale: locale, useIsolating: false);
  try {
    bundle.addSource(source);
  } on FtlParseException catch (e) {
    errors.add('Parse error in $label: $e');
  }
  return bundle;
}

/// The key and attribute contract declared by one locale source.
///
/// The contract is read from the parsed AST rather than from a loaded
/// [FluentBundle]: a bundle indexes messages, but a message may validly have
/// attributes and no value (`key =` followed by `.attribute = value`), and the
/// contract has to cover those attributes too.
class _SourceContract {
  const _SourceContract(this.keys, this.attributes);

  final Set<String> keys;
  final Map<String, Set<String>> attributes;

  Set<String> attributesOf(String id) => attributes[id] ?? const <String>{};
}

_SourceContract _contractOf(String source) {
  final messages = FtlParser.parse(source).messages.values;
  return _SourceContract(
    messages.map((message) => message.id).toSet(),
    {
      for (final message in messages)
        message.id: message.attributes.map((attribute) => attribute.id).toSet(),
    },
  );
}

/// First few sorted items of [items], for compact reporting.
String _preview(Set<String> items) {
  final sorted = items.toList()..sort();
  final shown = sorted.take(5).join(', ');
  return sorted.length > 5 ? '$shown, ...' : shown;
}

int _runCheck(ArgResults options, CliOutput write, CliOutput writeErr) {
  final dirPath = options['dir'] as String;
  final baseline = options['baseline'] as String;
  final recursive = options['recursive'] as bool;
  final dir = Directory(dirPath);

  if (!dir.existsSync()) {
    writeErr('Error: Directory "$dirPath" does not exist.');
    return exitFailure;
  }

  final ftlFiles = dir
      .listSync(recursive: recursive)
      .whereType<File>()
      .where((f) => f.path.endsWith('.ftl'))
      .toList()
    ..sort((a, b) => a.path.compareTo(b.path));

  if (ftlFiles.isEmpty) {
    write('Warning: No .ftl files found in "$dirPath".');
    return exitOk;
  }

  final baselineFiles = ftlFiles.where((f) => _localeOf(f) == baseline).toList();
  if (baselineFiles.isEmpty) {
    writeErr('Error: Baseline file "$baseline.ftl" not found in "$dirPath".');
    return exitFailure;
  }
  final baselineFile = baselineFiles.first;
  if (baselineFiles.length > 1) {
    final others = baselineFiles
        .skip(1)
        .map((file) => file.path)
        .join(', ');
    write('Warning: ${baselineFiles.length} "$baseline.ftl" files found; '
        'using ${baselineFile.path} (also found: $others).');
  }

  final baselineErrors = <String>[];
  final baselineSource = baselineFile.readAsStringSync();
  final baselineBundle =
      _loadBundle(baselineSource, baseline, baselineFile.path, baselineErrors);
  final baselineContract = _contractOf(baselineSource);
  final baselineKeys = baselineContract.keys;

  write('=== RusToK i18n Parity Check ===');
  write('Baseline: ${baselineFile.path} (${baselineKeys.length} keys)');

  var failures = 0;

  final baselineProblems = [...baselineErrors, ...baselineBundle.diagnostics];
  if (baselineProblems.isNotEmpty) {
    failures++;
    write('  [FAIL] $baseline (baseline): invalid source');
    for (final problem in baselineProblems) {
      write('    $problem');
    }
  }

  for (final file in ftlFiles) {
    if (file.path == baselineFile.path) continue;
    final localeName = _localeOf(file);
    final problems = <String>[];
    final source = file.readAsStringSync();
    final bundle = _loadBundle(source, localeName, file.path, problems);
    final contract = _contractOf(source);
    final keys = contract.keys;

    final missing = baselineKeys.difference(keys);
    final extra = keys.difference(baselineKeys);
    if (missing.isNotEmpty) {
      problems.add('Missing keys (${missing.length}): ${_preview(missing)}');
    }
    if (extra.isNotEmpty) {
      problems.add('Extra keys (${extra.length}): ${_preview(extra)}');
    }

    // Attributes are part of the contract: a missing `.placeholder` is a parity
    // bug, and an attribute the baseline does not define is one too.
    for (final id in baselineKeys.toList()..sort()) {
      if (!keys.contains(id)) continue;
      final expected = baselineContract.attributesOf(id);
      final actual = contract.attributesOf(id);
      final missingAttributes = expected.difference(actual);
      if (missingAttributes.isNotEmpty) {
        problems.add(
          'Message "$id" is missing attributes: ${_preview(missingAttributes)}',
        );
      }
      final extraAttributes = actual.difference(expected);
      if (extraAttributes.isNotEmpty) {
        problems.add(
          'Message "$id" has extra attributes: ${_preview(extraAttributes)}',
        );
      }
    }

    problems.addAll(bundle.diagnostics);

    if (problems.isEmpty) {
      write('  [PASS] $localeName: ${keys.length} keys in full parity with baseline');
    } else {
      failures++;
      write('  [FAIL] $localeName: parity mismatch');
      for (final problem in problems) {
        write('    $problem');
      }
    }
  }

  if (failures > 0) {
    write('\nCheck failed with $failures mismatch(es).');
    return exitFailure;
  }
  write('\nAll locales are in full parity!');
  return exitOk;
}

int _runTypegen(ArgResults options, CliOutput write, CliOutput writeErr) {
  final inputPath = options['input'] as String;
  final outputPath = options['output'] as String;
  final className = options['className'] as String;

  if (!RegExp(r'^[A-Z][A-Za-z0-9_]*$').hasMatch(className)) {
    writeErr('Error: "$className" is not a valid Dart class name.');
    return exitFailure;
  }

  if (_reservedClassNames.contains(className)) {
    writeErr('Error: "$className" is already the name of a type the generated '
        'file uses; choose another class name.');
    return exitFailure;
  }

  final inputFile = File(inputPath);
  if (!inputFile.existsSync()) {
    writeErr('Error: Input file "$inputPath" does not exist.');
    return exitFailure;
  }

  final errors = <String>[];
  final bundle =
      _loadBundle(inputFile.readAsStringSync(), 'generated', inputPath, errors);
  final problems = [...errors, ...bundle.diagnostics];
  if (problems.isNotEmpty) {
    writeErr('Error: "$inputPath" contains invalid entries:');
    for (final problem in problems) {
      writeErr('  $problem');
    }
    return exitFailure;
  }

  final source = generateTypedMessages(bundle, className: className, writeErr: writeErr);
  if (source == null) return exitFailure;

  try {
    File(outputPath)
      ..createSync(recursive: true)
      ..writeAsStringSync(source.code);
  } on FileSystemException catch (e) {
    writeErr('Error: cannot write "$outputPath": ${e.message}');
    return exitFailure;
  }

  write('Successfully generated $className at "$outputPath" '
      '(accessors: ${source.generated}, keys: ${bundle.messageIds.length}; '
      'a message without a value contributes attribute getters only).');
  return exitOk;
}

/// Result of [generateTypedMessages].
class GeneratedSource {
  const GeneratedSource(this.code, this.generated);

  /// Full Dart source of the generated file.
  final String code;

  /// Number of message getters or methods generated.
  final int generated;
}

/// Typed accessor for one message value or attribute.
class _TypedAccessor {
  const _TypedAccessor({
    required this.member,
    required this.attribute,
    required this.pattern,
  });

  /// Dart member name, e.g. `welcome` or `welcomeAriaLabel`.
  final String member;

  /// Attribute name, or `null` for the message value itself.
  final String? attribute;

  final FtlPattern pattern;

  /// Human-readable name used in error messages.
  String describe(String key) =>
      attribute == null ? 'message key "$key"' : 'attribute "$key.$attribute"';
}

/// Builds the typed accessor class for [bundle]. Returns `null` (and reports
/// through [writeErr]) when two keys or variables map to the same Dart name.
///
/// Every message value becomes a getter — or a method when the pattern uses
/// variables — and every attribute becomes an additional accessor that calls
/// `tAttr`, so `input.placeholder` is reachable as `inputPlaceholder`. A
/// translator comment (`# ...` directly above the entry) becomes a doc comment.
///
/// The parameters of a method are the variables the resolver reads from the
/// caller's scope: those written in the pattern plus those of every message it
/// references, because a message reference is resolved in the caller's scope.
/// Term references are not followed — a term is resolved with its own scope.
/// Members that would collide with the generated class itself (`locale`), with
/// `Object` (`toString`, `hashCode`, `runtimeType`, `noSuchMethod`) or with a
/// Dart keyword get a `Message` suffix; a `locale` parameter becomes
/// `localeArg`, so it cannot shadow the field passed as the bundle locale.
GeneratedSource? generateTypedMessages(
  FluentBundle bundle, {
  required String className,
  CliOutput? writeErr,
}) {
  final report = writeErr ?? _printLine;
  final buffer = StringBuffer();
  final memberOwners = <String, String>{};
  var generated = 0;

  buffer.writeln('// Auto-generated by rustok_i18n typegen. DO NOT EDIT DIRECTLY.');
  buffer.writeln("import 'package:rustok_i18n/rustok_i18n.dart';");
  buffer.writeln();
  buffer.writeln('class $className {');
  buffer.writeln('  const $className(this._messages, {this.locale});');
  buffer.writeln();
  buffer.writeln('  final UiMessages _messages;');
  buffer.writeln('  final String? locale;');
  buffer.writeln();

  void writeDocComment(String? comment) {
    if (comment == null) return;
    final lines = comment.split('\n');
    if (lines.every((line) => line.trim().isEmpty)) return;
    for (final line in lines) {
      buffer.writeln('  /// ${line.replaceFirst(RegExp(r'[ \t]+$'), '')}');
    }
  }

  for (final key in bundle.messageIds) {
    final message = bundle.getMessage(key);
    if (message == null) continue;

    final dartName = _toDartMethodName(key);
    final accessors = <_TypedAccessor>[];
    if (message.value != null) {
      accessors.add(_TypedAccessor(
        member: dartName,
        attribute: null,
        pattern: message.value!,
      ));
    }
    for (final attribute in message.attributes) {
      accessors.add(_TypedAccessor(
        member: _reserveMemberName(
          '$dartName${_capitalize(_toDartParamName(attribute.id))}',
        ),
        attribute: attribute.id,
        pattern: attribute.value,
      ));
    }

    writeDocComment(message.comment);
    for (final accessor in accessors) {
      final owner = memberOwners[accessor.member];
      if (owner != null) {
        report('Error: $owner and ${accessor.describe(key)} both map to '
            'Dart member "${accessor.member}".');
        return null;
      }
      memberOwners[accessor.member] = accessor.describe(key);

      final vars = <String>{};
      _collectVariables(accessor.pattern, bundle, vars, <String>{});

      final params = <String, String>{};
      final seenParams = <String>{};
      for (final variable in vars) {
        final param = _toDartParamName(variable);
        if (!seenParams.add(param)) {
          report('Error: variables in "${accessor.member}" collide on Dart '
              'parameter "$param".');
          return null;
        }
        params[variable] = param;
      }

      final call = accessor.attribute == null ? 't' : 'tAttr';
      if (params.isEmpty) {
        buffer.writeln('  String get ${accessor.member} => _messages.$call(');
      } else {
        final paramList = params.values.map((p) => 'required dynamic $p').join(', ');
        buffer.writeln('  String ${accessor.member}({$paramList}) => _messages.$call(');
      }
      buffer.writeln("        '${_escapeSingleQuoted(key)}',");
      if (accessor.attribute != null) {
        buffer.writeln("        '${_escapeSingleQuoted(accessor.attribute!)}',");
      }
      buffer.writeln('        locale: locale,');
      if (params.isNotEmpty) {
        final argMap = params.entries
            .map((e) => "'${_escapeSingleQuoted(e.key)}': ${e.value}")
            .join(', ');
        buffer.writeln('        args: {$argMap},');
      }
      buffer.writeln('      );');
      buffer.writeln();
      generated++;
    }
  }

  buffer.writeln('}');
  return GeneratedSource(buffer.toString(), generated);
}

/// Upper-cases the first character of [input], for member names such as
/// `inputPlaceholder`.
String _capitalize(String input) {
  if (input.isEmpty) return input;
  return input[0].toUpperCase() + input.substring(1);
}

/// Escapes [input] for use inside a single-quoted Dart string literal.
/// Backslash, single quote and dollar sign are prefixed with a backslash.
String _escapeSingleQuoted(String input) {
  const int backslash = 0x5C;
  const int singleQuote = 0x27;
  const int dollar = 0x24;
  final buffer = StringBuffer();
  for (final unit in input.codeUnits) {
    if (unit == backslash || unit == singleQuote || unit == dollar) {
      buffer.writeCharCode(backslash);
    }
    buffer.writeCharCode(unit);
  }
  return buffer.toString();
}

/// Collects the variables [pattern] reads from the caller's scope into [vars].
///
/// [visited] carries the message entries already followed, so a reference cycle
/// (`a = { b }` next to `b = { a }`) stays finite; pass a fresh set per accessor.
void _collectVariables(
  FtlPattern pattern,
  FluentBundle bundle,
  Set<String> vars,
  Set<String> visited,
) {
  for (final elem in pattern.elements) {
    if (elem is FtlPlaceable) {
      _collectExpressionVariables(elem.expression, bundle, vars, visited);
    }
  }
}

void _collectExpressionVariables(
  FtlExpression expr,
  FluentBundle bundle,
  Set<String> vars,
  Set<String> visited,
) {
  if (expr is FtlVariableReference) {
    vars.add(expr.id);
  } else if (expr is FtlPlaceableExpression) {
    _collectExpressionVariables(expr.expression, bundle, vars, visited);
  } else if (expr is FtlFunctionReference) {
    for (final arg in expr.positional) {
      _collectExpressionVariables(arg, bundle, vars, visited);
    }
    // A named argument is a string or number literal by the grammar, so it can
    // never carry a variable; walking them anyway keeps the generated signature
    // correct if the parser ever accepts a wider form.
    for (final arg in expr.named.values) {
      _collectExpressionVariables(arg, bundle, vars, visited);
    }
  } else if (expr is FtlTermReference) {
    // A term is resolved with its own scope, so the variables inside its
    // pattern are not read from the caller; only the arguments written at the
    // call site are.
    for (final arg in expr.positionalArguments) {
      _collectExpressionVariables(arg, bundle, vars, visited);
    }
    for (final arg in (expr.arguments ?? const <String, FtlExpression>{}).values) {
      _collectExpressionVariables(arg, bundle, vars, visited);
    }
  } else if (expr is FtlMessageReference) {
    // A message reference is resolved in the caller's scope, so every variable
    // its pattern reads is a variable the generated method must accept.
    final suffix = expr.attribute != null ? '.${expr.attribute}' : '';
    if (!visited.add('${expr.id}$suffix')) return;
    final message = bundle.getMessage(expr.id);
    if (message == null) return;
    final pattern = expr.attribute != null
        ? message.findAttribute(expr.attribute!)?.value
        : message.value;
    if (pattern != null) _collectVariables(pattern, bundle, vars, visited);
  } else if (expr is FtlSelectExpression) {
    _collectExpressionVariables(expr.selector, bundle, vars, visited);
    for (final variant in expr.variants) {
      _collectVariables(variant.value, bundle, vars, visited);
    }
  }
}

const Set<String> _dartReservedWords = {
  'assert', 'break', 'case', 'catch', 'class', 'const', 'continue',
  'default', 'do', 'else', 'enum', 'extends', 'false', 'final',
  'finally', 'for', 'if', 'in', 'is', 'new', 'null', 'rethrow',
  'return', 'super', 'switch', 'this', 'throw', 'true', 'try',
  'var', 'void', 'while', 'with',
  'abstract', 'as', 'covariant', 'deferred', 'dynamic', 'export',
  'extension', 'external', 'factory', 'function', 'get', 'implements',
  'import', 'interface', 'late', 'library', 'mixin', 'operator',
  'part', 'required', 'set', 'static', 'typedef',
};

/// Class names the generated file cannot use: it imports `UiMessages` and holds
/// one as a field, so a same-named class would shadow the type it needs.
const Set<String> _reservedClassNames = {'UiMessages'};

/// Members the generated class must not declare. `locale` is the field the
/// generated bodies pass as the bundle locale; the rest come from `Object`,
/// where a `String` getter is not a valid override.
const Set<String> _reservedMemberNames = {
  'locale',
  'toString',
  'hashCode',
  'runtimeType',
  'noSuchMethod',
};

/// Parameter names the generated methods must not use: a parameter called
/// `locale` would shadow the field of the same name, so the accessor would pass
/// the message argument as the bundle locale instead of the instance locale.
const Set<String> _reservedParamNames = {'locale'};

/// Appends `Message` when [name] cannot be declared as a member: a Dart keyword,
/// a member of the generated class or of `Object`, or a leading digit.
String _reserveMemberName(String name) {
  if (_dartReservedWords.contains(name) ||
      _reservedMemberNames.contains(name) ||
      RegExp(r'^[0-9]').hasMatch(name)) {
    return '${name}Message';
  }
  return name;
}

String _toDartMethodName(String input) => _reserveMemberName(_toCamelCase(input));

String _toDartParamName(String input) {
  var name = _toCamelCase(input);
  if (_dartReservedWords.contains(name) ||
      _reservedParamNames.contains(name) ||
      RegExp(r'^[0-9]').hasMatch(name)) {
    name = '${name}Arg';
  }
  return name;
}

String _toCamelCase(String input) {
  final parts = input.split(RegExp(r'[-_.]'));
  if (parts.isEmpty) return input;
  final first = parts.first.toLowerCase();
  final rest = parts.skip(1).map((p) {
    if (p.isEmpty) return '';
    return p[0].toUpperCase() + p.substring(1);
  }).join('');
  return '$first$rest';
}
