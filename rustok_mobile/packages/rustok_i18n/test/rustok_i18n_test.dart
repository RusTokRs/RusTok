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

    test('supports multiline values and escaped braces', () {
      final bundle = FluentBundle(locale: 'en', useIsolating: false);
      bundle.addSource('''
multiline-msg =
    Line 1
    Line 2
escaped-msg = Literal \\{escaped\\} braces
negative-num = Balance: { -42 }
unescaped-str = { "hello\\nworld" }
''');

      expect(bundle.format('multiline-msg'), 'Line 1\nLine 2');
      expect(bundle.format('escaped-msg'), 'Literal {escaped} braces');
      expect(bundle.format('negative-num'), 'Balance: -42');
      expect(bundle.format('unescaped-str'), 'hello\nworld');
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
}
