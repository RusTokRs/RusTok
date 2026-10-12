import 'package:test/test.dart';
import 'package:rustok_i18n/rustok_i18n.dart';

void main() {
  group('LocaleTag & BCP-47 Normalization', () {
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
      expect(resource.junk.single.message, contains('Invalid variant key'));
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
          'Hello, $bidiFsiAlex$bidiPdi!');
      expect(bundle.format('literal', args: {'name': 'Alex'}),
          'quoted and $bidiFsiAlex$bidiPdi');
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
