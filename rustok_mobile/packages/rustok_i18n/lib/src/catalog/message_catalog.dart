import '../locale/locale_tag.dart';
import '../resolver/fluent_resolver.dart';

/// Canonical storage key for a locale tag: BCP-47 form, lower-cased.
///
/// `ru_RU`, `ru-ru` and `RU-RU` all map to `ru-ru`, so catalogs accept any
/// spelling and lookups with the canonical candidates from `localeCandidates`
/// always find them.
String catalogLocaleKey(String locale) {
  final tag = LocaleTag.tryParse(locale);
  return (tag?.toCatalogTag() ?? locale.trim()).toLowerCase();
}

/// Canonical spelling of a locale tag for display (e.g. `ru_ru` -> `ru-RU`).
String _canonicalLocaleName(String locale) =>
    LocaleTag.tryParse(locale)?.toCatalogTag() ?? locale.trim();

final RegExp _placeholderPattern = RegExp(r'\{([A-Za-z_][A-Za-z0-9_-]*)\}');

/// Replaces `{name}` placeholders in a single pass.
///
/// Values are inserted verbatim and never re-scanned, so an argument whose
/// value looks like a placeholder is not substituted a second time. Unknown
/// placeholders are left unchanged.
String _substituteArgs(String template, Map<String, dynamic>? args) {
  if (args == null || args.isEmpty) return template;
  return template.replaceAllMapped(_placeholderPattern, (match) {
    final name = match.group(1)!;
    if (!args.containsKey(name)) return match.group(0)!;
    return '${args[name]}';
  });
}

/// Abstract contract for all pluggable message catalogs.
abstract class MessageCatalog {
  const MessageCatalog();

  /// Checks if this catalog has a message for [locale] and [id].
  bool hasMessage(String locale, String id);

  /// Resolves and formats a message value. Returns `null` if not found.
  ///
  /// When [attribute] is given, only that attribute is considered: a message
  /// without the attribute yields `null`, never its main value.
  String? getMessage(
    String locale,
    String id, {
    Map<String, dynamic>? args,
    String? attribute,
  });

  /// Returns the set of locales explicitly provided by this catalog,
  /// spelled as BCP-47 tags (e.g. `ru-RU`).
  Set<String> getSupportedLocales();
}

/// Project Fluent (.ftl) message catalog.
class FluentCatalog extends MessageCatalog {
  FluentCatalog({this.useIsolating = true});

  final bool useIsolating;
  final Map<String, FluentBundle> _bundles = {};

  void registerBundle(FluentBundle bundle) {
    _bundles[catalogLocaleKey(bundle.locale)] = bundle;
  }

  /// Adds Fluent source for [locale]. Several calls for the same locale merge
  /// their messages; the first definition of a duplicate ID wins.
  void registerFtl(String locale, String source) {
    final key = catalogLocaleKey(locale);
    final bundle = _bundles.putIfAbsent(
      key,
      () => FluentBundle(
        locale: _canonicalLocaleName(locale),
        useIsolating: useIsolating,
      ),
    );
    bundle.addSource(source);
  }

  /// Diagnostics (junk entries, duplicate IDs) for [locale], empty if none.
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
  Set<String> getSupportedLocales() =>
      _bundles.values.map((bundle) => bundle.locale).toSet();
}

/// In-memory static match table catalog for ultra-fast O(1) lookups.
///
/// Mirrors the compile-time zero-allocation pattern of `rustok-core::i18n`.
/// Attributes are stored under the flat key `'$id.$attribute'`.
class StaticCatalog extends MessageCatalog {
  StaticCatalog([Map<String, Map<String, String>>? initialTables]) {
    if (initialTables != null) {
      for (final entry in initialTables.entries) {
        registerStatic(entry.key, entry.value);
      }
    }
  }

  final Map<String, Map<String, String>> _tables = {};
  final Map<String, String> _localeNames = {};

  void registerStatic(String locale, Map<String, String> table) {
    final key = catalogLocaleKey(locale);
    _localeNames.putIfAbsent(key, () => _canonicalLocaleName(locale));
    _tables.putIfAbsent(key, () => {}).addAll(table);
  }

  @override
  bool hasMessage(String locale, String id) {
    return _tables[catalogLocaleKey(locale)]?.containsKey(id) ?? false;
  }

  @override
  String? getMessage(
    String locale,
    String id, {
    Map<String, dynamic>? args,
    String? attribute,
  }) {
    final table = _tables[catalogLocaleKey(locale)];
    if (table == null) return null;
    final template = attribute == null ? table[id] : table['$id.$attribute'];
    if (template == null) return null;
    return _substituteArgs(template, args);
  }

  @override
  Set<String> getSupportedLocales() => _localeNames.values.toSet();
}

/// JSON-compatible catalog adapter for legacy dictionaries and nested JSON maps.
///
/// Nested maps are flattened with `.` separators. `null` values are ignored.
/// List values are not supported and are stored as their string form.
class JsonCatalog extends MessageCatalog {
  JsonCatalog();

  final Map<String, Map<String, String>> _flattened = {};
  final Map<String, String> _localeNames = {};

  void registerJson(String locale, Map<dynamic, dynamic> json) {
    final key = catalogLocaleKey(locale);
    _localeNames.putIfAbsent(key, () => _canonicalLocaleName(locale));
    final target = _flattened.putIfAbsent(key, () => {});
    _flatten('', json, target);
  }

  void _flatten(String prefix, Map<dynamic, dynamic> map, Map<String, String> out) {
    for (final entry in map.entries) {
      final key = prefix.isEmpty ? '${entry.key}' : '$prefix.${entry.key}';
      final value = entry.value;
      if (value == null) continue;
      if (value is Map) {
        _flatten(key, value, out);
      } else {
        out[key] = '$value';
      }
    }
  }

  @override
  bool hasMessage(String locale, String id) {
    return _flattened[catalogLocaleKey(locale)]?.containsKey(id) ?? false;
  }

  @override
  String? getMessage(
    String locale,
    String id, {
    Map<String, dynamic>? args,
    String? attribute,
  }) {
    final table = _flattened[catalogLocaleKey(locale)];
    if (table == null) return null;
    final template = attribute == null ? table[id] : table['$id.$attribute'];
    if (template == null) return null;
    return _substituteArgs(template, args);
  }

  @override
  Set<String> getSupportedLocales() => _localeNames.values.toSet();
}

/// Composite catalog that queries multiple catalogs in priority order.
class CompositeCatalog extends MessageCatalog {
  CompositeCatalog(this.catalogs);

  final List<MessageCatalog> catalogs;

  @override
  bool hasMessage(String locale, String id) {
    for (final cat in catalogs) {
      if (cat.hasMessage(locale, id)) return true;
    }
    return false;
  }

  @override
  String? getMessage(
    String locale,
    String id, {
    Map<String, dynamic>? args,
    String? attribute,
  }) {
    for (final cat in catalogs) {
      final msg = cat.getMessage(locale, id, args: args, attribute: attribute);
      if (msg != null) return msg;
    }
    return null;
  }

  @override
  Set<String> getSupportedLocales() {
    final all = <String>{};
    for (final cat in catalogs) {
      all.addAll(cat.getSupportedLocales());
    }
    return all;
  }
}
