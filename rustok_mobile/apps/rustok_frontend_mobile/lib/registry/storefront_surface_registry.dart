import 'package:app_module_contracts/app_module_contracts.dart';

import 'storefront_mobile_manifest.g.dart' as generated;

const storefrontSurfaceRegistry = StorefrontSurfaceRegistry(
  generated.generatedMobileManifest,
);

enum StorefrontMountedSurfaceKind { catalog, cart, generic }

class StorefrontSurfaceRegistry {
  const StorefrontSurfaceRegistry(this.entries);

  final List<MobileModuleEntry> entries;

  StorefrontSurfaceMatch resolve(String routeSegment) {
    final normalized = normalizeStorefrontRouteSegment(routeSegment);
    if (normalized.isEmpty) {
      return const StorefrontSurfaceMatch(
        kind: StorefrontMountedSurfaceKind.generic,
      );
    }

    for (final entry in entries) {
      if (normalizeStorefrontRouteSegment(entry.routeSegment) == normalized) {
        return StorefrontSurfaceMatch(
          kind: _surfaceKindFor(entry),
          entry: entry,
        );
      }
    }

    return StorefrontSurfaceMatch(
      kind: StorefrontMountedSurfaceKind.generic,
      routeSegment: normalized,
    );
  }
}

class StorefrontSurfaceMatch {
  const StorefrontSurfaceMatch({
    required this.kind,
    this.entry,
    this.routeSegment,
  });

  final StorefrontMountedSurfaceKind kind;
  final MobileModuleEntry? entry;
  final String? routeSegment;

  String? get title => entry?.nav.title;
}

StorefrontMountedSurfaceKind _surfaceKindFor(MobileModuleEntry entry) {
  return switch (entry.moduleKey) {
    'rustok_product' => StorefrontMountedSurfaceKind.catalog,
    'rustok_cart' => StorefrontMountedSurfaceKind.cart,
    _ => StorefrontMountedSurfaceKind.generic,
  };
}

String normalizeStorefrontRouteSegment(String value) {
  return value.trim().toLowerCase().replaceAll('_', '-');
}
