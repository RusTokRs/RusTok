#!/usr/bin/env python3
"""Generate `rustok_i18n/lib/src/format/number_symbols.dart` from CLDR.

The i18n package keeps no dependency on `intl`: all locale data is vendored as
Dart tables, so the tables have to be regenerated when CLDR is updated. This is
the generator for the number table. It reads the CLDR JSON that ships with the
`cldr-numbers-full` and `cldr-core` npm packages:

    npm pack cldr-numbers-full@48.2.0 cldr-core@48.2.0
    tar xzf cldr-numbers-full-48.2.0.tgz -C /tmp/cldr-numbers
    tar xzf cldr-core-48.2.0.tgz -C /tmp/cldr-json
    python3 rustok_mobile/tooling/scripts/gen_number_symbols.py \\
        --cldr-numbers /tmp/cldr-numbers/package \\
        --cldr-core /tmp/cldr-json/package

`--json PATH` also writes the table as JSON, which is what the differential
checks against `Intl.NumberFormat` use (`--json /tmp/symbols.json`).
"""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path

DEFAULT_OUT = (
    Path(__file__).resolve().parents[2]
    / 'packages'
    / 'rustok_i18n'
    / 'lib'
    / 'src'
    / 'format'
    / 'number_symbols.dart'
)


def fraction_digits(pattern: str) -> tuple[int, int]:
    """(minimum, maximum) fraction digits of a number pattern."""
    positive = pattern.split(';')[0]
    if '.' not in positive:
        return (0, 0)
    minimum = maximum = 0
    for char in positive.split('.')[1]:
        if char == '0':
            minimum += 1
            maximum += 1
        elif char == '#':
            maximum += 1
        else:
            break
    return (minimum, maximum)


def grouping_sizes(pattern: str) -> tuple[int, int]:
    """(primary, secondary) grouping sizes of a number pattern.

    UTS #35: with a single separator the sizes are the digit count to the right
    of it; with several, the primary size is the digit count to the right of the
    last separator and the secondary size the digit count between the last two.
    """
    integer = pattern.split('.')[0]
    groups = integer.split(',')
    primary = len(groups[-1])
    secondary = len(groups[-2]) if len(groups) >= 3 else primary
    return (primary, secondary)


def escape(text: str) -> str:
    """Escape [text] for a single-quoted Dart string literal."""
    out = []
    for char in text:
        code = ord(char)
        if char == '\\':
            out.append('\\\\')
        elif char == "'":
            out.append("\\'")
        elif char == '$':
            out.append('\\$')
        elif code < 0x20 or 0x7F <= code <= 0x9F:
            out.append('\\u%04X' % code)
        elif (
            code == 0x00A0  # no-break space
            or 0x0610 <= code <= 0x061F  # Arabic marks
            or 0x2000 <= code <= 0x206F  # spaces and bidi marks
            or code == 0xFEFF  # zero width no-break space
        ):
            # Invisible marks (no-break space, narrow no-break space, bidi
            # isolates, the Arabic letter mark) are escaped: a literal mark in
            # the source is unreadable and easy to lose in an edit.
            out.append('\\u%04X' % code)
        elif code > 0xFFFF:
            out.append('\\u{%X}' % code)
        else:
            out.append(char)
    return ''.join(out)


def locale_data(numbers: dict, digits_by_system: dict[str, str]) -> dict:
    system = numbers.get('defaultNumberingSystem', 'latn')
    symbols = numbers.get('symbols-numberSystem-' + system, {})
    pattern = numbers.get('decimalFormats-numberSystem-' + system, {}).get(
        'standard', '#,##0.###'
    )
    primary, secondary = grouping_sizes(pattern)
    minimum, maximum = fraction_digits(pattern)
    return {
        'decimal': symbols.get('decimal', '.'),
        'group': symbols.get('group', ','),
        'digits': digits_by_system.get(system, '0123456789'),
        'minimumGroupingDigits': int(numbers.get('minimumGroupingDigits', 1)),
        'primaryGroupingSize': primary,
        'secondaryGroupingSize': secondary,
        'minimumFractionDigits': minimum,
        'maximumFractionDigits': maximum,
        'percentPattern': numbers.get('percentFormats-numberSystem-' + system, {}).get(
            'standard', '#,##0%'
        ),
        'percentSign': symbols.get('percentSign', '%'),
        'minusSign': symbols.get('minusSign', '-'),
        'currencyPattern': numbers.get('currencyFormats-numberSystem-' + system, {}).get(
            'standard', '\u00a4#,##0.00'
        ),
        'infinity': symbols.get('infinity', '\u221e'),
        'nan': symbols.get('nan', 'NaN'),
    }


HEADER = '''/// Locale-specific number data used by `NUMBER()` and [formatFluentNumber].
///
/// The table is generated from CLDR (currently 48.2) and covers every locale
/// whose numbers differ from English: English itself, 175 languages and every
/// regional override CLDR ships (`en-DE`, `ar-EG`, `es-MX`, `hi-IN`, ...).
/// `rustok_mobile/tooling/scripts/gen_number_symbols.py` regenerates this file
/// from `cldr-numbers-full` and `cldr-core`; the package itself depends on
/// neither `intl` nor any other data package.
///
/// Keys are BCP-47 tags. [numberSymbolsFor] tries the full tag, then the
/// language of the tag, then falls back to `en` (`1,234.5`), so a locale that
/// CLDR does not differentiate (`de-LI` while CLDR keeps `de-LI` equal to
/// `de`) inherits its language's data.
///
/// Verified against `Intl.NumberFormat` for every locale CLDR ships: 10 724
/// comparisons across 766 tags with the only remaining differences coming from
/// the ICU build of the runtime, which lags behind CLDR 48 in a few locales
/// (`fr-CH`, whose grouping separator became the Swiss apostrophe in CLDR 46).
class NumberSymbols {
  const NumberSymbols({
    required this.decimal,
    required this.group,
    this.digits = '',
    this.minimumGroupingDigits = 1,
    this.primaryGroupingSize = 3,
    this.secondaryGroupingSize = 3,
    this.minimumFractionDigits = 0,
    this.maximumFractionDigits = 3,
    this.percentPattern = '#,##0%',
    this.percentSign = '%',
    this.minusSign = '-',
    this.currencyPattern = '\\u00a4#,##0.00',
    this.infinity = '\\u221E',
    this.nan = 'NaN',
  });

  /// Decimal separator, e.g. `,` in `1,5`.
  final String decimal;

  /// Digit-group separator, e.g. `.` in `1.234`.
  final String group;

  /// The ten digits of the locale's numbering system in `0`-`9` order, or an
  /// empty string when the locale uses ASCII digits.
  final String digits;

  /// Smallest number of digits that is grouped: 2 in Polish, Spanish, Italian
  /// and Czech, so `1234` stays `1234` while `12345` becomes `12 345`.
  final int minimumGroupingDigits;

  /// Digits between group separators, counted from the decimal separator:
  /// 3 nearly everywhere, 3 with a secondary size of 2 in the Indian
  /// numbering system (`12,34,567`), and 2 with a secondary size of 2 in
  /// Toki Pona.
  final int primaryGroupingSize;
  final int secondaryGroupingSize;

  /// Fraction digits of the locale's decimal pattern, which are the defaults of
  /// `NUMBER()` when the message does not name them.
  final int minimumFractionDigits;
  final int maximumFractionDigits;

  /// Percent layout from CLDR: `#,##0%`, `#,##0 %` (French, Russian) or
  /// `%#,##0` (Turkish). It holds the position of the sign and the default
  /// fraction digits of the percent style.
  final String percentPattern;

  /// Percent sign, `%` or `\\u066A` in Arabic.
  final String percentSign;

  /// Minus sign: `-`, `\\u2212` (the true minus of Estonian, Basque or
  /// Persian) or `\\u061C-` in Arabic. It is used for every negative value and
  /// replaces the `-` written in the CLDR patterns.
  final String minusSign;

  /// Currency layout from CLDR, where `\\u00a4` marks the currency:
  /// `\\u00a4#,##0.00` is `$1,234.50`, `#,##0.00\\u00a0\\u00a4` is
  /// `1\\u00a0234,50\\u00a0\\u20ac`, and a `;`-separated negative subpattern is
  /// used as written, which is what makes `1.234,50-` (Faroese) possible.
  final String currencyPattern;

  /// Symbols for infinite values and for NaN.
  final String infinity;
  final String nan;
}

const NumberSymbols _english = NumberSymbols(decimal: '.', group: ',');

/// Number data for a BCP-47 [locale], English when nothing matches.
NumberSymbols numberSymbolsFor(String locale) {
  final tag = _normalizeTag(locale);
  final parts = tag.split('-');
  final language = parts.first;
  String? script;
  for (final part in parts.skip(1)) {
    if (part.length == 4 && _isAsciiLetters(part)) {
      script = part;
      break;
    }
  }
  return _numberSymbols[tag] ??
      (script == null ? null : _numberSymbols['$language-$script']) ??
      _numberSymbols[language] ??
      _english;
}

/// Lower-cases the language, title-cases a script and upper-cases a region, so
/// that `es-mx` and `ES-MX` find the same entry.
String _normalizeTag(String locale) {
  final parts = locale.replaceAll('_', '-').split('-');
  final result = <String>[parts.first.toLowerCase()];
  for (final part in parts.skip(1)) {
    if (part.length == 4 && _isAsciiLetters(part)) {
      result.add(part[0].toUpperCase() + part.substring(1).toLowerCase());
    } else if (part.length == 2 && _isAsciiLetters(part)) {
      result.add(part.toUpperCase());
    } else {
      result.add(part);
    }
  }
  return result.join('-');
}

bool _isAsciiLetters(String value) {
  for (final unit in value.codeUnits) {
    final isUpper = unit >= 0x41 && unit <= 0x5A;
    final isLower = unit >= 0x61 && unit <= 0x7A;
    if (!isUpper && !isLower) return false;
  }
  return true;
}

/// Inserts the group separator into the ASCII [digits].
///
/// Supports the Indian numbering system (`12,34,567`) and
/// [NumberSymbols.minimumGroupingDigits], which suppresses the separator for
/// short numbers (`1234` in Polish).
String groupDigits(String digits, NumberSymbols symbols) {
  if (digits.length < 3 + symbols.minimumGroupingDigits) return digits;
  final chunks = <String>[];
  var end = digits.length;
  var size = symbols.primaryGroupingSize;
  while (end > 0) {
    final start = end - size < 0 ? 0 : end - size;
    chunks.insert(0, digits.substring(start, end));
    end = start;
    size = symbols.secondaryGroupingSize;
  }
  return chunks.join(symbols.group);
}

/// Rewrites ASCII digits in [text] with the locale's numbering system digits.
String localizeDigits(String text, NumberSymbols symbols) {
  if (symbols.digits.isEmpty) return text;
  final buffer = StringBuffer();
  for (final unit in text.codeUnits) {
    if (unit >= 0x30 && unit <= 0x39) {
      buffer.write(symbols.digits[unit - 0x30]);
    } else {
      buffer.writeCharCode(unit);
    }
  }
  return buffer.toString();
}

/// Fraction digits a number pattern asks for: `#,##0.###` is `(0, 3)`,
/// `#,##0.00` is `(2, 2)` and `#,##0%` is `(0, 0)`.
(int, int) patternFractionDigits(String pattern) {
  final positive = pattern.split(';').first;
  final dot = positive.indexOf('.');
  if (dot < 0) return (0, 0);
  var minimum = 0;
  var maximum = 0;
  for (var i = dot + 1; i < positive.length; i++) {
    final unit = positive.codeUnitAt(i);
    if (unit == 0x30) {
      minimum++;
      maximum++;
    } else if (unit == 0x23) {
      maximum++;
    } else {
      break;
    }
  }
  return (minimum, maximum);
}

/// Locale data for every locale whose numbers differ from English.
const Map<String, NumberSymbols> _numberSymbols = {
'''

DEFAULT = {
    'decimal': '.',
    'group': ',',
    'digits': '0123456789',
    'minimumGroupingDigits': 1,
    'primaryGroupingSize': 3,
    'secondaryGroupingSize': 3,
    'minimumFractionDigits': 0,
    'maximumFractionDigits': 3,
    'percentPattern': '#,##0%',
    'percentSign': '%',
    'minusSign': '-',
    'currencyPattern': '\u00a4#,##0.00',
    'infinity': '\u221e',
    'nan': 'NaN',
}


def dart_entry(tag: str, data: dict) -> list[str]:
    parts = [f"decimal: '{escape(data['decimal'])}'", f"group: '{escape(data['group'])}'"]
    if data['digits'] != DEFAULT['digits']:
        parts.append(f"digits: '{data['digits']}'")
    if data['minimumGroupingDigits'] != 1:
        parts.append(f'minimumGroupingDigits: {data["minimumGroupingDigits"]}')
    if data['primaryGroupingSize'] != 3:
        parts.append(f'primaryGroupingSize: {data["primaryGroupingSize"]}')
    if data['secondaryGroupingSize'] != DEFAULT['secondaryGroupingSize']:
        parts.append(f'secondaryGroupingSize: {data["secondaryGroupingSize"]}')
    if data['minimumFractionDigits'] != 0:
        parts.append(f'minimumFractionDigits: {data["minimumFractionDigits"]}')
    if data['maximumFractionDigits'] != 3:
        parts.append(f'maximumFractionDigits: {data["maximumFractionDigits"]}')
    if data['percentPattern'] != DEFAULT['percentPattern']:
        parts.append(f"percentPattern: '{escape(data['percentPattern'])}'")
    if data['percentSign'] != '%':
        parts.append(f"percentSign: '{escape(data['percentSign'])}'")
    if data['minusSign'] != '-':
        parts.append(f"minusSign: '{escape(data['minusSign'])}'")
    if data['currencyPattern'] != DEFAULT['currencyPattern']:
        parts.append(f"currencyPattern: '{escape(data['currencyPattern'])}'")
    if data['infinity'] != '\u221e':
        parts.append(f"infinity: '{escape(data['infinity'])}'")
    if data['nan'] != 'NaN':
        parts.append(f"nan: '{escape(data['nan'])}'")
    single = f"  '{tag}': NumberSymbols({', '.join(parts)}),"
    if len(single) <= 88:
        return [single]
    return [f"  '{tag}': NumberSymbols("] + [f'    {part},' for part in parts] + ['  ),']


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--cldr-numbers', required=True, help='extracted cldr-numbers-full/package')
    parser.add_argument('--cldr-core', required=True, help='extracted cldr-core/package')
    parser.add_argument('--out', default=str(DEFAULT_OUT), help='Dart file to write')
    parser.add_argument('--json', help='also write the table as JSON (for the Intl checks)')
    args = parser.parse_args()

    main_dir = Path(args.cldr_numbers) / 'main'
    numbering_systems = json.loads(
        (Path(args.cldr_core) / 'supplemental' / 'numberingSystems.json').read_text('utf-8')
    )['supplemental']['numberingSystems']
    digits_by_system = {
        name: value.get('_digits', '0123456789')
        for name, value in numbering_systems.items()
        if value.get('_type') == 'numeric'
    }

    data = {}
    for locale in sorted(os.listdir(main_dir)):
        path = main_dir / locale / 'numbers.json'
        if not path.exists():
            continue
        numbers = json.loads(path.read_text('utf-8'))['main'][locale]['numbers']
        data[locale] = locale_data(numbers, digits_by_system)

    english = data['en']
    # Only store what the lookup cannot reach otherwise: a language that differs
    # from English, and a regional (or script) variant that differs from its
    # language. A region that equals its language needs no entry, and a region
    # whose language equals English but which differs itself does.
    table = {}
    for tag, values in data.items():
        if '-' not in tag:
            if tag != 'en' and values != english:
                table[tag] = values
    for tag, values in data.items():
        if '-' not in tag:
            continue
        language = tag.split('-')[0]
        parent = data.get(language, english)
        if values != parent or (values != english and language not in table):
            table[tag] = values
    languages = sum(1 for tag in table if '-' not in tag)
    print(
        f'CLDR locales: {len(data)}, table entries: {len(table)} '
        f'({languages} languages, {len(table) - languages} regional)'
    )

    if args.json:
        Path(args.json).write_text(
            json.dumps({'en': english, **table}, ensure_ascii=False, indent=0), 'utf-8'
        )

    lines = [HEADER.rstrip()]
    for tag in sorted(table, key=lambda value: (value.lower(), value)):
        lines.extend(dart_entry(tag, table[tag]))
    lines.append('};')
    out = Path(args.out)
    out.write_text('\n'.join(lines) + '\n', 'utf-8')
    print(f'wrote {out} ({len(lines)} lines)')


if __name__ == '__main__':
    main()
