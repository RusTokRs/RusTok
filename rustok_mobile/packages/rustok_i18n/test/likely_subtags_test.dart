@TestOn('vm')
library;

import 'dart:convert';
import 'dart:io';
import 'package:test/test.dart';
import 'package:rustok_i18n/rustok_i18n.dart';

void main() {
  test('every pinned CLDR mapping maximizes and is idempotent', () {
    final source = jsonDecode(File('test/data/likely_subtags_cldr.json').readAsStringSync());
    final mappings = source['supplemental']['likelySubtags'] as Map<String, dynamic>;
    expect(mappings.length, 7788);
    for (final entry in mappings.entries) {
      final actual = maximizeLocale(LocaleTag.parse(entry.key));
      // LocaleTag canonicalizes legacy aliases (iw/in/ji) on input and output.
      expect(actual?.toLanguageTag(), LocaleTag.parse(entry.value as String).toLanguageTag(),
          reason: entry.key);
      expect(maximizeLocale(actual!), actual, reason: entry.key);
    }
  });
}
