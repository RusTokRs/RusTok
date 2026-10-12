import 'package:rustok_i18n/rustok_i18n.dart';

void main() {
  final catalog = FluentCatalog(useIsolating: false);

  // Register English FTL resource
  catalog.registerFtl('en', '''
-brand-name = RusToK
welcome = Welcome to { -brand-name }!
email-input = Email address
    .placeholder = name@example.com
items-count = { \$count ->
    [0] No items in cart
    [one] 1 item in cart
   *[other] { \$count } items in cart
}
''');

  // Register Russian FTL resource
  catalog.registerFtl('ru', '''
-brand-name = RusToK
welcome = Добро пожаловать в { -brand-name }!
email-input = Электронная почта
    .placeholder = name@example.com
items-count = { \$count ->
    [0] Корзина пуста
    [one] { \$count } товар в корзине
    [few] { \$count } товара в корзине
   *[other] { \$count } товаров в корзине
}
''');

  final messages = UiMessages(
    catalog: catalog,
    defaultLocale: 'en',
  );

  print('--- English ---');
  print(messages.t('welcome'));
  print(messages.t('email-input.placeholder'));
  print(messages.t('items-count', args: {'count': 0}));
  print(messages.t('items-count', args: {'count': 1}));
  print(messages.t('items-count', args: {'count': 5}));

  print('\n--- Russian ---');
  print(messages.t('welcome', locale: 'ru'));
  print(messages.t('items-count', locale: 'ru', args: {'count': 0}));
  print(messages.t('items-count', locale: 'ru', args: {'count': 1}));
  print(messages.t('items-count', locale: 'ru', args: {'count': 2}));
  print(messages.t('items-count', locale: 'ru', args: {'count': 5}));
  print(messages.t('items-count', locale: 'ru', args: {'count': 21}));

  print('\n--- High-performance batch translator ---');
  final ruTranslator = messages.forLocale('ru');
  print(ruTranslator.t('welcome'));
  print(ruTranslator.t('items-count', args: {'count': 42}));
}
