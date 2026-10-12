import 'dart:io';
import 'package:args/args.dart';
import 'package:rustok_i18n/rustok_i18n.dart';

void main(List<String> arguments) {
  final parser = ArgParser()
    ..addCommand('check')
    ..addCommand('typegen');

  final checkParser = parser.commands['check']!
    ..addOption('dir', abbr: 'd', help: 'Directory containing .ftl files', mandatory: true)
    ..addOption('baseline', abbr: 'b', help: 'Baseline locale tag', defaultsTo: 'en')
    ..addFlag('recursive', abbr: 'r', help: 'Search subdirectories recursively', defaultsTo: false);

  final typegenParser = parser.commands['typegen']!
    ..addOption('input', abbr: 'i', help: 'Input .ftl file', mandatory: true)
    ..addOption('output', abbr: 'o', help: 'Output Dart file', mandatory: true)
    ..addOption('className', abbr: 'c', help: 'Generated class name', defaultsTo: 'AppMessages');

  if (arguments.isEmpty) {
    print('Usage: dart run rustok_i18n:i18n_cli <command> [options]');
    print(parser.usage);
    exit(1);
  }

  final results = parser.parse(arguments);
  final command = results.command;

  if (command == null) {
    print('Unknown command. Available commands: check, typegen');
    exit(1);
  }

  if (command.name == 'check') {
    _runCheck(command);
  } else if (command.name == 'typegen') {
    _runTypegen(command);
  }
}

/// Locale name of an `.ftl` file: `ru.ftl` -> `ru`.
String _localeOf(File file) {
  final name = file.uri.pathSegments.last;
  return name.endsWith('.ftl') ? name.substring(0, name.length - 4) : name;
}

/// Parses a file into a bundle. Junk entries and duplicate IDs are kept as diagnostics.
FluentBundle _loadBundle(File file, String locale) {
  final bundle = FluentBundle(locale: locale, useIsolating: false);
  bundle.addSource(file.readAsStringSync());
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

void _runCheck(ArgResults options) {
  final dirPath = options['dir'] as String;
  final baseline = options['baseline'] as String;
  final recursive = options['recursive'] as bool;
  final dir = Directory(dirPath);

  if (!dir.existsSync()) {
    print('Error: Directory "$dirPath" does not exist.');
    exit(1);
  }

  final ftlFiles = dir
      .listSync(recursive: recursive)
      .whereType<File>()
      .where((f) => f.path.endsWith('.ftl'))
      .toList();

  if (ftlFiles.isEmpty) {
    print('Warning: No .ftl files found in "$dirPath".');
    return;
  }

  final baselineFiles = ftlFiles.where((f) => _localeOf(f) == baseline).toList();
  if (baselineFiles.isEmpty) {
    print('Error: Baseline file "$baseline.ftl" not found in "$dirPath".');
    exit(1);
  }
  final baselineFile = baselineFiles.first;

  final baselineBundle = _loadBundle(baselineFile, baseline);
  final baselineKeys = baselineBundle.messageIds.toSet();

  print('=== RusToK i18n Parity Check ===');
  print('Baseline: ${baselineFile.path} (${baselineKeys.length} keys)');

  var failures = 0;

  if (baselineBundle.diagnostics.isNotEmpty) {
    failures++;
    print('  [FAIL] $baseline (baseline): invalid source');
    for (final diagnostic in baselineBundle.diagnostics) {
      print('    $diagnostic');
    }
  }

  for (final file in ftlFiles) {
    if (file.path == baselineFile.path) continue;
    final localeName = _localeOf(file);
    final bundle = _loadBundle(file, localeName);
    final keys = bundle.messageIds.toSet();
    final problems = <String>[];

    final missing = baselineKeys.difference(keys);
    final extra = keys.difference(baselineKeys);
    if (missing.isNotEmpty) {
      problems.add('Missing keys (${missing.length}): ${_preview(missing)}');
    }
    if (extra.isNotEmpty) {
      problems.add('Extra keys (${extra.length}): ${_preview(extra)}');
    }

    // Attributes are part of the contract too: a missing `.placeholder` is a parity bug.
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
      print('  [PASS] $localeName: ${keys.length} keys in full parity with baseline');
    } else {
      failures++;
      print('  [FAIL] $localeName: parity mismatch');
      for (final problem in problems) {
        print('    $problem');
      }
    }
  }

  if (failures > 0) {
    print('\nCheck failed with $failures mismatch(es).');
    exit(1);
  } else {
    print('\nAll locales are in full parity!');
  }
}

void _runTypegen(ArgResults options) {
  final inputPath = options['input'] as String;
  final outputPath = options['output'] as String;
  final className = options['className'] as String;

  if (!RegExp(r'^[A-Z][A-Za-z0-9_]*$').hasMatch(className)) {
    print('Error: "$className" is not a valid Dart class name.');
    exit(1);
  }

  final inputFile = File(inputPath);
  if (!inputFile.existsSync()) {
    print('Error: Input file "$inputPath" does not exist.');
    exit(1);
  }

  final bundle = _loadBundle(inputFile, 'generated');
  if (bundle.diagnostics.isNotEmpty) {
    print('Error: "$inputPath" contains invalid entries:');
    for (final diagnostic in bundle.diagnostics) {
      print('  $diagnostic');
    }
    exit(1);
  }

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
      print('Error: message keys "$owner" and "$key" both map to Dart member "$dartName".');
      exit(1);
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
        print('Error: variables in "$key" collide on Dart parameter "$param".');
        exit(1);
      }
      params[variable] = param;
    }

    if (params.isEmpty) {
      buffer.writeln('  String get $dartName => _messages.t(');
      buffer.writeln("        '$key',");
      buffer.writeln('        locale: locale,');
      buffer.writeln('      );');
    } else {
      final paramList = params.values.map((p) => 'required dynamic $p').join(', ');
      final argMap = params.entries.map((e) => "'${e.key}': ${e.value}").join(', ');
      buffer.writeln('  String $dartName({$paramList}) => _messages.t(');
      buffer.writeln("        '$key',");
      buffer.writeln('        locale: locale,');
      buffer.writeln('        args: {$argMap},');
      buffer.writeln('      );');
    }
    buffer.writeln();
    generated++;
  }

  buffer.writeln('}');

  File(outputPath)
    ..createSync(recursive: true)
    ..writeAsStringSync(buffer.toString());

  print('Successfully generated $className at "$outputPath" ($generated of '
      '${bundle.messageIds.length} keys; attribute-only messages are skipped).');
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
