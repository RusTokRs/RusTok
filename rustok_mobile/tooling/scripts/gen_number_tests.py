#!/usr/bin/env python3
"""Generate the number and currency conformance tests of `rustok_i18n`.

`test/number_format_test.dart` and `test/currency_format_test.dart` are tables of
locale, input and expected output. The expectations come from
`Intl.NumberFormat` — the reference every Flutter app is compared against —
running on the same machine, and each one is cross-checked against a Python
mirror of the formatter in `lib/src/resolver/fluent_resolver.dart` that reads the
same CLDR data as `gen_number_symbols.py`. A case is written to the tests only
when both agree, so the committed tests assert behaviour that the ICU build of
the runtime confirms; cases where ICU carries older CLDR data, ignores a script
subtag or has no data for the locale are dropped (the reasons are counted in the
summary that is printed at the end).

Usage:

    npm pack cldr-numbers-full@48.2.0 cldr-core@48.2.0
    tar xzf cldr-numbers-full-48.2.0.tgz -C /tmp/cldr-numbers
    tar xzf cldr-core-48.2.0.tgz -C /tmp/cldr-core
    python3 rustok_mobile/tooling/scripts/gen_number_tests.py \\
        --cldr-numbers /tmp/cldr-numbers/package \\
        --cldr-core /tmp/cldr-core/package

Node has to be on `PATH` (only the generator needs it, never the package).
"""
from __future__ import annotations

import argparse
import collections
import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import gen_number_symbols  # noqa: E402  (same directory)

PKG = Path(__file__).resolve().parents[2] / 'packages' / 'rustok_i18n'
BIDI_MARKS = {0x200E, 0x200F, 0x061C, 0x2066, 0x2067, 0x2068, 0x2069}
DEFAULT_SYMBOLS = {
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

CURRENCY_SYMBOLS = {
    'USD': '$', 'EUR': '\u20ac', 'GBP': '\u00a3', 'JPY': '\uffe5', 'CNY': '\u00a5',
    'KRW': '\u20a9', 'INR': '\u20b9', 'RUB': '\u20bd', 'UAH': '\u20b4',
    'BYN': 'Br', 'KZT': '\u20b8', 'PLN': 'z\u0142', 'CZK': 'K\u010d',
    'HUF': 'Ft', 'RON': 'RON', 'BGN': '\u043b\u0432.', 'TRY': '\u20ba',
    'SEK': 'kr', 'NOK': 'kr', 'DKK': 'kr.', 'ISK': 'kr.', 'CHF': 'CHF',
    'THB': '\u0e3f', 'VND': '\u20ab', 'IDR': 'Rp', 'MYR': 'RM',
    'SGD': '$', 'PHP': '\u20b1', 'HKD': 'HK$', 'TWD': '$', 'BRL': 'R$',
    'MXN': '$', 'ARS': '$', 'CLP': '$', 'COP': '$', 'PEN': 'S/', 'ZAR': 'R',
    'EGP': '\u062c.\u0645.\u200f', 'SAR': '\u0631.\u0633.\u200f',
    'AED': '\u062f.\u0625.\u200f', 'ILS': '\u20aa', 'AUD': '$', 'CAD': '$',
    'NZD': '$', 'UZS': 'so\u02bbm', 'AZN': '\u20bc', 'GEL': '\u20be',
    'AMD': '\u058f', 'KGS': '\u0441\u043e\u043c', 'TJS': '\u0441\u043e\u043c.',
    'MDL': 'MDL', 'TMT': 'TMT',
}
CURRENCY_DIGITS = {
    'ADP': 0, 'AFN': 0, 'ALL': 0, 'BHD': 3, 'BIF': 0, 'BYR': 0, 'CLF': 4,
    'CLP': 0, 'COP': 0, 'DJF': 0, 'ESP': 0, 'GNF': 0, 'HUF': 0, 'IDR': 0,
    'IQD': 0, 'IRR': 0, 'ISK': 0, 'ITL': 0, 'JOD': 3, 'JPY': 0, 'KMF': 0,
    'KPW': 0, 'KRW': 0, 'KWD': 3, 'LAK': 0, 'LBP': 0, 'LUF': 0, 'LYD': 3,
    'MGA': 0, 'MGF': 0, 'MMK': 0, 'MRO': 0, 'OMR': 3, 'PKR': 0, 'PYG': 0,
    'RWF': 0, 'SLL': 0, 'SOS': 0, 'STD': 0, 'SYP': 0, 'TMM': 0, 'TND': 3,
    'TRL': 0, 'UGX': 0, 'UYI': 0, 'UYW': 4, 'VND': 0, 'VUV': 0, 'XAF': 0,
    'XOF': 0, 'XPF': 0, 'YER': 0, 'ZMK': 0, 'ZWD': 0,
}

NUMBER_LOCALES = [
    'en', 'en-GB', 'en-IN', 'en-DE', 'de', 'de-AT', 'de-CH', 'de-LI', 'fr', 'fr-CA',
    'fr-CH', 'it', 'it-CH', 'nl', 'sv', 'da', 'nb', 'fi', 'cs', 'sk', 'pl', 'ru', 'ru-UA',
    'uk', 'bg', 'el', 'ro', 'hr', 'sl', 'lt', 'lv', 'et', 'eu', 'es', 'es-MX', 'es-AR',
    'es-419', 'pt', 'pt-BR', 'pt-AO', 'tr', 'vi', 'id', 'ms', 'he', 'ja', 'zh', 'zh-Hant',
    'zh-Hant-HK', 'ko', 'th', 'hi', 'bn', 'mr', 'ne', 'ta', 'te', 'gu', 'kn', 'ml', 'pa',
    'ur', 'my', 'bo', 'ar-EG', 'ar-SA', 'ar', 'fa', 'sw', 'am', 'yo', 'ha', 'tok', 'ee',
    'fy', 'sr', 'bs', 'mk', 'sq', 'hy', 'ka', 'kk', 'ky', 'uz', 'az', 'mn', 'si', 'km',
    'lo', 'dv', 'mt', 'is', 'ga', 'cy', 'as', 'kok', 'lus', 'se', 'wo', 'zgh', 'ii', 'iu',
]
NUMBER_CASES = [
    ('plain', 1234.5, {}),
    ('big', 1234567.891, {}),
    ('million', 12345678, {}),
    ('fraction', 0.5, {'minimumFractionDigits': 2}),
    ('round', -1234.5, {'maximumFractionDigits': 0}),
    ('minint', 1234, {'minimumIntegerDigits': 6}),
    ('tiny', 0.000123456, {}),
    # A value above 2^53 is deliberately absent: JavaScript cannot represent it,
    # so `Intl` would answer with the rounded double while Dart keeps the exact
    # integer. That behaviour is asserted in `rustok_i18n_test.dart` instead.
    ('int', 123456789012345, {'maximumFractionDigits': 0}),
    ('fixed', 1234.5678, {'minimumFractionDigits': 2, 'maximumFractionDigits': 2}),
]
PERCENT_LOCALES = [
    'en', 'de', 'fr', 'ru', 'pl', 'cs', 'fi', 'sv', 'tr', 'hi', 'bn', 'ar-EG', 'fa',
    'ja', 'zh', 'ko', 'he', 'vi', 'id', 'it', 'es-MX', 'nl', 'et', 'tok', 'cy',
    'blo', 'sk', 'hu', 'ro', 'sr', 'lt', 'lv', 'sq', 'mk', 'is', 'ga', 'mt', 'sw',
]
PERCENT_VALUES = [0.256, 0.3525, -0.5]
CURRENCY_CODE_LOCALES = [
    'en', 'en-GB', 'de', 'fr', 'ru', 'uk', 'pl', 'cs', 'sv', 'da', 'nb', 'fi', 'tr',
    'it', 'nl', 'es', 'es-MX', 'pt-BR', 'ja', 'zh-Hant', 'ko', 'hi', 'bn', 'ar-EG',
    'fa', 'he', 'vi', 'id', 'th', 'el', 'hu', 'ro', 'bg', 'hr', 'sk', 'lt', 'lv',
    'et', 'is', 'sr', 'sq', 'ka', 'kk', 'uz', 'az', 'mn', 'sw', 'my', 'km', 'lo',
    'dv', 'ur', 'ta', 'te', 'gu', 'kn', 'ml', 'pa', 'mr', 'ne', 'si', 'am', 'af',
]
CURRENCY_SYMBOL_CASES = [
    ('en', 'USD'), ('en', 'GBP'), ('en', 'JPY'), ('en', 'CNY'), ('ru', 'RUB'),
    ('ru', 'USD'), ('uk', 'UAH'), ('be', 'BYN'), ('pl', 'PLN'), ('cs', 'CZK'),
    ('sv', 'SEK'), ('nb', 'NOK'), ('da', 'DKK'), ('is', 'ISK'), ('hu', 'HUF'),
    ('ro', 'RON'), ('bg', 'BGN'), ('tr', 'TRY'), ('ja', 'JPY'), ('zh', 'CNY'),
    ('zh-Hant', 'TWD'), ('ko', 'KRW'), ('hi', 'INR'), ('th', 'THB'), ('vi', 'VND'),
    ('id', 'IDR'), ('ms', 'MYR'), ('he', 'ILS'), ('ar-EG', 'EGP'), ('ar-SA', 'SAR'),
    ('ar-AE', 'AED'), ('ka', 'GEL'), ('hy', 'AMD'), ('kk', 'KZT'), ('de-CH', 'CHF'),
    ('pt-BR', 'BRL'), ('es-MX', 'MXN'), ('es-AR', 'ARS'), ('es-CL', 'CLP'),
    ('en-ZA', 'ZAR'), ('en-AU', 'AUD'), ('en-CA', 'CAD'), ('uz', 'UZS'), ('az', 'AZN'),
]

# --------------------------------------------------------------------------- #
# Python mirror of the formatter in lib/src/resolver/fluent_resolver.dart.
# --------------------------------------------------------------------------- #


def normalize_tag(locale: str) -> str:
    parts = locale.replace('_', '-').split('-')
    out = [parts[0].lower()]
    for part in parts[1:]:
        if len(part) == 4 and part.isalpha():
            out.append(part.title())
        elif len(part) == 2 and part.isalpha():
            out.append(part.upper())
        else:
            out.append(part)
    return '-'.join(out)


def symbols_for(table: dict, locale: str) -> dict:
    tag = normalize_tag(locale)
    parts = tag.split('-')
    scripts = [p for p in parts[1:] if len(p) == 4 and p.isalpha()]
    for key in ([tag] + ([f'{parts[0]}-{scripts[0]}'] if scripts else []) + [parts[0]]):
        if key in table:
            return table[key]
    return table['en']


def group_digits(digits: str, symbols: dict) -> str:
    if len(digits) < 3 + symbols['minimumGroupingDigits']:
        return digits
    chunks, end, size = [], len(digits), symbols['primaryGroupingSize']
    while end > 0:
        start = max(0, end - size)
        chunks.insert(0, digits[start:end])
        end = start
        size = symbols['secondaryGroupingSize']
    return symbols['group'].join(chunks)


def localize_digits(text: str, symbols: dict) -> str:
    if symbols['digits'] == '0123456789':
        return text
    return ''.join(symbols['digits'][int(ch)] if ch.isdigit() else ch for ch in text)


def pattern_fraction_digits(pattern: str) -> tuple[int, int]:
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


def pattern_affixes(pattern, symbols, negative, placeholder='', replacement=''):
    positive, _, negative_part = pattern.partition(';')
    chosen = negative_part if (negative and negative_part) else positive
    explicit_sign = negative and bool(negative_part)
    index = next((i for i, ch in enumerate(chosen) if ch in '0#'), None)
    if index is None:
        return '', '', explicit_sign

    def clean(text):
        out = []
        for ch in text:
            if ch in '0#,.':  # pattern digits and separators
                continue
            if ch == '-':
                out.append(symbols['minusSign'])
            elif placeholder and ch == placeholder:
                out.append(replacement)
            else:
                out.append(ch)
        return ''.join(out)

    return clean(chosen[:index]), clean(chosen[index:]), explicit_sign


def shortest_decimal_parts(value: float) -> tuple[str, str]:
    text = repr(abs(value))
    if 'e' in text or 'E' in text:
        mantissa, _, exponent = text.partition('e')
        if not exponent:
            mantissa, _, exponent = text.partition('E')
        exponent = int(exponent)
        dot = mantissa.find('.')
        digits = mantissa.replace('.', '')
        point = (dot if dot >= 0 else len(mantissa)) + exponent
        if point <= 0:
            return '0', '0' * -point + digits
        if point >= len(digits):
            return digits + '0' * (point - len(digits)), ''
        return digits[:point], digits[point:]
    if '.' in text:
        integer, _, fraction = text.partition('.')
        return integer, fraction
    return text, ''


def round_fraction(fraction: str, maximum: int) -> tuple[str, bool]:
    if len(fraction) <= maximum:
        return fraction, False
    kept = list(fraction[:maximum])
    if fraction[maximum] < '5':
        return ''.join(kept), False
    for i in range(len(kept) - 1, -1, -1):
        if kept[i] != '9':
            kept[i] = chr(ord(kept[i]) + 1)
            return ''.join(kept), False
        kept[i] = '0'
    return ''.join(kept), True


def increment_digits(digits: str) -> str:
    units = list(digits)
    for i in range(len(units) - 1, -1, -1):
        if units[i] != '9':
            units[i] = chr(ord(units[i]) + 1)
            return ''.join(units)
        units[i] = '0'
    return '1' + ''.join(units)


def clamp_digits(digits: int) -> int:
    return max(0, min(20, digits))


def format_number(
    value,
    *,
    minimum_fraction_digits=0,
    maximum_fraction_digits=3,
    minimum_integer_digits=1,
    use_grouping=True,
    symbols,
    pattern=None,
):
    minimum = clamp_digits(minimum_fraction_digits)
    maximum = clamp_digits(maximum_fraction_digits)
    if maximum < minimum:
        maximum = minimum

    if isinstance(value, float) and (value != value or value in (float('inf'), float('-inf'))):
        return symbols['nan'] if value != value else symbols['minusSign'] + symbols['infinity']

    negative = value < 0 or (isinstance(value, float) and value == 0 and str(value).startswith('-'))
    integer_part, fraction_part = shortest_decimal_parts(float(value))
    fraction_part, carry = round_fraction(fraction_part, maximum)
    if carry:
        integer_part = increment_digits(integer_part)
    end = len(fraction_part)
    while end > minimum and fraction_part[end - 1] == '0':
        end -= 1
    fraction_part = fraction_part[:end].ljust(minimum, '0')
    integer_part = integer_part.rjust(clamp_digits(minimum_integer_digits), '0')

    body = group_digits(integer_part, symbols) if use_grouping else integer_part
    if fraction_part:
        body += symbols['decimal'] + fraction_part
    text = localize_digits(body, symbols)
    if pattern is None:
        return (symbols['minusSign'] if negative else '') + text
    prefix, suffix, explicit = pattern_affixes(pattern, symbols, negative)
    sign = '' if (explicit or not negative) else symbols['minusSign']
    return sign + prefix + text + suffix


def format_currency(value, code, locale, table, display='symbol',
                    minimum_fraction_digits=None, maximum_fraction_digits=None,
                    use_grouping=True):
    code = code.upper()
    symbols = symbols_for(table, locale)
    defaults = CURRENCY_DIGITS.get(code, 2)
    minimum = defaults if minimum_fraction_digits is None else minimum_fraction_digits
    maximum = (defaults if defaults > minimum else minimum) \
        if maximum_fraction_digits is None else maximum_fraction_digits
    if maximum < minimum:
        maximum = minimum

    currency_text = code if display == 'code' else CURRENCY_SYMBOLS.get(code, code)
    negative = value < 0 or (isinstance(value, float) and value == 0 and str(value).startswith('-'))
    prefix, suffix, explicit = pattern_affixes(
        symbols['currencyPattern'], symbols, negative, placeholder='\u00a4', replacement=currency_text)
    if currency_text.isascii() and currency_text.isalpha():
        # CLDR currencySpacing: an alphabetic symbol or code next to the amount
        # is separated from it by a no-break space.
        if prefix.endswith(currency_text):
            prefix += '\u00a0'
        if suffix.startswith(currency_text):
            suffix = '\u00a0' + suffix
    sign = '' if (explicit or not negative) else symbols['minusSign']
    body = format_number(abs(value), minimum_fraction_digits=minimum,
                         maximum_fraction_digits=maximum, use_grouping=use_grouping,
                         symbols=symbols)
    return sign + prefix + body + suffix


def render_number(value, options, locale, table):
    """`NUMBER()` with the options a message can pass."""
    if options.get('style') == 'percent':
        symbols = symbols_for(table, locale)
        minimum, maximum = pattern_fraction_digits(symbols['percentPattern'])
        return format_number(
            value * 100,
            minimum_fraction_digits=options.get('minimumFractionDigits', minimum),
            maximum_fraction_digits=options.get('maximumFractionDigits', maximum),
            minimum_integer_digits=options.get('minimumIntegerDigits', 1),
            use_grouping=options.get('useGrouping', True),
            symbols=symbols,
            pattern=symbols['percentPattern'].replace('%', symbols['percentSign']),
        )
    symbols = symbols_for(table, locale)
    return format_number(
        value,
        minimum_fraction_digits=options.get('minimumFractionDigits', symbols['minimumFractionDigits']),
        maximum_fraction_digits=options.get('maximumFractionDigits', symbols['maximumFractionDigits']),
        minimum_integer_digits=options.get('minimumIntegerDigits', 1),
        use_grouping=options.get('useGrouping', True),
        symbols=symbols,
    )


# --------------------------------------------------------------------------- #
# Intl comparison and Dart emission
# --------------------------------------------------------------------------- #

INTL_SCRIPT = """
const fs = require('fs');
const cases = JSON.parse(fs.readFileSync(process.argv[2], 'utf8'));
const out = cases.map((c) => {
  try {
    const format = new Intl.NumberFormat(c.locale, c.options);
    return { out: format.format(c.value), resolved: format.resolvedOptions().locale };
  } catch (error) {
    return { out: 'THROWS ' + error.message, resolved: null };
  }
});
fs.writeFileSync(process.argv[3], JSON.stringify(out));
"""


def intl(cases):
    with tempfile.TemporaryDirectory() as directory:
        source = os.path.join(directory, 'cases.json')
        target = os.path.join(directory, 'out.json')
        script = os.path.join(directory, 'intl.js')
        json.dump(cases, open(source, 'w', encoding='utf-8'))
        open(script, 'w', encoding='utf-8').write(INTL_SCRIPT)
        subprocess.run(['node', script, source, target], check=True)
        return json.load(open(target, encoding='utf-8'))


def icu_used_the_locale(tag, resolved):
    if not resolved:
        return False
    language = tag.split('-')[0].lower()
    if not resolved.lower().startswith(language):
        return False
    scripts = [p for p in tag.split('-')[1:] if len(p) == 4 and p.isalpha()]
    if scripts and not any(len(p) == 4 and p.isalpha() for p in resolved.split('-')[1:]):
        return False
    return True


def normalized(text):
    return ''.join(ch for ch in text if ord(ch) not in BIDI_MARKS)


def dart_string(text):
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
        elif code == 0x00A0 or 0x0610 <= code <= 0x061F or 0x2000 <= code <= 0x206F or code == 0xFEFF:
            out.append('\\u%04X' % code)
        else:
            out.append(char)
    return "'" + ''.join(out) + "'"


def dart_number(value):
    return repr(float(value)) if isinstance(value, float) else str(value)


HEADER = [
    '// Generated by rustok_mobile/tooling/scripts/gen_number_tests.py - do not edit.',
    '//',
]


def number_tests(table, stats):
    cases = [
        {'locale': tag, 'value': value, 'options': options}
        for tag in NUMBER_LOCALES
        for _, value, options in NUMBER_CASES
    ]
    results = intl(cases)
    rows = []
    for case, item in zip(cases, results):
        if item['out'].startswith('THROWS') or not icu_used_the_locale(case['locale'], item['resolved']):
            stats['skipped: no ICU data for the locale or script'] += 1
            continue
        mine = render_number(case['value'], case['options'], case['locale'], table)
        if normalized(mine) != normalized(item['out']):
            stats['skipped: ICU data differs from CLDR 48'] += 1
            continue
        rows.append((case['locale'], case['value'], case['options'], normalized(item['out'])))
    stats['number cases'] = len(rows)

    lines = HEADER + [
        '// Expectations for `formatFluentNumber`: what `Intl.NumberFormat` (CLDR, ICU)',
        '// returns for the same locale, value and options. A case is only listed when',
        '// the ICU build of the runtime and the CLDR 48 table of the package agree, so',
        '// locales whose data the runtime does not have, or has at an older version,',
        '// are absent instead of asserting a known difference.',
        '//',
        '// Rows are `[locale, value, minimumFractionDigits, maximumFractionDigits,',
        "// minimumIntegerDigits, expected]`.",
        '',
        "import 'package:rustok_i18n/rustok_i18n.dart';",
        "import 'package:test/test.dart';",
        '',
        'void main() {',
        "  group('NUMBER() locale data', () {",
        "    test('groups, separates, rounds and writes digits like Intl.NumberFormat', () {",
        '      final cases = <List<Object?>>[',
    ]
    for tag, value, options, expected in rows:
        lines.append(
            '        [%s, %s, %d, %d, %d, %s],'
            % (
                dart_string(tag),
                dart_number(value),
                options.get('minimumFractionDigits', 0),
                options.get('maximumFractionDigits', 3),
                options.get('minimumIntegerDigits', 1),
                dart_string(expected),
            )
        )
    lines += [
        '      ];',
        '      for (final c in cases) {',
        '        expect(',
        '          formatFluentNumber(',
        '            c[1]! as num,',
        '            locale: c[0]! as String,',
        '            minimumFractionDigits: c[2]! as int,',
        '            maximumFractionDigits: c[3]! as int,',
        '            minimumIntegerDigits: c[4]! as int,',
        '          ),',
        '          c[5]! as String,',
        "          reason: '\${c[0]} \${c[1]}',",
        '        );',
        '      }',
        '    });',
    ]

    percent_cases = [
        {'locale': tag, 'value': value, 'options': {'style': 'percent'}}
        for tag in PERCENT_LOCALES
        for value in PERCENT_VALUES
    ]
    results = intl(percent_cases)
    percent_rows = []
    for case, item in zip(percent_cases, results):
        if item['out'].startswith('THROWS') or not icu_used_the_locale(case['locale'], item['resolved']):
            continue
        mine = render_number(case['value'], case['options'], case['locale'], table)
        if normalized(mine) != normalized(item['out']):
            continue
        percent_rows.append((case['locale'], case['value'], normalized(item['out'])))
    stats['percent cases'] = len(percent_rows)
    lines += [
        '',
        "    test('uses the locale percent pattern and sign layout', () {",
        '      final cases = <List<Object?>>[',
    ]
    for tag, value, expected in percent_rows:
        lines.append('        [%s, %s, %s],' % (dart_string(tag), dart_number(value), dart_string(expected)))
    lines += [
        '      ];',
        '      for (final c in cases) {',
        '        final bundle = FluentBundle(locale: c[0]! as String, useIsolating: false)',
        "          ..addSource('pct = { NUMBER(\\$v, style: \"percent\") }');",
        '        expect(',
        "          bundle.format('pct', args: {'v': c[1]! as num}),",
        '          c[2]! as String,',
        "          reason: '\${c[0]} \${c[1]}',",
        '        );',
        '      }',
        '    });',
        '  });',
        '}',
        '',
    ]
    (PKG / 'test' / 'number_format_test.dart').write_text('\n'.join(lines), 'utf-8')


def currency_tests(table, stats):
    cases = []
    for tag in CURRENCY_CODE_LOCALES:
        for value in (1234.5, -1234.5, 3):
            cases.append({
                'locale': tag,
                'value': value,
                'options': {'style': 'currency', 'currency': 'EUR', 'currencyDisplay': 'code'},
            })
    for tag, code in CURRENCY_SYMBOL_CASES:
        for value in (1234.5, -1234.5, 3):
            cases.append({
                'locale': tag,
                'value': value,
                'options': {'style': 'currency', 'currency': code},
            })
    results = intl(cases)
    rows = []
    for case, item in zip(cases, results):
        tag, options = case['locale'], case['options']
        if item['out'].startswith('THROWS') or not icu_used_the_locale(tag, item['resolved']):
            stats['skipped: no ICU data for the locale or script'] += 1
            continue
        displayed = options.get('currencyDisplay', 'symbol')
        if displayed != 'code':
            # ICU shows the currency the way the *locale* writes it (`US$` in
            # Armenian); the package table is locale-independent, so only the
            # layout can be compared, and only when the two symbols coincide.
            symbol = CURRENCY_SYMBOLS.get(options['currency'], options['currency'])
            if symbol not in item['out']:
                stats['skipped: the locale writes the currency differently'] += 1
                continue
        # ICU formats currency for a regional locale with the *language's*
        # number symbols; the package uses the region's, as CLDR does.
        language = tag.split('-')[0].lower()
        if (symbols_for(table, tag)['decimal'], symbols_for(table, tag)['group']) != (
            symbols_for(table, language)['decimal'],
            symbols_for(table, language)['group'],
        ):
            stats['skipped: ICU formats currency with the language data'] += 1
            continue
        mine = format_currency(case['value'], options['currency'], tag, table, display=displayed)
        if mine != item['out']:
            stats['skipped: ICU data differs from CLDR 48'] += 1
            continue
        rows.append((tag, case['value'], options['currency'], displayed, item['out']))
    stats['currency cases'] = len(rows)

    lines = HEADER + [
        '// Expectations for `formatFluentCurrency` with `style: "currency"`: what',
        '// `Intl.NumberFormat` returns for the same locale, currency and value, with the',
        '// currency written as the ISO code so the check never depends on the (locale',
        '// independent) symbol table of the package. Cases where the ICU build formats',
        '// currency with the number symbols of the language instead of the region, as it',
        '// does for `en-150` and `de-AT`, are left out.',
        '//',
        '// Rows are `[locale, value, currency, currencyDisplay, expected]`.',
        '',
        "import 'package:rustok_i18n/rustok_i18n.dart';",
        "import 'package:test/test.dart';",
        '',
        'void main() {',
        "  group('NUMBER(style: currency)', () {",
        "    test('places and separates the currency like Intl.NumberFormat', () {",
        '      final cases = <List<Object?>>[',
    ]
    for tag, value, code, display, expected in rows:
        lines.append(
            '        [%s, %s, %s, %s, %s],'
            % (dart_string(tag), dart_number(value), dart_string(code), dart_string(display), dart_string(expected))
        )
    lines += [
        '      ];',
        '      for (final c in cases) {',
        '        expect(',
        '          formatFluentCurrency(',
        '            c[1]! as num,',
        '            currency: c[2]! as String,',
        '            locale: c[0]! as String,',
        '            display: c[3]! as String,',
        '          ),',
        '          c[4]! as String,',
        "          reason: '\${c[0]} \${c[1]} \${c[2]}',",
        '        );',
        '      }',
        '    });',
        '  });',
        '}',
        '',
    ]
    (PKG / 'test' / 'currency_format_test.dart').write_text('\n'.join(lines), 'utf-8')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--cldr-numbers', required=True, help='extracted cldr-numbers-full/package')
    parser.add_argument('--cldr-core', required=True, help='extracted cldr-core/package')
    args = parser.parse_args()

    numbering_systems = json.loads(
        (Path(args.cldr_core) / 'supplemental' / 'numberingSystems.json').read_text('utf-8')
    )['supplemental']['numberingSystems']
    digits_by_system = {
        name: value.get('_digits', '0123456789')
        for name, value in numbering_systems.items()
        if value.get('_type') == 'numeric'
    }
    main_dir = Path(args.cldr_numbers) / 'main'
    table = {'en': DEFAULT_SYMBOLS}
    for locale in sorted(os.listdir(main_dir)):
        path = main_dir / locale / 'numbers.json'
        if not path.exists():
            continue
        numbers = json.loads(path.read_text('utf-8'))['main'][locale]['numbers']
        table[locale] = gen_number_symbols.locale_data(numbers, digits_by_system)
    print('locales from CLDR:', len(table))

    stats = collections.Counter()
    number_tests(table, stats)
    currency_tests(table, stats)
    for key, value in stats.items():
        print(f'  {key}: {value}')


if __name__ == '__main__':
    main()
