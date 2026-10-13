import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:rustok_i18n/rustok_i18n.dart';

/// Validates [raw] and returns its canonical BCP-47 form (e.g. `ru_ru` -> `ru-RU`).
///
/// Throws [ArgumentError] for malformed tags so that invalid locales never
/// reach the active state.
String _normalizeLocaleOrThrow(String raw) {
  final tag = LocaleTag.tryParse(raw);
  if (tag == null) {
    throw ArgumentError.value(
      raw,
      'locale',
      'Not a valid BCP-47 locale tag',
    );
  }
  return tag.toCatalogTag();
}

/// Holds the active locale as a canonical BCP-47 tag.
class LocaleNotifier extends StateNotifier<String> {
  LocaleNotifier([String initialLocale = 'en'])
      : super(_normalizeLocaleOrThrow(initialLocale));

  /// Switches the active locale. Throws [ArgumentError] for malformed tags.
  void setLocale(String newLocale) {
    final normalized = _normalizeLocaleOrThrow(newLocale);
    if (state != normalized) {
      state = normalized;
    }
  }
}

/// Riverpod provider holding the active locale tag (e.g. `ru-RU` or `en`).
///
/// Starts from [defaultLocaleProvider]; override that provider to change the initial locale.
final currentLocaleProvider =
    StateNotifierProvider<LocaleNotifier, String>((ref) {
  return LocaleNotifier(ref.read(defaultLocaleProvider));
}, dependencies: [defaultLocaleProvider]);

/// Riverpod provider for the global [MessageCatalog].
///
/// Has no default: the host app must override it at bootstrap, e.g.
/// `ProviderScope(overrides: [messageCatalogProvider.overrideWithValue(catalog)])`.
final messageCatalogProvider = Provider<MessageCatalog>((ref) {
  throw UnimplementedError(
    'messageCatalogProvider must be overridden with a MessageCatalog '
    'in ProviderScope.overrides at app bootstrap.',
  );
}, dependencies: const []);

/// Riverpod provider for the default fallback locale (defaults to `'en'`).
final defaultLocaleProvider =
    Provider<String>((ref) => 'en', dependencies: const []);

/// Riverpod provider for the platform fallback locale (defaults to `'en'`).
final platformFallbackProvider =
    Provider<String>((ref) => 'en', dependencies: const []);

/// Riverpod provider for the root [UiMessages] engine.
final uiMessagesProvider = Provider<UiMessages>((ref) {
  final catalog = ref.watch(messageCatalogProvider);
  final defaultLocale = ref.watch(defaultLocaleProvider);
  final platformFallback = ref.watch(platformFallbackProvider);
  return UiMessages(
    catalog: catalog,
    defaultLocale: _normalizeLocaleOrThrow(defaultLocale),
    platformFallback: _normalizeLocaleOrThrow(platformFallback),
  );
}, dependencies: [
  messageCatalogProvider,
  defaultLocaleProvider,
  platformFallbackProvider,
]);

/// Reactive translation function bound to the active locale.
///
/// Usage in a ConsumerWidget:
/// ```dart
/// final t = ref.watch(trProvider);
/// return Text(t('auth.login_title'));
/// ```
final trProvider = Provider.autoDispose<
    String Function(String, {Map<String, dynamic>? args, String? fallback})>(
  (ref) {
    final messages = ref.watch(uiMessagesProvider);
    final locale = ref.watch(currentLocaleProvider);
    // Loading must also work for consumers without a RiverpodFluentScope.
    ref.watch(localeReadyProvider(locale));
    return (key, {args, fallback}) => messages.t(
          key,
          locale: locale,
          args: args,
          fallback: fallback,
        );
  },
  dependencies: [uiMessagesProvider, currentLocaleProvider, localeReadyProvider],
);

/// Reactive provider indicating whether the active locale is right-to-left.
final isRtlLocaleProvider = Provider<bool>((ref) {
  final locale = ref.watch(currentLocaleProvider);
  return LocaleTag.tryParse(locale)?.isRtl ?? false;
}, dependencies: [currentLocaleProvider]);

/// Completes when the messages for [locale] and its fallback chain are available.
///
/// Loads lazy catalogs, including those nested in composite catalogs. The family
/// is auto-disposed when no longer observed; catalog data/in-flight work stays
/// cached by the catalog. Failures can be retried by invalidating this provider.
final localeReadyProvider =
    FutureProvider.autoDispose.family<void, String>((ref, locale) {
  final messages = ref.watch(uiMessagesProvider);
  return messages.catalog.loadForLocale(
    _normalizeLocaleOrThrow(locale),
    defaultLocale: messages.defaultLocale,
    platformFallback: messages.platformFallback,
  );
}, dependencies: [uiMessagesProvider]);
