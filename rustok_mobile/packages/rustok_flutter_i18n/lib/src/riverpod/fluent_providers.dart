import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:rustok_i18n/rustok_i18n.dart';

/// StateNotifier managing the currently active application locale tag.
class LocaleNotifier extends StateNotifier<String> {
  LocaleNotifier([String initialLocale = 'en']) : super(initialLocale);

  void setLocale(String newLocale) {
    final normalized = LocaleTag.tryParse(newLocale)?.toCatalogTag() ?? newLocale;
    if (state != normalized) {
      state = normalized;
    }
  }
}

/// Riverpod provider holding the current active locale tag (e.g. `ru` or `en`).
final currentLocaleProvider =
    StateNotifierProvider<LocaleNotifier, String>((ref) {
  return LocaleNotifier('en');
});

/// Riverpod provider for the global [MessageCatalog]. Overridden at host bootstrap.
final messageCatalogProvider = Provider<MessageCatalog>((ref) {
  return FluentCatalog();
});

/// Riverpod provider for the default fallback locale (defaults to `'en'`).
final defaultLocaleProvider = Provider<String>((ref) => 'en');

/// Riverpod provider for the platform fallback locale (defaults to `'en'`).
final platformFallbackProvider = Provider<String>((ref) => 'en');

/// Riverpod provider for the root [UiMessages] engine.
final uiMessagesProvider = Provider<UiMessages>((ref) {
  final catalog = ref.watch(messageCatalogProvider);
  final defaultLocale = ref.watch(defaultLocaleProvider);
  final platformFallback = ref.watch(platformFallbackProvider);
  return UiMessages(
    catalog: catalog,
    defaultLocale: defaultLocale,
    platformFallback: platformFallback,
  );
});

/// Reactive translation function provider bound to the active locale.
///
/// Usage in ConsumerWidget:
/// ```dart
/// final t = ref.watch(trProvider);
/// return Text(t('auth.login_title'));
/// ```
final trProvider = Provider<String Function(String, {Map<String, dynamic>? args, String? fallback})>((ref) {
  final messages = ref.watch(uiMessagesProvider);
  final locale = ref.watch(currentLocaleProvider);
  return (key, {args, fallback}) => messages.t(
        key,
        locale: locale,
        args: args,
        fallback: fallback,
      );
});

/// Reactive provider indicating if the currently active locale is RTL.
final isRtlLocaleProvider = Provider<bool>((ref) {
  final locale = ref.watch(currentLocaleProvider);
  return LocaleTag.tryParse(locale)?.isRtl ?? false;
});
