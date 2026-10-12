import 'dart:io';

import 'package:rustok_i18n/rustok_i18n.dart';
import 'package:rustok_i18n/src/cli/i18n_cli.dart';
import 'package:test/test.dart';

class _Run {
  _Run(this.code, this.out, this.err);

  final int code;
  final List<String> out;
  final List<String> err;

  String get stdoutText => out.join('\n');
  String get stderrText => err.join('\n');
}

_Run _run(List<String> args) {
  final out = <String>[];
  final err = <String>[];
  final code = runI18nCli(args, out: out.add, err: err.add);
  return _Run(code, out, err);
}

const String _en = '''
welcome = Welcome
input = Search
    .placeholder = Type here
unread = { \$count ->
    [one] One message
   *[other] { \$count } messages
}
''';

const String _ru = '''
welcome = Добро пожаловать
input = Поиск
    .placeholder = Введите текст
unread = { \$count ->
    [one] Одно сообщение
   *[other] { \$count } сообщений
}
''';

void main() {
  late Directory tmp;

  setUp(() {
    tmp = Directory.systemTemp.createTempSync('rustok_i18n_cli_');
  });

  tearDown(() {
    if (tmp.existsSync()) tmp.deleteSync(recursive: true);
  });

  void writeFile(String name, String content) {
    File('${tmp.path}/$name').writeAsStringSync(content);
  }

  String path(String name) => '${tmp.path}/$name';

  group('check', () {
    test('passes when every locale matches the baseline', () {
      writeFile('en.ftl', _en);
      writeFile('ru.ftl', _ru);

      final result = _run(['check', '--dir', tmp.path, '--baseline', 'en']);

      expect(result.code, exitOk);
      expect(result.stdoutText, contains('[PASS] ru'));
      expect(result.stdoutText, contains('All locales are in full parity!'));
    });

    test('reports missing and extra keys', () {
      writeFile('en.ftl', _en);
      writeFile(
        'ru.ftl',
        _ru.replaceFirst('welcome = Добро пожаловать\n', '') +
            'extra-key = Лишнее\n',
      );

      final result = _run(['check', '--dir', tmp.path]);

      expect(result.code, exitFailure);
      expect(result.stdoutText, contains('Missing keys (1): welcome'));
      expect(result.stdoutText, contains('Extra keys (1): extra-key'));
    });

    test('reports a missing attribute even when the message exists', () {
      writeFile('en.ftl', _en);
      writeFile(
        'ru.ftl',
        _ru.replaceFirst('    .placeholder = Введите текст\n', ''),
      );

      final result = _run(['check', '--dir', tmp.path]);

      expect(result.code, exitFailure);
      expect(result.stdoutText,
          contains('Message "input" is missing attributes: placeholder'));
    });

    test('reports parser diagnostics in a locale file', () {
      writeFile('en.ftl', _en);
      writeFile('ru.ftl', '$_ru\nbroken line without equals\n');

      final result = _run(['check', '--dir', tmp.path]);

      expect(result.code, exitFailure);
      expect(result.stdoutText, contains('Junk'));
    });

    test('fails when the baseline file is absent', () {
      writeFile('ru.ftl', _ru);

      final result = _run(['check', '--dir', tmp.path, '--baseline', 'en']);

      expect(result.code, exitFailure);
      expect(result.stderrText, contains('Baseline file "en.ftl" not found'));
    });

    test('fails for a missing directory', () {
      final result = _run(['check', '--dir', path('nope')]);

      expect(result.code, exitFailure);
      expect(result.stderrText, contains('does not exist'));
    });

    test('warns and succeeds when there are no .ftl files', () {
      final result = _run(['check', '--dir', tmp.path]);

      expect(result.code, exitOk);
      expect(result.stdoutText, contains('No .ftl files found'));
    });
  });

  group('typegen', () {
    test('generates typed getters and parameterised methods', () {
      writeFile('en.ftl', _en);
      final output = path('messages.g.dart');

      final result = _run([
        'typegen',
        '--input', path('en.ftl'),
        '--output', output,
        '--className', 'AppMessages',
      ]);

      expect(result.code, exitOk);
      final code = File(output).readAsStringSync();
      expect(code, contains('class AppMessages {'));
      expect(code, contains('String get welcome => _messages.t('));
      expect(code, contains('String unread({required dynamic count}) => _messages.t('));
      expect(code, contains("args: {'count': count},"));
    });

    test('generates a parameter for each variable used in a message', () {
      final bundle = FluentBundle(locale: 'en', useIsolating: false)
        ..addSource('a = { \$x } and { \$y }\n');
      final source = generateTypedMessages(bundle, className: 'M');

      expect(source, isNotNull);
      expect(source!.code, contains('String a({required dynamic x, required dynamic y})'));
      expect(source.generated, 1);
    });

    test('rejects an invalid class name', () {
      writeFile('en.ftl', _en);

      final result = _run([
        'typegen',
        '--input', path('en.ftl'),
        '--output', path('out.dart'),
        '--className', 'lowerCase',
      ]);

      expect(result.code, exitFailure);
      expect(File(path('out.dart')).existsSync(), isFalse);
    });

    test('fails when two keys map to the same Dart member', () {
      writeFile('dup.ftl', 'a-b = one\na_b = two\n');

      final result = _run([
        'typegen',
        '--input', path('dup.ftl'),
        '--output', path('out.dart'),
      ]);

      expect(result.code, exitFailure);
      expect(result.stderrText, contains('both map to Dart member "aB"'));
    });

    test('fails on invalid input without writing output', () {
      writeFile('bad.ftl', 'broken line without equals\n');

      final result = _run([
        'typegen',
        '--input', path('bad.ftl'),
        '--output', path('out.dart'),
      ]);

      expect(result.code, exitFailure);
      expect(File(path('out.dart')).existsSync(), isFalse);
    });
  });

  group('argument handling', () {
    test('no arguments prints usage and returns the usage code', () {
      final result = _run([]);

      expect(result.code, exitUsage);
      expect(result.stdoutText, contains('Usage:'));
    });

    test('unknown command returns the usage code', () {
      final result = _run(['frobnicate']);

      expect(result.code, exitUsage);
    });

    test('missing mandatory option returns the usage code', () {
      final result = _run(['check']);

      expect(result.code, exitUsage);
      expect(result.stderrText, contains('Error:'));
    });
  });
}
