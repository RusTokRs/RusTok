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

/// Parses [file] into a bundle. Parser errors are appended to [errors].
FluentBundle _loadBundle(File file, String locale, List<String> errors) {
  final bundle = FluentBundle(locale: locale, useIsolating: false);
  try {
    bundle.addSource(file.readAsStringSync());
  } on FtlParseException catch (e) {
    errors.add('Parse error in ${file.path}: $e');
  }
  return bundle;
}

Set<String> _attributeNames(FluentBundle bundle, String id) {
  final message = bundle.getMessage(id);
  if (message == null) return <String>{};
  return message.attributes.map((attr) => attr.id).toSet();
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

  final baselineErrors = <String>[];
  final baselineBundle = _loadBundle(baselineFile, baseline, baselineErrors);
  final baselineKeys = baselineBundle.messageIds.toSet();

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
    final bundle = _loadBundle(file, localeName, problems);
    final keys = bundle.messageIds.toSet();

    final missing = baselineKeys.difference(keys);
    final extra = keys.difference(baselineKeys);
    if (missing.isNotEmpty) {
      problems.add('Missing keys (${missing.length}): ${_preview(missing)}');
    }
    if (extra.isNotEmpty) {
      problems.add('Extra keys (${extra.length}): ${_preview(extra)}');
    }

    // Attributes are part of the contract: a missing `.placeholder` is a parity bug.
    for (final id in baselineKeys.toList()..sort()) {
      if (!keys.contains(id)) continue;
      final missingAttributes = _attributeNames(baselineBundle, id)
          .difference(_attributeNames(bundle, id));
      if (missingAttributes.isNotEmpty) {
        problems.add(
          'Message "$id" is missing attributes: ${_preview(missingAttributes)}',
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

  final inputFile = File(inputPath);
  if (!inputFile.existsSync()) {
    writeErr('Error: Input file "$inputPath" does not exist.');
    return exitFailure;
  }

  final errors = <String>[];
  final bundle = _loadBundle(inputFile, 'generated', errors);
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
      '(${source.generated} of ${bundle.messageIds.length} keys; '
      'attribute-only messages are skipped).');
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

/// Builds the typed accessor class for [bundle]. Returns `null` (and reports
/// through [writeErr]) when two keys or variables map to the same Dart name.
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

  for (final key in bundle.messageIds) {
    final dartName = _toDartMethodName(key);
    final owner = memberOwners[dartName];
    if (owner != null) {
      report('Error: message keys "$owner" and "$key" both map to Dart member "$dartName".');
      return null;
    }
    memberOwners[dartName] = key;

    final pattern = bundle.getMessage(key)?.value;
    if (pattern == null) {
      // Attribute-only messages have no main value to generate a getter for.
      continue;
    }

    final vars = <String>{};
    _collectVariables(pattern, vars);

    final params = <String, String>{};
    final seenParams = <String>{};
    for (final variable in vars) {
      final param = _toDartParamName(variable);
      if (!seenParams.add(param)) {
        report('Error: variables in "$key" collide on Dart parameter "$param".');
        return null;
      }
      params[variable] = param;
    }

    if (params.isEmpty) {
      buffer.writeln('  String get $dartName => _messages.t(');
      buffer.writeln("        '${_escapeSingleQuoted(key)}',");
      buffer.writeln('        locale: locale,');
      buffer.writeln('      );');
    } else {
      final paramList = params.values.map((p) => 'required dynamic $p').join(', ');
      final argMap = params.entries
          .map((e) => "'${_escapeSingleQuoted(e.key)}': ${e.value}")
          .join(', ');
      buffer.writeln('  String $dartName({$paramList}) => _messages.t(');
      buffer.writeln("        '${_escapeSingleQuoted(key)}',");
      buffer.writeln('        locale: locale,');
      buffer.writeln('        args: {$argMap},');
      buffer.writeln('      );');
    }
    buffer.writeln();
    generated++;
  }

  buffer.writeln('}');
  return GeneratedSource(buffer.toString(), generated);
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

void _collectVariables(FtlPattern pattern, Set<String> vars) {
  for (final elem in pattern.elements) {
    if (elem is FtlPlaceable) {
      _collectExpressionVariables(elem.expression, vars);
    }
  }
}

void _collectExpressionVariables(FtlExpression expr, Set<String> vars) {
  if (expr is FtlVariableReference) {
    vars.add(expr.id);
  } else if (expr is FtlFunctionReference) {
    for (final arg in expr.positional) {
      _collectExpressionVariables(arg, vars);
    }
  } else if (expr is FtlSelectExpression) {
    _collectExpressionVariables(expr.selector, vars);
    for (final variant in expr.variants) {
      _collectVariables(variant.value, vars);
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

String _toDartMethodName(String input) {
  var name = _toCamelCase(input);
  if (_dartReservedWords.contains(name) || RegExp(r'^[0-9]').hasMatch(name)) {
    name = '${name}Message';
  }
  return name;
}

String _toDartParamName(String input) {
  var name = _toCamelCase(input);
  if (_dartReservedWords.contains(name) || RegExp(r'^[0-9]').hasMatch(name)) {
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
