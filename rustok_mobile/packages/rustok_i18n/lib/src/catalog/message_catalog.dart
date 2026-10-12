import '../resolver/fluent_resolver.dart';

/// Abstract contract for all pluggable message catalogs.
abstract class MessageCatalog {
  const MessageCatalog();

  /// Checks if this catalog has a message for [locale] and [id].
  bool hasMessage(String locale, String id);

  /// Resolves and formats a message value. Returns `null` if not found.
  String? getMessage(
    String locale,
    String id, {
    Map<String, dynamic>? args,
    String? attribute,
  });

  /// Returns the set of locales explicitly provided by this catalog.
  Set<String> getSupportedLocales();
}

/// Project Fluent (.ftl) message catalog.
class FluentCatalog extends MessageCatalog {
  FluentCatalog({this.useIsolating = true});

  final bool useIsolating;
  final Map<String, FluentBundle> _bundles = {};

  void registerBundle(FluentBundle bundle) {
    _bundles[bundle.locale.toLowerCase()] = bundle;
  }

  void registerFtl(String locale, String source) {
    final key = locale.toLowerCase();
    final bundle = _bundles.putIfAbsent(
      key,
      () => FluentBundle(locale: locale, useIsolating: useIsolating),
    );
    bundle.addSource(source);
  }

  @override
  bool hasMessage(String locale, String id) {
    return _bundles[locale.toLowerCase()]?.hasMessage(id) ?? false;
  }

  @override
  String? getMessage(
    String locale,
    String id, {
    Map<String, dynamic>? args,
    String? attribute,
  }) {
    return _bundles[locale.toLowerCase()]?.format(
      id,
      args: args,
      attribute: attribute,
    );
  }

  @override
  Set<String> getSupportedLocales() => _bundles.keys.toSet();
}

/// In-memory static match table catalog for ultra-fast O(1) lookups.
///
/// Mirrors the compile-time zero-allocation pattern of `rustok-core::i18n`.
class StaticCatalog extends MessageCatalog {
  StaticCatalog([Map<String, Map<String, String>>? initialTables]) {
    if (initialTables != null) {
      for (final entry in initialTables.entries) {
        _tables[entry.key.toLowerCase()] = Map.from(entry.value);
      }
    }
  }

  final Map<String, Map<String, String>> _tables = {};

  void registerStatic(String locale, Map<String, String> table) {
    final key = locale.toLowerCase();
    _tables.putIfAbsent(key, () => {}).addAll(table);
  }

  @override
  bool hasMessage(String locale, String id) {
    return _tables[locale.toLowerCase()]?.containsKey(id) ?? false;
  }

  @override
  String? getMessage(
    String locale,
    String id, {
    Map<String, dynamic>? args,
    String? attribute,
  }) {
    final table = _tables[locale.toLowerCase()];
    if (table == null) return null;
    var message = attribute != null ? (table['$id.$attribute'] ?? table[id]) : table[id];
    if (message == null) return null;

    if (args != null && args.isNotEmpty) {
      for (final arg in args.entries) {
        message = message!.replaceAll('{${arg.key}}', arg.value.toString());
      }
    }
    return message;
  }

  @override
  Set<String> getSupportedLocales() => _tables.keys.toSet();
}

/// JSON-compatible catalog adapter for legacy dictionaries and flat/nested JSON maps.
class JsonCatalog extends MessageCatalog {
  JsonCatalog();

  final Map<String, Map<String, String>> _flattened = {};

  void registerJson(String locale, Map<dynamic, dynamic> json) {
    final key = locale.toLowerCase();
    final target = _flattened.putIfAbsent(key, () => {});
    _flatten('', json, target);
  }

  void _flatten(String prefix, Map<dynamic, dynamic> map, Map<String, String> out) {
    for (final entry in map.entries) {
      final key = prefix.isEmpty ? entry.key.toString() : '$prefix.${entry.key}';
      if (entry.value is Map) {
        _flatten(key, entry.value as Map, out);
      } else {
        out[key] = entry.value.toString();
      }
    }
  }

  @override
  bool hasMessage(String locale, String id) {
    return _flattened[locale.toLowerCase()]?.containsKey(id) ?? false;
  }

  @override
  String? getMessage(
    String locale,
    String id, {
    Map<String, dynamic>? args,
    String? attribute,
  }) {
    final table = _flattened[locale.toLowerCase()];
    if (table == null) return null;
    var message = attribute != null ? (table['$id.$attribute'] ?? table[id]) : table[id];
    if (message == null) return null;

    if (args != null && args.isNotEmpty) {
      for (final arg in args.entries) {
        message = message!.replaceAll('{${arg.key}}', arg.value.toString());
      }
    }
    return message;
  }

  @override
  Set<String> getSupportedLocales() => _flattened.keys.toSet();
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
