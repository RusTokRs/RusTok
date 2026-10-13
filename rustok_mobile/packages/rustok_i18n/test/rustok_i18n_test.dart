import 'package:test/test.dart';
import 'package:rustok_i18n/rustok_i18n.dart';

void main() {
  group('Symmetric CLDR regional catalog fallback', () {
    test('explicit default script retains regional catalog before generic parents', () {
      expect(localeFallbackChain('en-Latn-GB'),
          ['en-Latn-GB', 'en-GB', 'en-Latn', 'en']);
      expect(localeFallbackChain('zh-Hant-TW'),
          ['zh-Hant-TW', 'zh-TW', 'zh-Hant', 'zh']);
    });

    test('equivalent spellings preserve variants in both directions', () {
      expect(localeFallbackChain('de-Latn-DE-1996'), [
        'de-Latn-DE-1996', 'de-DE-1996', 'de-Latn-DE', 'de-DE', 'de-Latn', 'de',
      ]);
      expect(localeFallbackChain('de-DE-1996'), [
        'de-DE-1996', 'de-Latn-DE-1996', 'de-DE', 'de-Latn-DE', 'de-Latn', 'de',
      ]);
    });

    test('conflicting or unknown regional scripts cannot lose explicit script', () {
      expect(localeFallbackChain('az-Latn-IR'), ['az-Latn-IR', 'az-Latn', 'az']);
      expect(localeFallbackChain('qaa-Arab-AQ'), ['qaa-Arab-AQ', 'qaa-Arab', 'qaa']);
      expect(localeFallbackChain('en-Cyrl-US'), ['en-Cyrl-US', 'en-Cyrl', 'en']);
    });

    test('exact regional spelling wins and the equivalent keeps fallback provenance', () {
      final catalog = StaticCatalog({
        'en': {'greet': 'generic', 'exact': 'generic'},
        'en-GB': {'greet': 'British', 'exact': 'regional'},
        'en-Latn-GB': {'exact': 'explicit'},
      });
      final messages = UiMessages(catalog: catalog);
      final result = messages.resolve('greet', locale: 'en-Latn-GB');
      expect(result.value, 'British');
      expect(result.resolvedLocale, 'en-GB');
      expect(result.provenance, ResolutionProvenance.parentLanguage);
      expect(messages.resolve('exact', locale: 'en-Latn-GB').value, 'explicit');
      expect(messages.resolve('exact', locale: 'en-GB').value, 'regional');
      expect(messages.forLocale('en-Latn-GB').t('greet'), 'British');
    });

    test('locale negotiation retains exclusions and candidate weights', () {
      expect(negotiateLocale(acceptLanguageHeader: 'en-Latn-GB',
          supportedLocales: ['en', 'en-GB']), 'en-GB');
      expect(negotiateLocale(acceptLanguageHeader: 'en-Latn-GB, en-GB;q=0, fr;q=0.8',
          supportedLocales: ['en-GB', 'fr']), 'fr');
      expect(negotiateLocale(acceptLanguageHeader: 'en-Latn-GB, en-GB;q=0.2, fr;q=0.8',
          supportedLocales: ['en-GB', 'fr']), 'fr');
    });

    test('lazy preparation loads equivalent regional resources only once', () async {
      final requested = <String>[];
      final catalog = LazyCatalog(loader: (locale) {
        requested.add(locale);
        return locale == 'en-GB' ? 'greet = British' : null;
      });
      await catalog.loadForLocale('en-Latn-GB');
      expect(requested, ['en-Latn-GB', 'en-GB', 'en-Latn', 'en']);
      expect(UiMessages(catalog: catalog).t('greet', locale: 'en-Latn-GB'), 'British');
      await catalog.loadForLocale('en-GB');
      expect(requested, hasLength(4));
    });

    test('regional bundle also controls date formatting after lookup', () {
      const source = 'day = { DATETIME(\$date, dateStyle: "short") }\n';
      final catalog = FluentCatalog(useIsolating: false)
        ..registerFtl('en', source)
        ..registerFtl('en-GB', source);
      final messages = UiMessages(catalog: catalog);
      expect(messages.t('day', locale: 'en-Latn-GB',
          args: {'date': DateTime.utc(2026, 9, 5)}), '05/09/2026');
    });

    test('candidate snapshots stay unique and immutable after expansion', () {
      final candidates = localeCandidates(locale: 'en-Latn-GB', defaultLocale: 'en-GB');
      expect(candidates, ['en-Latn-GB', 'en-GB', 'en-Latn', 'en']);
      expect(() => candidates.add('fr'), throwsUnsupportedError);
    });
  });

  group('CLDR locale maximization', () {
    test('lookup precedence distinguishes full, script and region mappings', () {
      for (final entry in {
        'und-Arab-TR': 'apc-Arab-TR',
        'und-Thai-TR': 'th-Thai-TR',
        'und-Zzzz-TR': 'tr-Latn-TR',
        'und-AQ': 'en-Latn-AQ',
      }.entries) {
        expect(maximizeLocale(LocaleTag.parse(entry.key))?.toLanguageTag(),
            entry.value, reason: entry.key);
      }
    });
    test('unknown script marker does not mask regional script fallback', () {
      expect(localeFallbackChain('pa-Zzzz-PK'),
          ['pa-Zzzz-PK', 'pa-PK', 'pa-Arab-PK', 'pa-Arab', 'pa']);
      expect(negotiateLocale(acceptLanguageHeader: 'pa-Zzzz-PK',
          supportedLocales: ['pa-Guru', 'pa-Arab', 'en']), 'pa-Arab');
      expect(LocaleTag.parse('pa-Zzzz-PK').isRtl, isTrue);
    });
    test('unknown region does not add an inferred real-region catalog', () {
      expect(localeFallbackChain('en-ZZ'), ['en-ZZ', 'en']);
      expect(localeFallbackChain('en-Zzzz-ZZ'), ['en-Zzzz-ZZ', 'en']);
      expect(localeFallbackChain('pa-Arab-ZZ'), ['pa-Arab-ZZ', 'pa-Arab', 'pa']);
      expect(localeFallbackChain('qaa-Zzzz-AQ'), ['qaa-Zzzz-AQ', 'qaa-AQ', 'qaa']);
    });
    test('literal unknown-marker catalogs still have exact-match priority', () {
      final messages = UiMessages(catalog: StaticCatalog({
        'pa-Zzzz-PK': {'greet': 'exact'}, 'pa-Arab': {'greet': 'script'},
      }));
      final resolved = messages.resolve('greet', locale: 'pa-Zzzz-PK');
      expect(resolved.value, 'exact');
      expect(resolved.provenance, ResolutionProvenance.exactMatch);
    });
    test('lazy preparation and resolution share cleaned fallback candidates', () async {
      final requested = <String>[];
      final catalog = LazyCatalog(loader: (locale) {
        requested.add(locale);
        return locale == 'pa-Arab' ? 'greet = Script' : null;
      });
      await catalog.loadForLocale('pa-Zzzz-PK');
      expect(requested, ['pa-Zzzz-PK', 'pa-PK', 'pa-Arab-PK', 'pa-Arab', 'pa', 'en']);
      final result = UiMessages(catalog: catalog).resolve('greet', locale: 'pa-Zzzz-PK');
      expect(result.value, 'Script');
      expect(result.provenance, ResolutionProvenance.parentLanguage);
      await catalog.loadForLocale('pa-Zzzz-PK');
      expect(requested, hasLength(6));
    });

    test('fills missing components beyond the former regional shortlist', () {
      for (final entry in {
        'az-IR': 'az-Arab-IR', 'pa-PK': 'pa-Arab-PK',
        'mn-CN': 'mn-Mong-CN', 'und-TW': 'zh-Hant-TW',
        'und-Arab': 'ar-Arab-EG', 'en': 'en-Latn-US',
        'en-Zzzz-ZZ': 'en-Latn-US',
      }.entries) {
        expect(maximizeLocale(LocaleTag.parse(entry.key))?.toLanguageTag(),
            entry.value, reason: entry.key);
      }
    });
    test('explicit components are preserved even against regional defaults', () {
      expect(maximizeLocale(LocaleTag.parse('az-Latn-IR'))?.toLanguageTag(), 'az-Latn-IR');
      expect(maximizeLocale(LocaleTag.parse('en-Cyrl'))?.toLanguageTag(), 'en-Cyrl-US');
      expect(maximizeLocale(LocaleTag.parse('en-AQ'))?.toLanguageTag(), 'en-Latn-AQ');
    });
    test('unknown language is not silently assigned English defaults', () {
      expect(maximizeLocale(LocaleTag.parse('qaa')), isNull);
      expect(maximizeLocale(LocaleTag.parse('qaa-Arab')), isNull);
      expect(maximizeLocale(LocaleTag.parse('qaa-Arab-AQ'))?.toLanguageTag(), 'qaa-Arab-AQ');
    });
    test('maximization preserves extensions and variants without mutating identity', () {
      final input = LocaleTag.parse('de-1996-u-nu-latn-x-test');
      final result = maximizeLocale(input)!;
      expect(result.toLanguageTag(), 'de-Latn-DE-1996-u-nu-latn-x-test');
      expect(input.toCatalogTag(), 'de-1996');
      expect(() => result.variants.add('rozaj'), throwsUnsupportedError);
      expect(() => result.extensions['a'] = 'test', throwsUnsupportedError);
    });
    test('regional catalog lookup retains exact then inferred then generic order', () {
      expect(localeFallbackChain('pa-PK'), ['pa-PK', 'pa-Arab-PK', 'pa-Arab', 'pa']);
      expect(localeFallbackChain('en-GB'), ['en-GB', 'en-Latn-GB', 'en-Latn', 'en']);
      expect(localeFallbackChain('en'), ['en']);
      expect(localeFallbackChain('qaa-AQ'), ['qaa-AQ', 'qaa']);
    });
    test('negotiation rejects mismatched regional scripts beyond old shortlist', () {
      expect(negotiateLocale(acceptLanguageHeader: 'az-Latn, en;q=0.8',
          supportedLocales: ['az-IR', 'en']), 'en');
      expect(negotiateLocale(acceptLanguageHeader: 'pa-PK',
          supportedLocales: ['pa-Guru', 'pa-Arab', 'en']), 'pa-Arab');
    });
    test('language-only catalogs remain generic rather than renamed', () {
      expect(negotiateLocale(acceptLanguageHeader: 'pa-PK',
          supportedLocales: ['pa', 'en']), 'pa');
      final catalog = StaticCatalog({'pa-PK': {'greet': 'exact'}, 'pa-Arab': {'greet': 'script'}});
      expect(UiMessages(catalog: catalog).t('greet', locale: 'pa-PK'), 'exact');
      expect(catalog.getSupportedLocales(), {'pa-PK', 'pa-Arab'});
    });
    test('direction follows inferred regional script with explicit override', () {
      expect(LocaleTag.parse('az-IR').isRtl, isTrue);
      expect(LocaleTag.parse('pa-PK').isRtl, isTrue);
      expect(LocaleTag.parse('az-Latn-IR').isRtl, isFalse);
      expect(LocaleTag.parse('az').isRtl, isFalse);
      expect(detectTextDirection(language: 'en', script: 'Gara').isRtl, isTrue);
      expect(detectTextDirection(language: 'en', script: 'Chrs').isRtl, isTrue);
    });
  });

  group('Negotiation with region-implied scripts', () {
    test('Montenegrin Serbian expands to Latin per CLDR', () {
      expect(localeFallbackChain('sr-ME'), ['sr-ME', 'sr-Latn-ME', 'sr-Latn', 'sr']);
      expect(negotiateLocale(
        acceptLanguageHeader: 'sr-Latn', supportedLocales: ['en', 'sr-ME'],
      ), 'sr-ME');
      expect(negotiateLocale(
        acceptLanguageHeader: 'sr-Cyrl, ru;q=0.8',
        supportedLocales: ['sr-ME', 'ru'],
      ), 'ru');
    });

    test('explicit traditional Chinese does not select simplified regional sibling', () {
      expect(negotiateLocale(
        acceptLanguageHeader: 'zh-Hant, ru;q=0.8',
        supportedLocales: ['zh-CN', 'ru'],
      ), 'ru');
    });

    test('regional request does not select conflicting explicit script', () {
      expect(negotiateLocale(
        acceptLanguageHeader: 'zh-TW, ru;q=0.8',
        supportedLocales: ['zh-Hans', 'ru'],
      ), 'ru');
    });

    test('regional siblings compare both inferred scripts', () {
      expect(negotiateLocale(
        acceptLanguageHeader: 'zh-TW',
        supportedLocales: ['zh-CN', 'zh-HK', 'en'],
      ), 'zh-HK');
    });

    test('explicit Serbian Latin does not select implicit Cyrillic', () {
      expect(negotiateLocale(
        acceptLanguageHeader: 'sr-Latn, ru;q=0.8',
        supportedLocales: ['sr-RS', 'ru'],
      ), 'ru');
    });

    test('explicit scripts override regional defaults and preserve original spelling', () {
      expect(negotiateLocale(
        acceptLanguageHeader: 'sr-Latn',
        supportedLocales: ['sr-RS', 'SR_latn_RS'],
      ), 'SR_latn_RS');
    });

    test('parent catalog stays preferable to compatible regional sibling', () {
      expect(negotiateLocale(
        acceptLanguageHeader: 'zh-TW',
        supportedLocales: ['zh-HK', 'zh-Hant'],
      ), 'zh-Hant');
      expect(negotiateLocale(
        acceptLanguageHeader: 'zh-Hant',
        supportedLocales: ['zh', 'en'],
      ), 'zh');
    });

    test('extension text never becomes an inferred script', () {
      expect(negotiateLocale(
        acceptLanguageHeader: 'sr-x-latn',
        supportedLocales: ['sr-Cyrl', 'en'],
      ), 'sr-Cyrl');
    });

    test('script filtering does not override explicitly configured default fallback', () {
      expect(negotiateLocale(
        acceptLanguageHeader: 'zh-Hant',
        supportedLocales: ['zh-CN', 'en'],
        defaultLocale: 'en',
      ), 'en');
      expect(negotiateLocale(
        acceptLanguageHeader: 'zh-Hant',
        supportedLocales: ['zh-CN', 'en'],
        defaultLocale: 'zh-CN',
      ), 'zh-CN');
    });
  });

  group('Quoted number patterns and currency boundaries', () {
    test('quoted digits and punctuation are not part of the numeric skeleton', () {
      expect(formatFluentNumber(12.5, pattern: "'v2.0: '0.00' items, #0.'"),
          'v2.0: 12.5 items, #0.');
      expect(patternFractionDigits("'v2.0: '0.00' items, #0.'"), (2, 2));
      expect(patternFractionDigits("'0.000'0.0##;0.0000"), (1, 3));
    });

    test('quoted semicolons do not select a negative subpattern', () {
      const pattern = "'a;b '0;'negative; '0";
      expect(formatFluentNumber(3, pattern: pattern), 'a;b 3');
      expect(formatFluentNumber(-3, pattern: pattern), 'negative; 3');
      expect(formatFluentNumber(-3, pattern: "'a;b '0"), '-a;b 3');
    });

    test('doubled apostrophes emit one literal apostrophe', () {
      expect(formatFluentNumber(3, pattern: "'it''s '0''"), "it's 3'");
      expect(formatFluentNumber(3, pattern: "''0"), "'3");
    });

    test('only unquoted percent and minus signs are localized', () {
      expect(formatFluentNumber(3, locale: 'fa', pattern: "%0'%'"), '٪۳%');
      expect(formatFluentNumber(-3, locale: 'et', pattern: "0;'-'0"), '-3');
      expect(formatFluentNumber(-3, locale: 'et', pattern: '0;-0'), '−3');
    });

    test('non-ASCII currency letters and punctuation receive boundary spacing', () {
      expect(formatFluentCurrency(3, currency: 'CZK', locale: 'en'), 'Kč\u00A03.00');
      expect(formatFluentCurrency(3, currency: 'PLN', locale: 'en'), 'zł\u00A03.00');
      expect(formatFluentCurrency(3, currency: 'BGN', locale: 'en'), 'лв.\u00A03.00');
      expect(isAlphabeticSymbol('Kč'), isTrue);
      expect(isAlphabeticSymbol('zł'), isTrue);
      expect(isAlphabeticSymbol('лв.'), isFalse);
    });

    test('spacing uses the adjacent boundary, not the whole currency symbol', () {
      expect(formatFluentCurrency(3, currency: 'BRL', locale: 'en'), 'R\$3.00');
      expect(formatFluentCurrency(3, currency: 'BRL', locale: 'ksb'), '3.00\u00A0R\$');
      expect(formatFluentCurrency(3, currency: 'CHF', locale: 'fa'), '\u200ECHF\u00A0۳٫۰۰');
      expect(formatFluentCurrency(3, currency: 'CHF', locale: 'ru'), '3,00\u00A0CHF');
    });

    test('automatic currency spacing requires a digit, not NaN or infinity', () {
      expect(formatFluentCurrency(double.infinity, currency: 'CHF', locale: 'en'), 'CHF∞');
      expect(formatFluentCurrency(double.nan, currency: 'CHF', locale: 'en'), 'CHFNaN');
      expect(formatFluentCurrency(double.infinity, currency: 'CHF', locale: 'ru'), '∞\u00A0CHF');
    });

    test('NUMBER currency calls share the Unicode spacing implementation', () {
      final bundle = FluentBundle(locale: 'en', useIsolating: false)..addSource(
        'price = { NUMBER(\$n, style: "currency", currency: "PLN") }\n');
      expect(bundle.format('price', args: {'n': 3}), 'zł\u00A03.00');
    });
  });

  group('Reference scopes and catalog fallback boundaries', () {
    test('nested terms restore outer parameters and never consume caller args', () {
      final bundle = FluentBundle(locale: 'en', useIsolating: false)..addSource(
        '-inner = { \$case }\n'
        '-outer = { \$case } / { -inner(case: "inner") } / { \$case }\n'
        'message = { -outer(case: "outer") } / { \$case }\n'
        'missing = { -inner }\n');
      expect(bundle.diagnostics, isEmpty);
      expect(bundle.format('message', args: {'case': 'caller'}),
          'outer / inner / outer / caller');
      expect(bundle.format('missing', args: {'case': 'caller'}), r'{$case}');
    });

    test('message references inside a term see the term parameter scope', () {
      final bundle = FluentBundle(locale: 'en', useIsolating: false)..addSource(
        'shared = { \$case }\n-term = { shared }\n'
        'message = { -term(case: "local") } / { shared }\n');
      expect(bundle.format('message', args: {'case': 'caller'}), 'local / caller');
    });

    test('missing attribute falls back without substituting the main value', () {
      final catalog = FluentCatalog(useIsolating: false)
        ..registerFtl('ru', 'button = Русский\n')
        ..registerFtl('en', 'button = English\n    .title = English title\n');
      final messages = UiMessages(catalog: catalog);
      final result = messages.resolve('button', attribute: 'title', locale: 'ru');
      expect(result.value, 'English title');
      expect(result.resolvedLocale, 'en');
      expect(result.provenance, ResolutionProvenance.defaultLocale);
      expect(messages.forLocale('ru').tAttr('button', 'title'), 'English title');
    });

    test('empty and unresolved-reference values are found, not locale misses', () {
      final catalog = CompositeCatalog([
        StaticCatalog({'ru': {'empty': ''}, 'en': {'empty': 'not empty'}}),
        FluentCatalog(useIsolating: false)
          ..registerFtl('ru', 'broken = { absent }\n')
          ..registerFtl('en', 'broken = English\n'),
      ]);
      final messages = UiMessages(catalog: catalog);
      expect(messages.resolve('empty', locale: 'ru').value, '');
      final result = messages.resolve('broken', locale: 'ru');
      expect(result.value, '{absent}');
      expect(result.provenance, ResolutionProvenance.exactMatch);
    });
  });

  group('Canonical number locales and Unicode scalar escapes', () {
    test('regional symbols survive extensions, variants and explicit scripts', () {
      for (final locale in [
        'es-MX-u-nu-latn', 'es-Latn-MX', 'es-MX-1996',
        'es_MX.UTF-8', ' ES-mx ', 'es-MX-x-test',
      ]) {
        expect(formatFluentNumber(1234.5, locale: locale), '1,234.5',
            reason: locale);
      }
      expect(formatFluentNumber(12345.5, locale: 'es'), '12.345,5');
    });

    test('invalid tags use English instead of a partially parsed language', () {
      expect(formatFluentNumber(1234.5, locale: 'de--CH'), '1,234.5');
      expect(formatFluentNumber(1234.5, locale: 'de-CH-x'), '1,234.5');
    });

    test('canonical number lookup also applies to percent and currency', () {
      final canonical = FluentBundle(locale: 'en-DE', useIsolating: false);
      final extended = FluentBundle(locale: 'en-Latn-DE-u-nu-latn', useIsolating: false);
      const source = 'decimal = { NUMBER(\$n) }\n'
          'percent = { NUMBER(\$n, style: "percent") }\n'
          'currency = { NUMBER(\$n, style: "currency", currency: "EUR") }\n';
      canonical.addSource(source);
      extended.addSource(source);
      for (final key in ['decimal', 'percent', 'currency']) {
        expect(extended.format(key, args: {'n': 1234.5}),
            canonical.format(key, args: {'n': 1234.5}), reason: key);
      }
    });

    test('escaped surrogates are replaced separately, not joined as a pair', () {
      final bundle = FluentBundle(locale: 'en')..addSource(
        r'value = { "\uD800\uDFFF\uD83D\uDE00\U00D900" }');
      expect(bundle.diagnostics, isEmpty);
      expect(bundle.format('value'), '\uFFFD' * 5);
    });

    test('Unicode scalar boundaries and astral escapes remain intact', () {
      final bundle = FluentBundle(locale: 'en')..addSource(
        r'value = { "\uD7FF\uE000\U01F600\U10FFFF" }');
      expect(bundle.diagnostics, isEmpty);
      expect(bundle.format('value'), String.fromCharCodes([0xD7FF, 0xE000, 0x1F600, 0x10FFFF]));
    });

    test('out-of-range escape is junk and does not discard following messages', () {
      final bundle = FluentBundle(locale: 'en')..addSource(
        r'bad = { "\U110000" }' '\nnext = Valid\n');
      expect(bundle.diagnostics, isNotEmpty);
      expect(bundle.format('bad'), isNull);
      expect(bundle.format('next'), 'Valid');
    });
  });

  group('Typed Fluent resolution semantics', () {
    test('nested DATETIME inherits options and applies outer overrides', () {
      final bundle = FluentBundle(locale: 'en')..addSource(
        'inherit = { DATETIME(DATETIME(\$epoch, year: "numeric"), month: "long") }\n'
        'override = { DATETIME(DATETIME(\$epoch, year: "numeric"), year: "2-digit", month: "long") }\n');
      expect(bundle.format('inherit', args: {'epoch': 0}), 'January 1970');
      expect(bundle.format('override', args: {'epoch': 0}), 'January 70');
    });

    test('built-in arguments evaluate message and term references', () {
      final bundle = FluentBundle(locale: 'en', useIsolating: false)..addSource(
        'amount = 12.5\n-term = 12.5\n'
        'message = { NUMBER(amount, minimumFractionDigits: 2) }\n'
        'term = { NUMBER(-term, minimumFractionDigits: 2) }\n');
      expect(bundle.format('message'), '12.50');
      expect(bundle.format('term'), '12.50');
    });

    test('strings are identifiers, never implicit numeric selectors', () {
      final bundle = FluentBundle(locale: 'en', useIsolating: false)..addSource(
        'pick = { \$value ->\n [1] exact\n [one] singular\n *[other] default\n}\n');
      expect(bundle.format('pick', args: {'value': 1}), 'exact');
      expect(bundle.format('pick', args: {'value': '1'}), 'default');
      expect(bundle.format('pick', args: {'value': 'one'}), 'singular');
      expect(bundle.format('pick', args: {'value': ' one '}), 'default');
      expect(bundle.format('pick', args: {'value': '${bidiFsi}one$bidiPdi'}), 'default');
      expect(bundle.format('pick', args: {'value': true}), 'default');
      expect(bundle.format('pick'), 'default');
    });

    test('rounding and percent display cannot change numeric variant identity', () {
      final bundle = FluentBundle(locale: 'en', useIsolating: false)..addSource(
        'rounded = { NUMBER(\$n, maximumFractionDigits: 0) ->\n'
        ' [1] exactly one\n [one] singular\n *[other] other\n}\n'
        'percent = { NUMBER(\$n, style: "percent") ->\n'
        ' [50] fifty\n [0.5] half\n *[other] other\n}\n');
      expect(bundle.format('rounded', args: {'n': 1.2}), 'other');
      expect(bundle.format('rounded', args: {'n': 1}), 'exactly one');
      expect(bundle.format('percent', args: {'n': 0.5}), 'half');
    });

    test('literal numbers and strings keep distinct selector types', () {
      final bundle = FluentBundle(locale: 'en')..addSource(
        'numeric = { 1 ->\n [1] number\n *[other] default\n}\n'
        'string = { "1" ->\n [1] number\n *[other] default\n}\n');
      expect(bundle.format('numeric'), 'number');
      expect(bundle.format('string'), 'default');
    });

    test('NaN and Infinity identifiers are not numeric literal keys', () {
      final bundle = FluentBundle(locale: 'en')..addSource(
        'pick = { \$n ->\n [NaN] text NaN\n [Infinity] text infinity\n *[other] other\n}\n');
      expect(bundle.format('pick', args: {'n': double.nan}), 'other');
      expect(bundle.format('pick', args: {'n': double.infinity}), 'other');
      expect(bundle.format('pick', args: {'n': 'NaN'}), 'text NaN');
      expect(bundle.format('pick', args: {'n': 'Infinity'}), 'text infinity');
    });

    test('unknown functions cannot select by their display fallback', () {
      final bundle = FluentBundle(locale: 'en')..addSource(
        'pick = { UNKNOWN("one") ->\n [one] wrong\n *[other] default\n}\n'
        'display = { UNKNOWN("one") }\n');
      expect(bundle.format('pick'), 'default');
      expect(bundle.format('display'), 'one');
    });

    test('nested NUMBER preserves inner options and lets outer options override', () {
      final bundle = FluentBundle(locale: 'en', useIsolating: false)..addSource(
        'inherit = { NUMBER(NUMBER(\$n, minimumFractionDigits: 2), useGrouping: "false") }\n'
        'override = { NUMBER(NUMBER(\$n, minimumFractionDigits: 2), minimumFractionDigits: 1, maximumFractionDigits: 1, useGrouping: "false") }\n'
        'select = { NUMBER(NUMBER(\$n)) ->\n [1234.5] exact\n *[other] other\n}\n');
      expect(bundle.format('inherit', args: {'n': 1234.5}), '1234.50');
      expect(bundle.format('override', args: {'n': 1234.5}), '1234.5');
      expect(bundle.format('select', args: {'n': 1234.5}), 'exact');
    });

    test('nested numeric date arguments use their value instead of display', () {
      final bundle = FluentBundle(locale: 'en')..addSource(
        'date = { DATETIME(NUMBER(\$epoch), year: "numeric", timeZone: "UTC") }\n');
      expect(bundle.format('date', args: {'epoch': 0}), '1970');
    });

    test('term attribute selection and isolated named arguments remain valid', () {
      final bundle = FluentBundle(locale: 'en', useIsolating: false)..addSource(
        '-form = { \$case }\n    .kind = singular\n'
        'kind = { -form.kind ->\n [singular] yes\n *[other] no\n}\n'
        'named = { -form(case: "local") }\n');
      expect(bundle.format('kind'), 'yes');
      expect(bundle.format('named', args: {'case': 'caller'}), 'local');
    });

    test('currency applies integer padding and a maximum-only fraction option', () {
      final bundle = FluentBundle(locale: 'en', useIsolating: false)..addSource(
        'rounded = { NUMBER(\$n, style: "currency", currency: "USD", maximumFractionDigits: 0) }\n'
        'padded = { NUMBER(\$n, style: "currency", currency: "USD", minimumIntegerDigits: 3, useGrouping: "false") }\n');
      expect(bundle.format('rounded', args: {'n': 3.45}), '\$3');
      expect(bundle.format('padded', args: {'n': 3.4}), '\$003.40');
      expect(formatFluentCurrency(3.45, currency: 'USD', locale: 'en',
          maximumFractionDigits: 0), '\$3');
    });

    test('small exponential fractions are not treated as integers by CLDR', () {
      expect(resolveCldrPluralCategory('ru', 1e-7), 'other');
      expect(resolveCldrPluralCategory('cs', 1e-7), 'many');
      expect(resolveCldrPluralCategory('sl', 1e-7), 'few');
      expect(resolveCldrPluralCategory('ru', -1e-7), 'other');
    });

    test('Portuguese regional rule survives script, extension and POSIX input', () {
      for (final locale in ['pt-PT', 'pt-Latn-PT', 'pt-PT-u-nu-latn', 'pt_PT.UTF-8']) {
        expect(resolveCldrPluralCategory(locale, 0), 'other', reason: locale);
      }
      expect(resolveCldrPluralCategory('pt-BR', 0), 'one');
    });

    test('native minimum integer keeps exact plural modulo operands', () {
      final minimum = int.parse('-9223372036854775808');
      expect(resolveCldrPluralCategory('ru', minimum), 'many');
      expect(resolveCldrPluralCategory('pl', minimum), 'many');
    }, testOn: 'vm');
  });

  group('Fluent resource and resolution limits', () {
    test('exponential acyclic references are bounded and budgets reset per call', () {
      final source = StringBuffer('m0 = x\n');
      for (var i = 1; i <= 20; i++) {
        source.writeln('m$i = { m${i - 1} }{ m${i - 1} }');
      }
      final bundle = FluentBundle(locale: 'en', useIsolating: false)
        ..addSource(source.toString());
      expect(bundle.diagnostics, isEmpty);
      expect(bundle.format('m20'), '{???}');
      expect(bundle.format('m0'), 'x');
      expect(bundle.format('m1'), 'xx');
      expect(bundle.format('m20'), '{???}');
    });

    test('terms and message references share the same resolution budget', () {
      final source = StringBuffer('-t0 = x\n');
      for (var i = 0; i < 15; i++) {
        source.writeln('m$i = { -t$i }{ -t$i }');
        source.writeln('-t${i + 1} = { m$i }{ m$i }');
      }
      final bundle = FluentBundle(locale: 'en')..addSource(source.toString());
      expect(bundle.diagnostics, isEmpty);
      expect(bundle.formatTerm('t15'), '{???}');
      expect(bundle.format('m14'), '{???}');
      expect(bundle.formatTerm('t0'), 'x');
    });

    test('zero-length expansions cannot evade the step budget', () {
      final bundle = FluentBundle(locale: 'en')..addResource(FtlResource(entries: [
        FtlMessage(id: 'empty', value: FtlPattern(elements: List.generate(
          maxFluentResolutionSteps + 1,
          (_) => const FtlPlaceable(FtlStringLiteral('')),
        ))),
      ]));
      expect(bundle.format('empty'), '{???}');
    });

    test('character budget accepts its boundary and rejects oversized values', () {
      final boundary = 'a' * maxFluentResolutionCharacters;
      final bundle = FluentBundle(locale: 'en')..addSource(
        'plain = $boundary\n'
        'greet = Hello { \$name }!\n'
        'select = { \$name ->\n    [yes] Yes\n   *[other] Other\n}\n'
        'number = { NUMBER(\$name) }\n'
        'date = { DATETIME(\$name) }\n',
      );
      expect(bundle.format('plain'), boundary);
      final huge = '$boundary😀';
      for (final id in ['greet', 'select', 'number', 'date']) {
        // No partial output, unmatched isolate, or split surrogate is exposed.
        expect(bundle.format(id, args: {'name': huge}), '{???}', reason: id);
      }
      expect(bundle.format('greet', args: {'name': 'Ann'}),
          'Hello ${bidiFsi}Ann$bidiPdi!');
    });

    test('deep raw built-in arguments from a programmatic AST are bounded', () {
      FtlExpression argument = const FtlVariableReference('n');
      for (var i = 0; i < 1000; i++) {
        argument = FtlPlaceableExpression(argument);
      }
      final bundle = FluentBundle(locale: 'en')..addResource(FtlResource(entries: [
        FtlMessage(id: 'deep', value: FtlPattern(elements: [
          FtlPlaceable(FtlFunctionReference(name: 'NUMBER', positional: [argument])),
        ])),
      ]));
      expect(bundle.format('deep', args: {'n': 1}), '{???}');
    });

    test('deep placeables and function calls become junk and parsing recovers', () {
      final count = maxFtlNestingDepth + 5;
      for (final value in [
        '${'{ ' * count}1${' }' * count}',
        '{ ${'NUMBER(' * count}1${')' * count} }',
      ]) {
        final resource = FtlParser.parse('bad = $value\ngood = { \$n }\n');
        expect(resource.junk, hasLength(1));
        expect(resource.junk.single.message, contains('nesting limit'));
        final bundle = FluentBundle(locale: 'en')..addResource(resource);
        expect(bundle.hasMessage('bad'), isFalse);
        expect(bundle.format('good', args: {'n': 2}), '2');
      }
    });

    test('resource entries cannot drift from their cached lookup maps', () {
      final entries = <FtlEntry>[
        const FtlMessage(id: 'one', value: FtlPattern(elements: [FtlTextElement('1')])),
      ];
      final resource = FtlResource(entries: entries);
      entries.clear();
      expect(resource.entries, hasLength(1));
      expect(resource.messages.keys, ['one']);
      expect(() => resource.entries.clear(), throwsUnsupportedError);
      final bundle = FluentBundle(locale: 'en')..addResource(resource);
      expect(bundle.format('one'), '1');
    });
  });

  group('Numeric and date boundary regressions', () {
    test('invalid epoch arguments use the existing function fallback', () {
      final bundle = FluentBundle(locale: 'en')..addSource(
        'date = { DATETIME(\$value, year: "numeric") }\n',
      );
      for (final entry in <num, String>{
        double.nan: 'NaN',
        double.infinity: 'Infinity',
        double.negativeInfinity: '-Infinity',
        8640000000000001: '8640000000000001',
        -8640000000000001: '-8640000000000001',
      }.entries) {
        expect(bundle.format('date', args: {'value': entry.key}), entry.value);
      }
      expect(bundle.format('date', args: {'value': 0}), '1970');
      expect(bundle.format('date', args: {'value': 'not a date'}), 'not a date');
    });

    test('overflowing numeric options use defaults instead of throwing', () {
      final huge = '9' * 400;
      final bundle = FluentBundle(locale: 'en')..addSource(
        'number = { NUMBER(\$n, minimumFractionDigits: $huge, '
        'maximumFractionDigits: $huge, minimumIntegerDigits: $huge) }\n',
      );
      expect(bundle.diagnostics, isEmpty);
      expect(bundle.format('number', args: {'n': 1.25}), '1.25');
    });

    test('percent scaling does not introduce binary rounding noise', () {
      final bundle = FluentBundle(locale: 'en')..addSource(
        'percent = { NUMBER(\$n, style: "percent", useGrouping: "false", '
        'maximumFractionDigits: 20) }\n',
      );
      expect(bundle.format('percent', args: {'n': 0.29}), '29%');
      expect(bundle.format('percent', args: {'n': -0.29}), '-29%');
      expect(bundle.format('percent', args: {'n': 0.0001}), '0.01%');
      expect(bundle.format('percent', args: {'n': -0.0}), '-0%');
    });

    test('VM integer endpoints format and scale without overflow', () {
      final minimum = int.parse('-9223372036854775808');
      final maximum = int.parse('9223372036854775807');
      expect(formatFluentNumber(minimum, useGrouping: false), '$minimum');
      expect(formatFluentCurrency(minimum, currency: 'USD', locale: 'en',
          useGrouping: false), '-\$9223372036854775808.00');
      final bundle = FluentBundle(locale: 'en')..addSource(
        'percent = { NUMBER(\$n, style: "percent", useGrouping: "false") }\n',
      );
      expect(bundle.format('percent', args: {'n': maximum}), '922337203685477580700%');
      expect(bundle.format('percent', args: {'n': minimum}), '-922337203685477580800%');
    }, testOn: 'vm');
  });

  group('LocaleTag & BCP-47 Normalization', () {
    test('locale bounds count UTF-8 bytes before normalization', () {
      expect(LocaleTag.tryParse('en${' ' * 62}'), isNotNull);
      expect(LocaleTag.tryParse('en${' ' * 63}'), isNull);
      expect(LocaleTag.tryParse('en${'\u2003' * 21}'), isNull);
    });

    test('extension blocks serialize canonically with private use last', () {
      final first = LocaleTag.parse('en-u-ca-gregory-t-fr-x-acme');
      final second = LocaleTag.parse('en-t-fr-u-ca-gregory-x-acme');
      expect(first, second);
      expect(first.toLanguageTag(), second.toLanguageTag());
      const manual = LocaleTag(language: 'en', extensions: {
        'x': 'acme', 'u': 'ca-gregory', 't': 'fr',
      });
      expect(LocaleTag.parse(manual.toLanguageTag()), manual);
      expect(acceptLanguageLocales(
          'en-u-ca-gregory-t-fr, en-t-fr-u-ca-gregory'),
          ['en-t-fr-u-ca-gregory']);
    });

    test('rejects malformed separators, variants and extensions', () {
      for (final locale in [
        'en-', 'en--US', 'en-u', 'en-x', 'en-!', 'en-u-!',
        'en-u-abcdefghi', 'en-u-ca-u-nu', 'de-1996-1996',
      ]) {
        expect(LocaleTag.tryParse(locale), isNull, reason: locale);
      }
    });

    test('private use retains single-character subtags and strips for lookup', () {
      final tag = LocaleTag.parse('en-u-ca-gregory-x-a-b');
      expect(tag.toLanguageTag(), 'en-u-ca-gregory-x-a-b');
      expect(tag.toCatalogTag(), 'en');
      expect(tag.extensions['x'], 'a-b');
    });

    test('fallback preserves intermediate variant catalogs', () {
      expect(localeFallbackChain('sl-rozaj-biske'),
          ['sl-rozaj-biske', 'sl-rozaj', 'sl']);
    });

    test('parses and normalizes standard tags', () {
      final tag = LocaleTag.parse('ru_RU.UTF-8');
      expect(tag.language, 'ru');
      expect(tag.region, 'RU');
      expect(tag.toCatalogTag(), 'ru-RU');
      expect(tag.isRtl, isFalse);
    });

    test('parses script and regional variants', () {
      final tag = LocaleTag.parse('zh-Hant-TW');
      expect(tag.language, 'zh');
      expect(tag.script, 'Hant');
      expect(tag.region, 'TW');
      expect(tag.toCatalogTag(), 'zh-Hant-TW');
    });

    test('detects RTL languages correctly', () {
      expect(LocaleTag.parse('ar-EG').isRtl, isTrue);
      expect(LocaleTag.parse('he').isRtl, isTrue);
      expect(LocaleTag.parse('fa').isRtl, isTrue);
      expect(LocaleTag.parse('ur').isRtl, isTrue);
      expect(LocaleTag.parse('en').isRtl, isFalse);
      expect(LocaleTag.parse('ru').isRtl, isFalse);
    });

    test('canonicalizes legacy Android/Java ISO 639 codes (iw->he, in->id, ji->yi)', () {
      expect(LocaleTag.parse('iw_IL').toCatalogTag(), 'he-IL');
      expect(LocaleTag.parse('in_ID').toCatalogTag(), 'id-ID');
      expect(LocaleTag.parse('ji').toCatalogTag(), 'yi');
    });

    test('canonicalizes grandfathered BCP-47 tags', () {
      // RFC 5646 2.2.8: these tags are valid but have no subtag structure.
      expect(LocaleTag.tryParse('i-klingon')!.toLanguageTag(), 'tlh');
      expect(LocaleTag.tryParse('en-GB-oed')!.toLanguageTag(), 'en-GB-oxendict');
      expect(LocaleTag.tryParse('zh-min-nan')!.toLanguageTag(), 'nan');
      expect(LocaleTag.tryParse('sgn-BE-FR')!.toLanguageTag(), 'sfb');
      expect(LocaleTag.tryParse('no-bok')!.toLanguageTag(), 'nb');
      expect(LocaleTag.tryParse('art-lojban')!.toLanguageTag(), 'jbo');
      expect(LocaleTag.tryParse('zh-guoyu')!.toLanguageTag(), 'cmn');
      // The five tags without a preferred value stay opaque instead of turning
      // into a lookup miss.
      expect(LocaleTag.tryParse('zh-min')!.toLanguageTag(), 'zh-min');
      expect(LocaleTag.tryParse('i-default')!.toLanguageTag(), 'i-default');
      expect(LocaleTag.tryParse('i-default')!.language, 'i-default');
    });

    test('rejects oversized inputs (DoS guard)', () {
      final huge = 'a' * 100;
      expect(LocaleTag.tryParse(huge), isNull);
      // Rejects oversized raw input before trim
      expect(LocaleTag.tryParse('   ${'a' * 65}   '), isNull);
    });
  });

  group('LocaleFallbackChain', () {
    test('generates expected candidate hierarchy', () {
      final candidates = localeCandidates(
        locale: 'ru-RU',
        defaultLocale: 'en',
      );
      expect(candidates, containsAllInOrder(['ru-RU', 'ru', 'en']));
    });

    test('generates regional likely script expansion for zh-TW', () {
      final candidates = localeCandidates(
        locale: 'zh-TW',
        defaultLocale: 'en',
      );
      expect(
        candidates,
        containsAllInOrder(['zh-TW', 'zh-Hant-TW', 'zh-Hant', 'zh', 'en']),
      );
    });
  });

  group('AcceptLanguageParser', () {
    test('specific lower quality is not promoted by a broad accepted range', () {
      const header = 'en;q=1, en-US;q=0.1, fr;q=0.9';
      expect(negotiateLocale(acceptLanguageHeader: header,
          supportedLocales: ['en-US', 'fr']), 'fr');
      expect(negotiateLocale(acceptLanguageHeader: header,
          supportedLocales: ['en-US', 'en-GB', 'fr']), 'en-GB');
    });

    test('strict parsing preserves exclusions, aliases and complete identity', () {
      final preferences = parseAcceptLanguageStrict(
          'iw_IL-u-ca-hebrew;q=0.8, *;q=0.5, en;q=0');
      expect(preferences.map((p) => p.tag), ['he-IL-u-ca-hebrew', '*', 'en']);
      expect(preferences.map((p) => p.quality), [800, 500, 0]);
      expect(preferences.last.isRejected, isTrue);
      expect(preferences[1].isWildcard, isTrue);
      expect(() => preferences.clear(), throwsUnsupportedError);
      expect(parseAcceptLanguage('en;q=0, iw_IL').single.tag, 'he-IL');
    });

    test('raw byte limit is checked before trimming or allocating large input', () {
      final exact = 'en${' ' * (maxAcceptLanguageLength - 2)}';
      expect(parseAcceptLanguageStrict(exact).single.tag, 'en');
      for (final header in ['$exact ', 'en;ignored=${'é' * 2050}']) {
        expect(() => parseAcceptLanguageStrict(header), throwsA(
          isA<AcceptLanguageLimitException>().having(
            (e) => e.limit, 'limit', AcceptLanguageLimit.headerBytes),
        ));
        expect(parseAcceptLanguage(header), isEmpty);
        expect(preferredLocaleFromAcceptLanguage(header), isNull);
      }
    });

    test('range limit counts rejected, empty and malformed input ranges', () {
      final exact = List.filled(maxAcceptLanguageRanges, 'en').join(',');
      expect(parseAcceptLanguageStrict(exact), hasLength(maxAcceptLanguageRanges));
      for (final header in [
        '$exact,ru',
        '${List.filled(maxAcceptLanguageRanges, 'en;q=0').join(',')},ru',
        '${',' * maxAcceptLanguageRanges}ru',
        '${List.filled(maxAcceptLanguageRanges, '!').join(',')},ru',
      ]) {
        expect(() => parseAcceptLanguageStrict(header), throwsA(
          isA<AcceptLanguageLimitException>().having(
            (e) => e.limit, 'limit', AcceptLanguageLimit.ranges),
        ));
        expect(parseAcceptLanguage(header), isEmpty);
      }
    });

    test('invalid locale ranges are skipped and source indices include blanks', () {
      final preferences = parseAcceptLanguageStrict(
          ', !!, en.UTF-8, ru@latin, en--US, , ru;q=0.8, de;q=0.8');
      expect(preferences.map((p) => p.tag), ['ru', 'de']);
      expect(preferences.map((p) => p.order), [6, 7]);
    });

    test('quality syntax retains Rust owner behavior', () {
      final preferences = parseAcceptLanguageStrict(
          'fr;q=1., de;q=0., en;Q=0.010, ru;q=NaN, pt;q=.5, '
          'nl;q=0.5;q=0.8, ja;q, es;ignored=value');
      expect(preferences.map((p) => p.tag), ['fr', 'es', 'en', 'de']);
      expect(preferences.map((p) => p.quality), [1000, 1000, 10, 0]);
    });

    test('negotiation never returns an invalid or unsupported fallback', () {
      expect(negotiateLocale(acceptLanguageHeader: null,
          supportedLocales: []), isNull);
      expect(negotiateLocale(acceptLanguageHeader: 'en',
          supportedLocales: ['!', 'en--US']), isNull);
      expect(negotiateLocale(acceptLanguageHeader: 'fr',
          supportedLocales: ['!', 'ru'], defaultLocale: 'en'), 'ru');
      expect(negotiateLocale(acceptLanguageHeader: null,
          supportedLocales: ['ru', 'EN_us'], defaultLocale: 'en-US'), 'EN_us');
    });

    test('q=0 exclusions apply to explicit matching and fallback choices', () {
      expect(negotiateLocale(acceptLanguageHeader: 'en;q=0',
          supportedLocales: ['en', 'en-US', 'ru']), 'ru');
      expect(negotiateLocale(acceptLanguageHeader: 'en;q=0',
          supportedLocales: ['en-US']), isNull);
      expect(negotiateLocale(acceptLanguageHeader: '*;q=0',
          supportedLocales: ['en', 'ru']), isNull);
    });

    test('specific positive preferences override broader exclusions', () {
      expect(negotiateLocale(acceptLanguageHeader: '*;q=0, ru;q=0.5',
          supportedLocales: ['en', 'ru-RU']), 'ru-RU');
      expect(negotiateLocale(acceptLanguageHeader: 'en;q=0, en-US;q=0.7',
          supportedLocales: ['en-GB', 'en-US']), 'en-US');
      expect(negotiateLocale(acceptLanguageHeader: 'en-US;q=0, en;q=0.7',
          supportedLocales: ['en-US', 'en-GB']), 'en-GB');
    });

    test('canonical duplicate qualities use highest priority', () {
      expect(negotiateLocale(acceptLanguageHeader: 'iw;q=0, he;q=0.8',
          supportedLocales: ['en', 'HE']), 'HE');
      expect(negotiateLocale(acceptLanguageHeader: 'he;q=0.8, iw;q=0',
          supportedLocales: ['en', 'HE']), 'HE');
    });

    test('negotiation uses catalog identities for formatting extensions', () {
      expect(negotiateLocale(acceptLanguageHeader: 'en-US-u-nu-latn;q=0',
          supportedLocales: ['en-US', 'ru']), 'ru');
      expect(negotiateLocale(acceptLanguageHeader: 'en-US-u-nu-latn;q=0.8',
          supportedLocales: ['ru', 'en_US']), 'en_US');
    });

    test('same-language sibling matching respects explicit scripts', () {
      expect(negotiateLocale(acceptLanguageHeader: 'sr-Latn, ru;q=0.8',
          supportedLocales: ['sr-Cyrl', 'ru']), 'ru');
      expect(negotiateLocale(acceptLanguageHeader: 'sr-Latn',
          supportedLocales: ['ru', 'sr-Latn-RS']), 'sr-Latn-RS');
    });

    test('whole-header limit failures use the documented absent-header policy', () {
      expect(negotiateLocale(acceptLanguageHeader: 'x' * 4097,
          supportedLocales: ['ru', 'EN_us'], defaultLocale: 'en-US'), 'EN_us');
    });

    test('parses and sorts by q-value descending', () {
      final header = 'ru-RU, ru;q=0.9, en-US;q=0.8, *;q=0.5';
      final prefs = parseAcceptLanguage(header);

      expect(prefs.length, 4);
      expect(prefs[0].tag, 'ru-RU');
      expect(prefs[0].quality, 1000);
      expect(prefs[1].tag, 'ru');
      expect(prefs[1].quality, 900);
      expect(prefs[2].tag, 'en-US');
      expect(prefs[2].quality, 800);
      expect(prefs[3].tag, '*');
      expect(prefs[3].quality, 500);
    });

    test('negotiates supported locale properly', () {
      final supported = ['en', 'ru', 'de'];
      final negotiated = negotiateLocale(
        acceptLanguageHeader: 'fr-FR, ru-RU;q=0.9, en;q=0.8',
        supportedLocales: supported,
      );
      expect(negotiated, 'ru');
    });

    test('extracts locales and preferred locale matching Rust accept_language functions', () {
      const header = 'ru-RU, ru;q=0.9, en-US;q=0.8, *;q=0.5';
      final locales = acceptLanguageLocales(header);
      expect(locales, ['ru-RU', 'ru', 'en-US']);
      expect(preferredLocaleFromAcceptLanguage(header), 'ru-RU');

      final catalogLocales = acceptLanguageCatalogLocales(header);
      expect(catalogLocales, ['ru-RU', 'ru', 'en-US']);
      expect(preferredCatalogLocaleFromAcceptLanguage(header), 'ru-RU');
      expect(preferredLocaleFromAcceptLanguage(null), isNull);
    });
  });


  group('Project Fluent Parser & Resolver', () {
    test('formats simple and term-based messages', () {
      final bundle = FluentBundle(locale: 'en', useIsolating: false);
      bundle.addSource('''
-brand-name = RusToK
welcome = Welcome to { -brand-name }!
save = Save changes
''');

      expect(bundle.format('save'), 'Save changes');
      expect(bundle.format('welcome'), 'Welcome to RusToK!');
    });

    test('formats variables and attributes', () {
      final bundle = FluentBundle(locale: 'en', useIsolating: false);
      bundle.addSource('''
user-greeting = Hello, { \$name }!
email-input = Email address
    .placeholder = name@example.com
''');

      expect(bundle.format('user-greeting', args: {'name': 'Alex'}), 'Hello, Alex!');
      expect(bundle.format('email-input'), 'Email address');
      expect(
        bundle.format('email-input', attribute: 'placeholder'),
        'name@example.com',
      );
    });

    test('formats complex Russian pluralization accurately', () {
      final bundle = FluentBundle(locale: 'ru', useIsolating: false);
      bundle.addSource('''
comments-count = { \$count ->
    [one] { \$count } комментарий
    [few] { \$count } комментария
   *[other] { \$count } комментариев
}
''');

      expect(bundle.format('comments-count', args: {'count': 1}), '1 комментарий');
      expect(bundle.format('comments-count', args: {'count': 2}), '2 комментария');
      expect(bundle.format('comments-count', args: {'count': 4}), '4 комментария');
      expect(bundle.format('comments-count', args: {'count': 5}), '5 комментариев');
      expect(bundle.format('comments-count', args: {'count': 11}), '11 комментариев');
      expect(bundle.format('comments-count', args: {'count': 21}), '21 комментарий');
      expect(bundle.format('comments-count', args: {'count': 24}), '24 комментария');
      expect(bundle.format('comments-count', args: {'count': 25}), '25 комментариев');
      // Fractional numbers in Russian CLDR select 'other'
      expect(bundle.format('comments-count', args: {'count': 1.5}), '1.5 комментариев');
    });

    test('exact numeric match takes precedence over CLDR plural category', () {
      final bundle = FluentBundle(locale: 'en', useIsolating: false);
      bundle.addSource('''
unread-messages = { \$count ->
    [0] No unread messages
    [one] 1 unread message
   *[other] { \$count } unread messages
}
''');

      expect(bundle.format('unread-messages', args: {'count': 0}), 'No unread messages');
      expect(bundle.format('unread-messages', args: {'count': 1}), '1 unread message');
      expect(bundle.format('unread-messages', args: {'count': 5}), '5 unread messages');
    });

    test('supports multiline values and spec-compliant escapes', () {
      final bundle = FluentBundle(locale: 'en', useIsolating: false);
      bundle.addSource('''
multiline-msg =
    Line 1
    Line 2
escaped-msg = Literal { "{" }escaped{ "}" } braces
negative-num = Balance: { -42 }
unescaped-str = { "hello\\"world\\\\" }
''');

      expect(bundle.format('multiline-msg'), 'Line 1\nLine 2');
      expect(bundle.format('escaped-msg'), 'Literal {escaped} braces');
      expect(bundle.format('negative-num'), 'Balance: -42');
      expect(bundle.format('unescaped-str'), 'hello"world\\');
    });

    test('protects against circular cross-message and cross-term references', () {
      final bundle = FluentBundle(locale: 'en', useIsolating: false);
      bundle.addSource('''
msg-a = Msg A: { msg-b }
msg-b = Msg B: { msg-a }
-term-x = Term X: { -term-y }
-term-y = Term Y: { -term-x }
use-terms = { -term-x }
''');

      // Without cycle protection, this causes a StackOverflowError crash!
      expect(bundle.format('msg-a'), contains('{???}'));
      expect(bundle.format('use-terms'), contains('{???}'));
    });
  });

  group('UiMessages Facade & Multi-Catalog Engine', () {
    test('dotted attributes take priority over lossy message aliases', () {
      final catalog = FluentCatalog(useIsolating: false)
        ..registerFtl('en', 'input = Input\n'
            '    .placeholder = Attribute\n'
            'input-placeholder = Other message\n');
      final messages = UiMessages(catalog: catalog);
      expect(messages.t('input.placeholder'), 'Attribute');
      expect(messages.tAttr('input', 'placeholder'), 'Attribute');
      expect(messages.t('input-placeholder'), 'Other message');
      final flat = UiMessages(catalog: StaticCatalog({
        'en': {'input.placeholder': 'Literal', 'input-placeholder': 'Alias'},
      }));
      expect(flat.t('input.placeholder'), 'Literal');
    });

    test('explicit attributes cannot bypass lookup key bounds', () {
      final catalog = _LookupCountingCatalog();
      final messages = UiMessages(catalog: catalog);
      for (final attribute in ['', ' ', 'bad\x00name', 'a' * 256]) {
        expect(messages.tAttr('input', attribute, fallback: 'missing'), 'missing');
        expect(messages.forLocale().tryResolve('input', attribute: attribute),
            isNull);
      }
      expect(messages.t(' ' * 256 + 'key', fallback: 'missing'), 'missing');
      expect(catalog.lookups, 0);
    });

    test('parent-chain provenance wins when it overlaps the default locale', () {
      final messages = UiMessages(catalog: StaticCatalog({
        'en': {'hi': 'Hello'},
      }));
      final regional = messages.resolve('hi', locale: 'en-US');
      expect(regional.provenance, ResolutionProvenance.parentLanguage);
      expect(regional.resolvedLocale, 'en');
      expect(messages.resolve('hi', locale: 'ru').provenance,
          ResolutionProvenance.defaultLocale);
      expect(messages.forLocale('en-US').resolve('hi').provenance,
          regional.provenance);
    });

    test('JSON registrations are atomic on cycles and flattened collisions', () {
      final catalog = JsonCatalog()..registerJson('en', {'hi': 'Before'});
      final cycle = <String, dynamic>{'hi': 'Partial write'};
      cycle['self'] = cycle;
      expect(() => catalog.registerJson('en', cycle), throwsFormatException);
      expect(catalog.getMessage('en', 'hi'), 'Before');
      expect(() => catalog.registerJson('fr', cycle), throwsFormatException);
      expect(catalog.getSupportedLocales(), {'en'});
      expect(() => catalog.registerJson('en', {
        'hi': 'Partial write',
        'a.b': 'Flat',
        'a': {'b': 'Nested'},
      }), throwsFormatException);
      expect(catalog.getMessage('en', 'hi'), 'Before');
      expect(catalog.getMessage('en', 'a.b'), isNull);
    });

    test('JSON bounds depth but permits shared non-cyclic subtrees', () {
      var nested = <String, dynamic>{'value': 'deep'};
      for (var i = 0; i < 100; i++) {
        nested = {'child': nested};
      }
      final catalog = JsonCatalog();
      expect(() => catalog.registerJson('en', nested), throwsFormatException);
      expect(catalog.getSupportedLocales(), isEmpty);
      final shared = {'title': 'Shared'};
      catalog.registerJson('en', {'first': shared, 'second': shared});
      expect(catalog.getMessage('en', 'first.title'), 'Shared');
      expect(catalog.getMessage('en', 'second.title'), 'Shared');
    });

    test('resolves with provenance and fallback chain', () {
      final fluentCatalog = FluentCatalog(useIsolating: false);
      fluentCatalog.registerFtl('en', '''
blog-title = RusToK Blog
blog-only-en = Only in English
auth-email-exists = Email already registered
login-form =
    .placeholder = Enter your email
''');
      fluentCatalog.registerFtl('ru', '''
blog-title = Блог RusToK
''');

      final uiMessages = UiMessages(
        catalog: fluentCatalog,
        defaultLocale: 'en',
      );

      // Exact match in Russian
      final resRu = uiMessages.resolve('blog-title', locale: 'ru');
      expect(resRu.value, 'Блог RusToK');
      expect(resRu.provenance, ResolutionProvenance.exactMatch);
      expect(resRu.resolvedLocale, 'ru');

      // Parent/default fallback to English for key missing in Russian
      final resFallback = uiMessages.resolve('blog-only-en', locale: 'ru');
      expect(resFallback.value, 'Only in English');
      expect(resFallback.provenance, ResolutionProvenance.defaultLocale);
      expect(resFallback.resolvedLocale, 'en');

      // Dotted key alias resolution
      final resAlias = uiMessages.resolve('blog.title', locale: 'ru');
      expect(resAlias.value, 'Блог RusToK');

      // Dotted attribute lookup: 'login-form.placeholder'
      final resAttr = uiMessages.t('login-form.placeholder', locale: 'en');
      expect(resAttr, 'Enter your email');

      // Snake_case alias: 'auth.email_exists' -> 'auth-email-exists'
      final resSnake = uiMessages.t('auth.email_exists', locale: 'en');
      expect(resSnake, 'Email already registered');

      // Rejects keys with control characters (returns missingKeyFallback)
      final resControl = uiMessages.resolve('blog\x00title', locale: 'ru');
      expect(resControl.provenance, ResolutionProvenance.missingKeyFallback);
    });

    test('supports StaticCatalog in composite setup', () {
      final staticCatalog = StaticCatalog({
        'en': {
          'error.not_found': 'Entity not found: {id}',
          'button.save.aria-label': 'Save document',
        },
        'ru': {'error.not_found': 'Сущность не найдена: {id}'},
      });

      final composite = CompositeCatalog([staticCatalog]);
      final uiMessages = UiMessages(catalog: composite);

      expect(
        uiMessages.t('error.not_found', locale: 'ru', args: {'id': '42'}),
        'Сущность не найдена: 42',
      );

      // Attribute query on StaticCatalog
      expect(
        uiMessages.tAttr('button.save', 'aria-label', locale: 'en'),
        'Save document',
      );
    });

    test('supports JsonCatalog with nested dynamic maps and attributes', () {
      final jsonCatalog = JsonCatalog();
      jsonCatalog.registerJson('en', {
        'profile': {
          'title': 'User Profile',
          'avatar': {'alt': 'Profile Picture'},
        },
      });

      final uiMessages = UiMessages(catalog: jsonCatalog);
      expect(uiMessages.t('profile.title', locale: 'en'), 'User Profile');
      expect(uiMessages.tAttr('profile.avatar', 'alt', locale: 'en'), 'Profile Picture');
    });

    test('supports UiLocaleTranslator for pre-bound batch lookups', () {
      final catalog = StaticCatalog({
        'ru': {'welcome': 'Добро пожаловать', 'btn': 'ОК'},
        'en': {'welcome': 'Welcome', 'btn': 'OK'},
      });
      final uiMessages = UiMessages(catalog: catalog);
      final ruTranslator = uiMessages.forLocale('ru');

      expect(ruTranslator.t('welcome'), 'Добро пожаловать');
      expect(ruTranslator.t('btn'), 'ОК');
      expect(ruTranslator.tryResolve('welcome')?.value, 'Добро пожаловать');
      expect(ruTranslator.tryResolve('missing-key'), isNull);

      final defaultTranslator = uiMessages.forLocale();
      expect(defaultTranslator.t('welcome'), 'Welcome');
      expect(defaultTranslator.locale, 'en');
    });

    test('normalizes kebab key matching with_kebab_key in Rust', () {
      expect(normalizeKebabKey('workflow.table.name'), 'workflow-table-name');
      expect(normalizeKebabKey('auth_email_exists'), 'auth-email-exists');
      expect(normalizeKebabKey('already-kebab'), 'already-kebab');
    });


    test('normalizes admin locale matching Rust contract', () {
      expect(normalizeAdminLocale('ru_RU.UTF-8'), 'ru');
      expect(normalizeAdminLocale('ru-RU'), 'ru');
      expect(normalizeAdminLocale('ru'), 'ru');
      expect(normalizeAdminLocale('en-US'), 'en');
      expect(normalizeAdminLocale('fr'), 'en');
      expect(normalizeAdminLocale(null), 'en');
    });
  });

  group('Parser conformance (Fluent 1.0)', () {
    test('value on the next line keeps only the common indent', () {
      final bundle = FluentBundle(locale: 'en', useIsolating: false);
      bundle.addSource('''
key =
    first
        nested
    last
''');
      expect(bundle.format('key'), 'first\n    nested\nlast');
    });

    test('final newline is not part of the value', () {
      final bundle = FluentBundle(locale: 'en', useIsolating: false);
      bundle.addSource('last = Done\n');
      expect(bundle.format('last'), 'Done');
    });

    test('a value may start with a dot', () {
      final bundle = FluentBundle(locale: 'en', useIsolating: false);
      bundle.addSource('loading = ...\nhidden = .Hidden\n');
      expect(bundle.format('loading'), '...');
      expect(bundle.format('hidden'), '.Hidden');
    });

    test('whitespace-only lines do not swallow the next entry', () {
      final bundle = FluentBundle(locale: 'en', useIsolating: false);
      bundle.addSource('a = one\n   \nb = two\n');
      expect(bundle.format('a'), 'one');
      expect(bundle.format('b'), 'two');
    });

    test('invalid entry becomes a diagnostic and parsing continues', () {
      final bundle = FluentBundle(locale: 'en', useIsolating: false);
      bundle.addSource('bad line here\ngood = ok\n');
      expect(bundle.format('good'), 'ok');
      expect(bundle.diagnostics.single, startsWith('Junk'));
    });

    test('duplicate message ids keep the first definition', () {
      final bundle = FluentBundle(locale: 'en', useIsolating: false);
      bundle.addSource('dup = first\ndup = second\n');
      expect(bundle.format('dup'), 'first');
      expect(bundle.diagnostics, hasLength(1));
    });

    test('string literals accept only spec escapes', () {
      final bundle = FluentBundle(locale: 'en', useIsolating: false);
      bundle.addSource('esc = { "a\\\\b" }\nbad = { "\\n" }\n');
      expect(bundle.format('esc'), 'a\\b');
      expect(bundle.format('bad'), isNull);
    });

    test('term arguments are visible only inside the term', () {
      final bundle = FluentBundle(locale: 'en', useIsolating: false);
      bundle.addSource('''
-noun = { \$case ->
    [gen] RusToK-gen
   *[nom] RusToK
}
-plain = { \$x }
title = Title: { -noun(case: "gen") }
leak = [{ -plain }]
''');
      expect(bundle.format('title'), 'Title: RusToK-gen');
      expect(bundle.format('leak', args: {'x': 'LEAK'}), '[{\$x}]');
    });

    test('NUMBER() applies fraction digit options', () {
      final bundle = FluentBundle(locale: 'en', useIsolating: false);
      bundle.addSource('''
price = { NUMBER(\$amount, minimumFractionDigits: 2) }
precise = { NUMBER(\$amount, maximumFractionDigits: 1) }
''');
      expect(bundle.format('price', args: {'amount': 3}), '3.00');
      expect(bundle.format('precise', args: {'amount': 2.345}), '2.3');
    });

    test('integral doubles print without a fraction', () {
      final bundle = FluentBundle(locale: 'en', useIsolating: false);
      bundle.addSource('count-text = Items: { \$count }\n');
      expect(bundle.format('count-text', args: {'count': 3.0}), 'Items: 3');
    });

    test('non-finite selectors fall back to the default variant', () {
      final bundle = FluentBundle(locale: 'ru', useIsolating: false);
      bundle.addSource('''
items = { \$count ->
    [one] один
   *[other] много
}
''');
      expect(bundle.format('items', args: {'count': double.nan}), 'много');
      expect(bundle.format('items', args: {'count': double.infinity}), 'много');
    });
  });

  group('CLDR plural categories', () {
    test('handles non-finite values and additional languages', () {
      expect(resolveCldrPluralCategory('ru', double.nan), 'other');
      expect(resolveCldrPluralCategory('ru', double.infinity), 'other');
      expect(resolveCldrPluralCategory('cs', 3), 'few');
      expect(resolveCldrPluralCategory('cs', 5), 'other');
      expect(resolveCldrPluralCategory('ro', 0), 'few');
      expect(resolveCldrPluralCategory('lv', 10), 'zero');
      expect(resolveCldrPluralCategory('lt', 21), 'one');
      expect(resolveCldrPluralCategory('sl', 102), 'two');
      expect(resolveCldrPluralCategory('fr', 0.5), 'one');
      expect(resolveCldrPluralCategory('pt_BR', 1), 'one');
    });

    test('covers the CLDR 48 cardinal rules beyond the original language set', () {
      // `n = 1` also matches fractions whose shortest form is 1.0.
      expect(resolveCldrPluralCategory('es', 1.0), 'one');
      expect(resolveCldrPluralCategory('es', 2), 'other');
      expect(resolveCldrPluralCategory('es', 1000000), 'many');
      expect(resolveCldrPluralCategory('it', 1000000), 'many');

      // `i = 0 or n = 1` (Hindi, Bengali, …).
      expect(resolveCldrPluralCategory('hi', 0), 'one');
      expect(resolveCldrPluralCategory('hi', 0.5), 'one');
      expect(resolveCldrPluralCategory('hi', 2), 'other');
      expect(resolveCldrPluralCategory('bn', 0.04), 'one');

      // Languages with more than two categories.
      expect(resolveCldrPluralCategory('cy', 0), 'zero');
      expect(resolveCldrPluralCategory('cy', 3), 'few');
      expect(resolveCldrPluralCategory('cy', 6), 'many');
      expect(resolveCldrPluralCategory('cy', 4), 'other');
      expect(resolveCldrPluralCategory('ga', 7), 'many');
      expect(resolveCldrPluralCategory('mt', 103), 'few');
      expect(resolveCldrPluralCategory('mt', 111), 'many');
      expect(resolveCldrPluralCategory('gd', 11), 'one');
      expect(resolveCldrPluralCategory('gd', 12), 'two');
      expect(resolveCldrPluralCategory('kw', 2), 'two');
      expect(resolveCldrPluralCategory('kw', 21), 'many');
      expect(resolveCldrPluralCategory('br', 1000000), 'many');
      expect(resolveCldrPluralCategory('shi', 0), 'one');

      // Visible fraction digits select non-trivial categories.
      expect(resolveCldrPluralCategory('da', 0.5), 'one');
      expect(resolveCldrPluralCategory('is', 21), 'one');
      expect(resolveCldrPluralCategory('fil', 4), 'other');
      expect(resolveCldrPluralCategory('gv', 0.5), 'many');
      expect(resolveCldrPluralCategory('sgs', 0.1), 'many');
      expect(resolveCldrPluralCategory('si', 0.1), 'one');

      // Region-specific and unknown tags.
      expect(resolveCldrPluralCategory('pt', 0), 'one');
      expect(resolveCldrPluralCategory('pt-PT', 0), 'other');
      expect(resolveCldrPluralCategory('kok-Latn', 0), 'one');
      expect(resolveCldrPluralCategory('xx', 1), 'other');
    });
  });

  group('Locale and catalog correctness', () {
    test('catalog accepts underscore locale spelling and canonicalizes it', () {
      final catalog = FluentCatalog(useIsolating: false)
        ..registerFtl('ru_RU', 'hi = Привет\n');
      final messages = UiMessages(catalog: catalog);
      final resolved = messages.resolve('hi', locale: 'ru-RU');
      expect(resolved.value, 'Привет');
      expect(resolved.provenance, ResolutionProvenance.exactMatch);
      expect(catalog.getSupportedLocales(), {'ru-RU'});
    });

    test('provenance compares canonical forms of the locale', () {
      final catalog = StaticCatalog({
        'ru-RU': {'hi': 'Привет'},
        'en': {'hi': 'Hi'},
      });
      final messages = UiMessages(catalog: catalog, defaultLocale: 'en_US');
      expect(messages.resolve('hi', locale: 'ru_RU').provenance,
          ResolutionProvenance.exactMatch);
      expect(messages.resolve('hi', locale: 'de').provenance,
          ResolutionProvenance.defaultLocale);
    });

    test('attribute lookups never fall back to the main message', () {
      final catalog = StaticCatalog({
        'en': {'btn': 'Save'},
      });
      final messages = UiMessages(catalog: catalog);
      expect(catalog.getMessage('en', 'btn', attribute: 'placeholder'), isNull);
      expect(
        messages.resolve('btn', attribute: 'placeholder', locale: 'en').provenance,
        ResolutionProvenance.missingKeyFallback,
      );
    });

    test('argument substitution is single-pass', () {
      final catalog = StaticCatalog({
        'en': {'k': '{a} {b}'},
      });
      expect(catalog.getMessage('en', 'k', args: {'a': '{b}', 'b': 'X'}), '{b} X');
    });

    test('JSON catalog skips null values', () {
      final catalog = JsonCatalog()..registerJson('en', {'a': null, 'b': 'x'});
      expect(catalog.hasMessage('en', 'a'), isFalse);
      expect(catalog.hasMessage('en', 'b'), isTrue);
    });

    test('language-only ranges match regional supported locales', () {
      expect(
        negotiateLocale(acceptLanguageHeader: 'ru', supportedLocales: ['en', 'ru-RU']),
        'ru-RU',
      );
      expect(
        negotiateLocale(
          acceptLanguageHeader: 'zh-Hant-TW',
          supportedLocales: ['en', 'zh-Hant'],
        ),
        'zh-Hant',
      );
    });

    test('variants are validated and extensions take part in equality', () {
      expect(LocaleTag.tryParse('en-US-abc'), isNull);
      expect(LocaleTag.parse('de-DE-1996').toCatalogTag(), 'de-DE-1996');
      expect(LocaleTag.parse('en-u-ca-buddhist') == LocaleTag.parse('en'), isFalse);
    });

    test('wildcard does not hide lower-priority explicit ranges', () {
      expect(
        negotiateLocale(
          acceptLanguageHeader: '*;q=1.0, de;q=0.5',
          supportedLocales: ['en', 'de'],
        ),
        'de',
      );
      expect(
        negotiateLocale(
          acceptLanguageHeader: '*;q=1.0',
          supportedLocales: ['en', 'de'],
        ),
        'en',
      );
    });

    test('Kurdish is RTL only with an Arabic script', () {
      expect(LocaleTag.parse('ku').isRtl, isFalse);
      expect(LocaleTag.parse('ku-Arab').isRtl, isTrue);
      expect(LocaleTag.parse('ks').isRtl, isTrue);
      expect(detectTextDirection(language: 'ku', script: 'Latn'),
          LocaleTextDirection.ltr);
    });
  });

  group('Locale-aware NUMBER()', () {
    test('non-finite values preserve signs, percent affixes and currency layout',
        () {
      expect(formatFluentNumber(double.infinity), '∞');
      expect(formatFluentNumber(double.negativeInfinity), '-∞');
      expect(formatFluentNumber(double.nan), 'NaN');
      expect(formatFluentNumber(double.infinity, pattern: '#,##0%'), '∞%');
      expect(formatFluentNumber(double.negativeInfinity,
          pattern: '#,##0%;(#,##0%)'), '(∞%)');
      expect(formatFluentNumber(double.nan, pattern: '#,##0%'), 'NaN%');
      expect(formatFluentCurrency(double.infinity, currency: 'USD', locale: 'en'),
          '\$∞');
      expect(formatFluentCurrency(double.negativeInfinity,
          currency: 'USD', locale: 'en'), '-\$∞');
      final catalog = FluentCatalog(useIsolating: false)..registerFtl('en',
          'percent = { NUMBER(\$n, style: "percent") }\n');
      expect(catalog.getMessage('en', 'percent', args: {'n': double.infinity}),
          '∞%');
    });

    test('groups digits and uses locale separators', () {
      expect(formatFluentNumber(1234567.5, locale: 'en', maximumFractionDigits: 1),
          '1,234,567.5');
      expect(formatFluentNumber(1234567.5, locale: 'ru', maximumFractionDigits: 1),
          '1\u00A0234\u00A0567,5');
      expect(formatFluentNumber(-1500), '-1,500');
      // `Intl.NumberFormat` keeps the sign of a negative value that rounds
      // to zero, so `-0.0001` is `-0` and not `0`.
      expect(formatFluentNumber(-0.0001), '-0');
      expect(formatFluentNumber(-0.0), '-0');
      expect(formatFluentNumber(1234567, useGrouping: false), '1234567');
      expect(formatFluentNumber(5, minimumIntegerDigits: 2), '05');
    });

    test('Polish and Spanish group only from five digits', () {
      expect(formatFluentNumber(1234, locale: 'pl'), '1234');
      expect(formatFluentNumber(12345, locale: 'pl'), '12\u00A0345');
      expect(formatFluentNumber(12345, locale: 'es'), '12.345');
    });

    test('NUMBER() in a bundle uses the bundle locale and percent style', () {
      final bundle = FluentBundle(locale: 'de', useIsolating: false);
      bundle.addSource('''
price = { NUMBER(\$n) }
pct = { NUMBER(\$v, style: "percent") }
''');
      expect(bundle.format('price', args: {'n': 1234.5}), '1.234,5');
      // The German percent pattern is `#,##0 %`, with a no-break space.
      expect(bundle.format('pct', args: {'v': 0.256}), '26\u00A0%');
    });
  });

  group('DATETIME()', () {
    final date = DateTime.utc(2026, 1, 5, 14, 3, 9);

    test('date locale selection uses the canonical locale parser', () {
      for (final locale in ['ru', 'RU', ' ru ', 'ru.UTF-8', 'ru_RU.UTF-8']) {
        expect(formatFluentDateTime(date, locale: locale, dateStyle: 'long'),
            '5 января 2026 г.', reason: locale);
      }
      expect(formatFluentDateTime(date, locale: '!'), '1/5/2026');
    });

    test('British English retains regional order, year and hour cycle', () {
      for (final locale in ['en-GB', 'en_GB', 'en-Latn-GB', 'en-GB-u-ca-gregory']) {
        expect(formatFluentDateTime(date, locale: locale, dateStyle: 'short',
            timeStyle: 'short'), '05/01/2026, 14:03', reason: locale);
      }
      expect(formatFluentDateTime(date, locale: 'en', dateStyle: 'short',
          timeStyle: 'short'), '1/5/26, 2:03 PM');
      expect(formatFluentDateTime(DateTime.utc(2026, 9, 5), locale: 'en-GB',
          dateStyle: 'medium'), '5 Sept 2026');
    });

    test('short date years follow the supported CLDR locale profiles', () {
      const expected = {
        'en': '1/5/26', 'en-GB': '05/01/2026', 'ru': '05.01.2026',
        'uk': '05.01.26', 'de': '05.01.26', 'fr': '05/01/2026',
        'es': '5/1/26', 'pl': '5.01.2026',
      };
      for (final entry in expected.entries) {
        expect(formatFluentDateTime(date, locale: entry.key, dateStyle: 'short'),
            entry.value, reason: entry.key);
      }
    });

    test('Polish month names use standalone or date context as appropriate', () {
      expect(formatFluentDateTime(date, locale: 'pl', month: 'long'), 'styczeń');
      expect(formatFluentDateTime(date, locale: 'pl', month: 'long', year: 'numeric'),
          'styczeń 2026');
      expect(formatFluentDateTime(date, locale: 'pl', month: 'long', day: 'numeric'),
          '5 stycznia');
    });

    test('abbreviated standalone Russian month does not use date-context name', () {
      final march = DateTime.utc(2026, 3, 5);
      expect(formatFluentDateTime(march, locale: 'ru', month: 'short'), 'март');
      expect(formatFluentDateTime(march, locale: 'ru', month: 'short', day: 'numeric'),
          '5 мар.');
    });

    test('narrow names are explicit CLDR symbols, not truncated long names', () {
      final wednesday = DateTime.utc(2026, 1, 7);
      const weekdays = {'en': 'W', 'en-GB': 'W', 'ru': 'С', 'uk': 'С',
        'de': 'M', 'fr': 'M', 'es': 'X', 'pl': 'Ś'};
      for (final entry in weekdays.entries) {
        expect(formatFluentDateTime(wednesday, locale: entry.key, weekday: 'narrow'),
            entry.value, reason: entry.key);
      }
      expect(formatFluentDateTime(date, locale: 'pl', month: 'narrow'), 'S');
      expect(formatFluentDateTime(date, locale: 'pl', month: 'narrow', day: 'numeric'),
          '5 s');
      expect(formatFluentDateTime(date, locale: 'uk', month: 'narrow'), 'С');
      expect(formatFluentDateTime(date, locale: 'uk', month: 'narrow', day: 'numeric'),
          '5 с');
    });

    test('standalone numeric components honor numeric and two-digit widths', () {
      expect(formatFluentDateTime(date, locale: 'ru', day: 'numeric'), '5');
      expect(formatFluentDateTime(date, locale: 'ru', day: '2-digit'), '05');
      expect(formatFluentDateTime(date, locale: 'ru', month: 'numeric'), '1');
      expect(formatFluentDateTime(date, locale: 'ru', month: '2-digit'), '01');
      expect(formatFluentDateTime(date, locale: 'en', minute: 'numeric'), '3');
      expect(formatFluentDateTime(date, locale: 'en', second: 'numeric'), '9');
      expect(formatFluentDateTime(date, locale: 'en', minute: '2-digit'), '03');
      expect(formatFluentDateTime(date, locale: 'en', second: '2-digit'), '09');
      expect(formatFluentDateTime(date, locale: 'en', minute: 'numeric', second: 'numeric'),
          '03:09');
    });

    test('invalid options are ignored before expansion and default selection', () {
      expect(formatFluentDateTime(date, locale: 'en', weekday: 'bad', month: 'bad',
          hour: 'bad', dateStyle: 'bad', timeStyle: 'bad'), '1/5/2026');
      expect(formatFluentDateTime(date, locale: 'en', dateStyle: 'long', month: 'bad'),
          'January 5, 2026');
      expect(formatFluentDateTime(date, locale: 'en', timeStyle: 'short', minute: 'bad'),
          '2:03 PM');
      expect(formatFluentDateTime(date, locale: 'en', dateStyle: 'long', month: '2-digit'),
          '01/5/2026');
    });

    test('two-digit proleptic years preserve sign instead of wrapping modulo', () {
      expect(formatFluentDateTime(DateTime.utc(-1, 1, 5), locale: 'en', year: '2-digit'),
          '-01');
      expect(formatFluentDateTime(DateTime.utc(0, 1, 5), locale: 'en', year: 'numeric'), '0');
      expect(formatFluentDateTime(DateTime.utc(2001, 1, 5), locale: 'en', year: '2-digit'),
          '01');
    });

    test('midnight and noon are stable under explicit hour-cycle override', () {
      final midnight = DateTime.utc(2026, 1, 5);
      final noon = DateTime.utc(2026, 1, 5, 12);
      expect(formatFluentDateTime(midnight, locale: 'en-GB', timeStyle: 'short', hour12: true),
          '12:00 AM');
      expect(formatFluentDateTime(noon, locale: 'en', timeStyle: 'short'), '12:00 PM');
      expect(formatFluentDateTime(midnight, locale: 'en', timeStyle: 'short', hour12: false),
          '00:00');
    });

    test('Fluent options and UTC conversion share the corrected date path', () {
      final bundle = FluentBundle(locale: 'en-GB', useIsolating: false)..addSource(
        'date = { DATETIME(\$value, dateStyle: "short", timeStyle: "short", timeZone: "UTC") }\n'
        'narrow = { DATETIME(\$value, weekday: "narrow") }\n');
      expect(bundle.format('date', args: {'value': date.toLocal()}), '05/01/2026, 14:03');
      expect(bundle.format('narrow', args: {'value': date}), 'M');
    });


    test('date styles in English', () {
      final bundle = FluentBundle(locale: 'en', useIsolating: false);
      bundle.addSource('''
numeric = { DATETIME(\$d) }
short = { DATETIME(\$d, dateStyle: "short") }
medium = { DATETIME(\$d, dateStyle: "medium") }
long = { DATETIME(\$d, dateStyle: "long") }
full = { DATETIME(\$d, dateStyle: "full") }
''');
      final args = {'d': date};
      expect(bundle.format('numeric', args: args), '1/5/2026');
      expect(bundle.format('short', args: args), '1/5/26');
      expect(bundle.format('medium', args: args), 'Jan 5, 2026');
      expect(bundle.format('long', args: args), 'January 5, 2026');
      expect(bundle.format('full', args: args), 'Monday, January 5, 2026');
    });

    test('time styles, 12 and 24 hour clocks', () {
      final bundle = FluentBundle(locale: 'en', useIsolating: false);
      bundle.addSource('''
t12 = { DATETIME(\$d, timeStyle: "short") }
t24 = { DATETIME(\$d, timeStyle: "short", hour12: "false") }
both = { DATETIME(\$d, dateStyle: "medium", timeStyle: "short") }
''');
      final args = {'d': date};
      expect(bundle.format('t12', args: args), '2:03 PM');
      expect(bundle.format('t24', args: args), '14:03');
      expect(bundle.format('both', args: args), 'Jan 5, 2026, 2:03 PM');
    });

    test('Russian long date uses the genitive month', () {
      final bundle = FluentBundle(locale: 'ru', useIsolating: false);
      bundle.addSource('d = { DATETIME(\$d, dateStyle: "long") }\n');
      expect(bundle.format('d', args: {'d': date}), '5 января 2026 г.');
    });

    test('accepts ISO strings and time zone UTC', () {
      final bundle = FluentBundle(locale: 'en', useIsolating: false);
      bundle.addSource('''
iso = { DATETIME("2026-01-05T14:03:09Z", dateStyle: "long") }
''');
      expect(bundle.format('iso'), 'January 5, 2026');
    });

    test('unparseable values fall back to the argument text', () {
      final bundle = FluentBundle(locale: 'en', useIsolating: false);
      bundle.addSource('bad = { DATETIME("not a date") }\n');
      expect(bundle.format('bad'), 'not a date');
    });
  });

  group('LazyCatalog', () {
    test('ready catalogs prepare synchronously, pending ones return a Future',
        () async {
      final lazy = LazyCatalog(loader: (_) => 'hi = Hello\n');
      final composite = CompositeCatalog([StaticCatalog(), lazy]);
      final first = composite.loadForLocale('en');
      expect(first, isA<Future<void>>());
      await first;
      expect(lazy.loadForLocale('en'), isNull);
      expect(composite.loadForLocale('en'), isNull);
      expect(StaticCatalog().loadForLocale('en'), isNull);
    });

    test('preparation validates all locale inputs before calling the loader',
        () async {
      var calls = 0;
      final lazy = LazyCatalog(loader: (_) { calls++; return null; });
      await expectLater(lazy.loadForLocale('!'), throwsArgumentError);
      await expectLater(lazy.loadForLocale('en', defaultLocale: '!'),
          throwsArgumentError);
      await expectLater(lazy.loadForLocale('en', platformFallback: '!'),
          throwsArgumentError);
      expect(calls, 0);
    });

    test('a synchronous adapter failure does not skip later children', () async {
      final lazy = LazyCatalog(loader: (_) => 'hi = Loaded\n');
      final catalog = CompositeCatalog([_FailingCatalog(), lazy]);
      await expectLater(catalog.loadForLocale('en'), throwsStateError);
      expect(catalog.getMessage('en', 'hi'), 'Loaded');
    });

    test('invalid locale never reaches the loader', () async {
      var calls = 0;
      final catalog = LazyCatalog(loader: (_) {
        calls++;
        return null;
      });
      await expectLater(catalog.loadLocale('../en'), throwsArgumentError);
      await expectLater(catalog.loadLocale('en-u'), throwsArgumentError);
      expect(calls, 0);
    });

    test('composites load all nested catalogs and preserve priority', () async {
      final locales = <String>[];
      final lazy = LazyCatalog(loader: (locale) {
        locales.add(locale);
        return 'hi = Lazy\nonly-lazy = Loaded\n';
      });
      final children = <MessageCatalog>[
        StaticCatalog({'en': {'hi': 'Static'}}),
        CompositeCatalog([lazy]),
      ];
      final catalog = CompositeCatalog(children);
      children.clear();
      await catalog.loadForLocale('ru-RU');
      expect(locales, containsAll(['ru-RU', 'ru', 'en']));
      expect(catalog.getMessage('en', 'hi'), 'Static');
      expect(catalog.getMessage('en', 'only-lazy'), 'Loaded');
      expect(() => catalog.catalogs.clear(), throwsUnsupportedError);
    });

    test('composite retries failed children without reloading successful ones',
        () async {
      var attempts = 0;
      var goodCalls = 0;
      final catalog = CompositeCatalog([
        LazyCatalog(loader: (_) {
          if (++attempts == 1) throw StateError('offline');
          return 'hi = Recovered\n';
        }),
        LazyCatalog(loader: (_) {
          goodCalls++;
          return 'hi = Fallback\n';
        }),
      ]);
      await expectLater(catalog.loadForLocale('en'), throwsStateError);
      expect(catalog.getMessage('en', 'hi'), 'Fallback');
      await catalog.loadForLocale('en');
      expect(catalog.getMessage('en', 'hi'), 'Recovered');
      expect(attempts, 2);
      expect(goodCalls, 1);
    });

    test('loads a locale once and then serves it synchronously', () async {
      var loads = 0;
      final catalog = LazyCatalog(loader: (locale) async {
        loads++;
        return locale.split('-').first == 'ru' ? 'hi = Привет\n' : null;
      });

      expect(catalog.getMessage('ru-RU', 'hi'), isNull);
      await Future.wait([
        catalog.loadLocale('ru-RU'),
        catalog.loadLocale('ru_RU'),
      ]);
      expect(loads, 1);
      expect(catalog.getMessage('ru-RU', 'hi'), 'Привет');
      expect(catalog.getSupportedLocales(), {'ru-RU'});

      await catalog.loadLocale('de');
      expect(catalog.isLoaded('de'), isTrue);
      expect(catalog.hasMessage('de', 'hi'), isFalse);
    });

    test('a failed load is retried, not cached', () async {
      var attempts = 0;
      final catalog = LazyCatalog(loader: (locale) async {
        attempts++;
        if (attempts == 1) throw StateError('network down');
        return 'hi = Hi\n';
      });

      await expectLater(catalog.loadLocale('en'), throwsA(isA<StateError>()));
      expect(catalog.isLoaded('en'), isFalse);

      await catalog.loadLocale('en');
      expect(catalog.getMessage('en', 'hi'), 'Hi');
      expect(attempts, 2);
    });

    test('loadForLocale preloads the whole fallback chain', () async {
      final requested = <String>[];
      final catalog = LazyCatalog(loader: (locale) async {
        requested.add(locale);
        return null;
      });
      await catalog.loadForLocale('ru-RU');
      expect(requested, containsAll(['ru-RU', 'ru', 'en']));
    });

    test('UiMessages resolves through a loaded LazyCatalog', () async {
      final catalog = LazyCatalog(loader: (locale) async {
        return locale == 'en' ? 'welcome = Welcome\n' : null;
      });
      final messages = UiMessages(catalog: catalog);
      expect(messages.t('welcome', locale: 'en'), 'welcome');
      await catalog.loadLocale('en');
      expect(messages.t('welcome', locale: 'en'), 'Welcome');
    });
  });

  group('CURRENCY()', () {
    test('symbol placement follows the locale', () {
      expect(formatFluentCurrency(1234.5, currency: 'USD', locale: 'en'),
          '\$1,234.50');
      expect(formatFluentCurrency(1234.5, currency: 'EUR', locale: 'ru'),
          '1\u00A0234,50\u00A0€');
      expect(formatFluentCurrency(3, currency: 'CHF', locale: 'en'),
          'CHF\u00A03.00');
    });

    test('uses the standard fraction digits of the currency', () {
      // The symbol table is locale-independent and follows the currency's own
      // locale, so JPY is the Japanese `￥` (U+FFE5) in every locale, as `￥` is
      // what CLDR writes in `ja`; CLDR `en` writes `¥` (U+00A5) instead.
      expect(formatFluentCurrency(1234.4, currency: 'JPY', locale: 'en'),
          '￥1,234');
      expect(formatFluentCurrency(1.5, currency: 'KWD', locale: 'en'),
          'KWD\u00A01.500');
    });

    test('negative amounts keep the sign in front of the symbol', () {
      expect(formatFluentCurrency(-5, currency: 'USD', locale: 'en'),
          '-\$5.00');
    });

    test('display "code" shows the ISO code instead of the symbol', () {
      expect(
        formatFluentCurrency(1234.5,
            currency: 'USD', locale: 'en', display: 'code'),
        'USD\u00A01,234.50',
      );
    });

    test('NUMBER() with style currency in a bundle', () {
      final bundle = FluentBundle(locale: 'en', useIsolating: false);
      bundle.addSource(
        'price = { NUMBER(\$v, style: "currency", currency: "USD") }\n'
        'bad = { NUMBER(\$v, style: "currency", currency: "us") }\n',
      );
      expect(bundle.format('price', args: {'v': 3}), '\$3.00');
      expect(bundle.format('bad', args: {'v': 1234.5}), '1,234.5');
    });
  });

  group('Regression: review fixes', () {
    test('plural selection works on NUMBER() output with grouping', () {
      final bundle = FluentBundle(locale: 'en', useIsolating: false);
      bundle.addSource(
        'items = { NUMBER(\$n) ->\n'
        '    [one] one item\n'
        '   *[other] many items\n'
        '}\n',
      );
      expect(bundle.format('items', args: {'n': 1}), 'one item');
      expect(bundle.format('items', args: {'n': 1234}), 'many items');
    });

    test('a loader that throws synchronously is retried, not cached', () async {
      var attempts = 0;
      final catalog = LazyCatalog(loader: (locale) {
        attempts++;
        if (attempts == 1) throw StateError('sync failure');
        return Future<String?>.value('hi = Hi\n');
      });

      await expectLater(catalog.loadLocale('en'), throwsA(isA<StateError>()));
      await catalog.loadLocale('en');
      expect(catalog.getMessage('en', 'hi'), 'Hi');
      expect(attempts, 2);
    });
  });

  group('Fluent 1.0 conformance (engineering audit fixes)', () {
    List<String> textsOf(FtlResource resource, String id) {
      final pattern = resource.messages[id]?.value;
      if (pattern == null) return const [];
      return pattern.elements.whereType<FtlTextElement>().map((e) => e.value).toList();
    }

    test('trailing blank_inline is trimmed from a value', () {
      final resource = FtlParser.parse('key = Value with trailing space   \nnext = ok\n');

      expect(resource.junk, isEmpty);
      expect(textsOf(resource, 'key'), ['Value with trailing space']);
      expect(textsOf(resource, 'next'), ['ok']);
    });

    test('a tab after "=" belongs to the value; elsewhere it is junk', () {
      final value = FtlParser.parse('key = \tValue\t01\n');
      expect(value.junk, isEmpty);
      expect(textsOf(value, 'key'), ['\tValue\t01']);

      // A tab is not blank_inline, so it cannot separate an ID from "=".
      final brokenKey = FtlParser.parse('key\t= Value\n');
      expect(brokenKey.messages, isEmpty);
      expect(brokenKey.junk.single.message, contains('Expected "="'));
    });

    test('a tab-indented line is not a pattern continuation', () {
      final resource = FtlParser.parse('key =\n\tIndented with a tab\n');

      expect(resource.messages, isEmpty);
      expect(resource.junk, hasLength(1));
      expect(resource.junk.single.message, contains('must have a value'));
    });

    test('a term or attribute without a value is junk', () {
      final term = FtlParser.parse('-term =\nnext = ok\n');
      expect(term.terms, isEmpty);
      expect(term.junk, hasLength(1));
      expect(term.messages['next']?.value, isNotNull);

      final attribute = FtlParser.parse('key =\n    .attr =\nnext = ok\n');
      expect(attribute.messages.containsKey('key'), isFalse);
      expect(attribute.junk, hasLength(1));
      expect(attribute.messages['next']?.value, isNotNull);
    });

    test('every select variant must have a value', () {
      final resource = FtlParser.parse('key = { \$n ->\n   *[other]\n}\n');

      expect(resource.messages, isEmpty);
      expect(resource.junk, hasLength(1));
      expect(resource.junk.single.message, contains('must have a value'));
    });

    test('a variant key must be an identifier or a number', () {
      final resource = FtlParser.parse('key = { \$n ->\n   *[a b] text\n}\n');

      expect(resource.messages, isEmpty);
      expect(
        resource.junk.single.message,
        anyOf(
          contains('Invalid variant key'),
          contains('Expected "]" after the variant key'),
        ),
      );
    });

    test('an unescaped "}" is junk, including inside a variant value', () {
      final resource =
          FtlParser.parse('key = { \$n ->\n    [one] a}b\n   *[other] c\n}\n');

      expect(resource.messages, isEmpty);
      expect(resource.junk, hasLength(1));
    });

    test('blank lines may separate a value from its attributes', () {
      final resource = FtlParser.parse('key = Value\n\n   .attr = Attr\n');

      expect(resource.junk, isEmpty);
      expect(resource.messages['key']?.attributes.single.id, 'attr');
    });

    test('whitespace-only lines inside a value collapse to newlines', () {
      final resource = FtlParser.parse('key =\n  first\n  \n\n  last\n');
      final pattern = resource.messages['key']!.value!;

      expect(pattern.elements.whereType<FtlTextElement>().single.value,
          'first\n\n\nlast');
    });

    test('a comment block ends when the marker level changes', () {
      final resource = FtlParser.parse('# one\n## two\n### three\nkey = ok\n');
      final comments = resource.entries.whereType<FtlComment>().toList();

      expect(resource.junk, isEmpty);
      expect(comments.map((c) => c.content), ['one', 'two', 'three']);
      expect(comments.map((c) => c.level), [0, 1, 2]);
      expect(resource.messages['key']?.value, isNotNull);
    });

    test('a comment marker must be followed by a space', () {
      final resource = FtlParser.parse('#one\nkey = ok\n');

      expect(resource.entries.whereType<FtlComment>(), isEmpty);
      expect(resource.junk, hasLength(1));
      expect(resource.messages['key']?.value, isNotNull);
    });

    test('callee rules: term attributes are values only as selectors', () {
      final invalid = FtlParser.parse(
        'key = { -term.attr }\n-term = value\n    .attr = attribute\n',
      );
      expect(invalid.messages.containsKey('key'), isFalse);
      expect(invalid.junk.single.message, contains('Term attributes'));

      final selector = FtlParser.parse(
        'key = { -term.attr ->\n   *[other] ok\n}\n'
        '-term = value\n    .attr = other\n',
      );
      expect(selector.junk, isEmpty);
      expect(selector.terms['term']?.attributes.single.id, 'attr');
    });

    test('a message reference cannot be a selector', () {
      final resource =
          FtlParser.parse('other = text\nkey = { other ->\n   *[other] ok\n}\n');

      expect(resource.messages.containsKey('key'), isFalse);
      expect(resource.junk, hasLength(1));
    });

    test('BiDi isolation is skipped for lone and literal placeables', () {
      final bundle = FluentBundle(locale: 'en');
      bundle.addSource('''
lone = { \$name }
mixed = Hello, { \$name }!
literal = { "quoted" } and { \$name }
''');

      expect(bundle.format('lone', args: {'name': 'Alex'}), 'Alex');
      expect(bundle.format('mixed', args: {'name': 'Alex'}),
          'Hello, ${bidiFsi}Alex${bidiPdi}!');
      expect(bundle.format('literal', args: {'name': 'Alex'}),
          'quoted and ${bidiFsi}Alex${bidiPdi}');
    });

    test('a referenced message sees the caller variables', () {
      final bundle = FluentBundle(locale: 'en', useIsolating: false);
      bundle.addSource('''
greeting = Hello, { \$name }
wrapped = [{ greeting }]
''');

      expect(bundle.format('wrapped', args: {'name': 'Alex'}), '[Hello, Alex]');
      // Messages resolved through the bundle keep the documented fallback.
      expect(bundle.format('greeting'), 'Hello, {\$name}');
    });

    test('an integer respects minimumFractionDigits without losing precision', () {
      final bundle = FluentBundle(locale: 'en', useIsolating: false);
      bundle.addSource('price = { NUMBER(\$amount, minimumFractionDigits: 2) }\n');

      expect(bundle.format('price', args: {'amount': 3}), '3.00');
      expect(bundle.format('price', args: {'amount': 9007199254740993}),
          '9,007,199,254,740,993.00');
    });

    test('CLDR fractions use the language-specific operands', () {
      expect(resolveCldrPluralCategory('pl', 1.5), 'other');
      expect(resolveCldrPluralCategory('cs', 1.5), 'many');
      expect(resolveCldrPluralCategory('lt', 1.5), 'many');
      expect(resolveCldrPluralCategory('sl', 1.5), 'few');
      expect(resolveCldrPluralCategory('ru', 1.5), 'other');
      expect(resolveCldrPluralCategory('sr', 1.2), 'few');
      expect(resolveCldrPluralCategory('lv', 0.1), 'one');
      expect(resolveCldrPluralCategory('ro', 101), 'few');
      expect(resolveCldrPluralCategory('ar', 3.5), 'other');
      expect(resolveCldrPluralCategory('fr', 1000000), 'many');
      expect(resolveCldrPluralCategory('he', 2), 'two');
      expect(resolveCldrPluralCategory('he', 0.5), 'one');
    });

    test('malformed and unacceptable language ranges are dropped', () {
      expect(parseAcceptLanguage('en;q=0, ru').map((p) => p.tag).toList(), ['ru']);
      expect(parseAcceptLanguage('en;q=abc, ru').map((p) => p.tag).toList(), ['ru']);
      expect(parseAcceptLanguage('en;q=0.5;q=0.8, ru').map((p) => p.tag).toList(),
          ['ru']);
      expect(parseAcceptLanguage('en;q=.5').map((p) => p.tag).toList(), isEmpty);
      expect(parseAcceptLanguage('en;q=1.001').map((p) => p.tag).toList(), isEmpty);
      expect(parseAcceptLanguage('en;q=0.1234').map((p) => p.tag).toList(), isEmpty);
    });

    test('equal qualities keep the original header order', () {
      final prefs = parseAcceptLanguage('ru;q=0.8, de;q=0.8, en;q=0.9');

      expect(prefs.map((p) => p.tag).toList(), ['en', 'ru', 'de']);
    });

    test('legacy and extended tags are canonicalized for catalog lookup', () {
      expect(
        acceptLanguageLocales('iw-IL-u-ca-hebrew;q=0.8, en;q=0.9'),
        ['en', 'he-IL-u-ca-hebrew'],
      );
      expect(acceptLanguageCatalogLocales('iw-IL-u-ca-hebrew;q=0.8'), ['he-IL']);
    });

    test('a translator comment is attached to the entry it precedes', () {
      final resource = FtlParser.parse('# Home screen\n'
          '# Shown on first launch\n'
          'welcome = Hello\n'
          '\n'
          '# Term note\n'
          '-term = value\n');

      expect(resource.messages['welcome']!.comment,
          'Home screen\nShown on first launch');
      expect(resource.terms['term']!.comment, 'Term note');
      // An attached comment is part of the entry, not an entry of its own.
      expect(resource.entries.whereType<FtlComment>(), isEmpty);
    });

    test('group comments and comments before junk stay standalone entries', () {
      final resource = FtlParser.parse('## Group\n'
          'key = v\n'
          '\n'
          '# translator note\n'
          'bad line\n'
          'other = w\n');

      expect(resource.messages['key']!.comment, isNull);
      expect(resource.messages['other']!.comment, isNull);
      final comments = resource.entries.whereType<FtlComment>().toList();
      expect(comments.map((c) => c.level).toList(), <int>[1, 0]);
      expect(
          comments.map((c) => c.content).toList(), <String>['Group', 'translator note']);
      expect(resource.junk.map((j) => j.content).toList(), <String>['bad line\n']);
    });

    test('a nested placeable resolves to the value it wraps', () {
      final bundle = FluentBundle(locale: 'en', useIsolating: false);
      bundle.addSource('wrapped = [{ { \$n } }]\n'
          'digits = { NUMBER({ \$n }, minimumFractionDigits: 2) }\n');

      expect(bundle.format('wrapped', args: {'n': 5}), '[5]');
      expect(bundle.format('digits', args: {'n': 5}), '5.00');
    });

    test('junk keeps lines that start with "//" or "[["', () {
      final resource = FtlParser.parse('key = {\n'
          '// not a comment\n'
          '[[not a block]]\n'
          'next = 1\n');

      expect(resource.junk.map((j) => j.content).toList(),
          <String>['key = {\n// not a comment\n[[not a block]]\n']);
      expect(resource.messages.keys, contains('next'));
    });

    test('junk ends before a line that starts a new entry', () {
      final resource = FtlParser.parse('key = {\n'
          'other = \$value\n'
          'next = 1\n');

      expect(resource.junk.map((j) => j.content).toList(), <String>['key = {\n']);
      expect(resource.messages.keys, containsAll(<String>['other', 'next']));
    });
  });
}

/// An external adapter may throw before returning its Future.
class _FailingCatalog extends StaticCatalog {
  @override
  Future<void> loadForLocale(
    String locale, {
    String defaultLocale = 'en',
    String platformFallback = 'en',
  }) {
    throw StateError('synchronous adapter failure');
  }
}

class _LookupCountingCatalog extends StaticCatalog {
  int lookups = 0;

  @override
  String? getMessage(String locale, String id, {
    Map<String, dynamic>? args,
    String? attribute,
  }) {
    lookups++;
    return 'unexpected lookup';
  }
}
