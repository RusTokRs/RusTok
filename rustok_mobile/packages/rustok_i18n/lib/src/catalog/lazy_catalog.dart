import '../locale/fallback_chain.dart';
import '../locale/locale_tag.dart';
import '../resolver/fluent_resolver.dart';
import 'message_catalog.dart';

/// Loads the Fluent source for one locale on demand.
///
/// Receives the canonical locale tag (e.g. `ru-RU`). Returns `null` when the
/// locale has no source. May read assets, files, or a remote endpoint.
typedef FtlSourceLoader = Future<String?> Function(String locale);

/// A catalog that loads each locale once, asynchronously, and then serves it
/// synchronously.
///
/// Lookups are synchronous, so a locale must be loaded before its messages
/// are visible. Use [loadLocale] or [loadForLocale] first. Until then, lookups
/// miss and `UiMessages` falls back to the default locale or the key.
///
/// A failed load is not cached: calling [loadLocale] again retries it.
/// A locale whose loader returns `null` is marked as loaded and empty, so it
/// is not requested again.
class LazyCatalog extends MessageCatalog {
  LazyCatalog({required this.loader, this.useIsolating = true});

  final FtlSourceLoader loader;
  final bool useIsolating;

  /// Catalog key -> bundle. A `null` value means the locale was loaded but has no source.
  final Map<String, FluentBundle?> _bundles = {};
  final Map<String, Future<void>> _inFlight = {};

  /// Whether the locale has finished loading, including when it has no source.
  bool isLoaded(String locale) => _bundles.containsKey(catalogLocaleKey(locale));

  /// Loads [locale] once. Concurrent calls for the same locale share one request.
  Future<void> loadLocale(String locale) {
    final canonical = LocaleTag.tryParse(locale)?.toCatalogTag() ?? locale.trim();
    final key = catalogLocaleKey(canonical);
    if (_bundles.containsKey(key)) return Future<void>.value();

    return _inFlight.putIfAbsent(key, () async {
      try {
        final source = await loader(canonical);
        _bundles[key] = source == null
            ? null
            : (FluentBundle(locale: canonical, useIsolating: useIsolating)
              ..addSource(source));
      } finally {
        _inFlight.remove(key);
      }
    });
  }

  /// Loads every locale that [locale] can fall back to, in priority order.
  Future<void> loadForLocale(
    String locale, {
    String defaultLocale = 'en',
    String platformFallback = 'en',
  }) {
    final candidates = localeCandidates(
      locale: locale,
      defaultLocale: defaultLocale,
      platformFallback: platformFallback,
    );
    return Future.wait(candidates.map(loadLocale));
  }

  /// Diagnostics for [locale]'s source, empty if none or not loaded.
  List<String> diagnosticsFor(String locale) =>
      _bundles[catalogLocaleKey(locale)]?.diagnostics ?? const <String>[];

  @override
  bool hasMessage(String locale, String id) {
    return _bundles[catalogLocaleKey(locale)]?.hasMessage(id) ?? false;
  }

  @override
  String? getMessage(
    String locale,
    String id, {
    Map<String, dynamic>? args,
    String? attribute,
  }) {
    return _bundles[catalogLocaleKey(locale)]?.format(
      id,
      args: args,
      attribute: attribute,
    );
  }

  @override
  Set<String> getSupportedLocales() => {
        for (final bundle in _bundles.values)
          if (bundle != null) bundle.locale,
      };
}
