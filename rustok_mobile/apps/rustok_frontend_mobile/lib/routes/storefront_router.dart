import 'package:app_module_contracts/app_module_contracts.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:rustok_catalog_mobile/rustok_catalog_mobile.dart';

import '../app_shell/storefront_context.dart';
import '../app_shell/storefront_shell_page.dart';
import '../data/storefront_catalog_repository.dart';
import '../registry/storefront_surface_registry.dart';

const homePath = '/';
const catalogPath = '/catalog';
const cartPath = '/cart';
const profilePath = '/profile';
const storefrontModulesRootPath = '/modules';

final storefrontRouterProvider = Provider<GoRouter>((ref) {
  ref.watch(storefrontGraphQlClientProvider);
  final catalogRepository = ref.watch(hostStorefrontCatalogRepositoryProvider);
  return buildStorefrontRouter(catalogRepository: catalogRepository);
});

GoRouter buildStorefrontRouter({
  StorefrontCatalogRepository? catalogRepository,
}) {
  return GoRouter(
    initialLocation: homePath,
    routes: [
      ShellRoute(
        builder: (context, state, child) => StorefrontShellPage(child: child),
        routes: [
          GoRoute(
            path: homePath,
            builder: (context, state) => const StorefrontHomePage(),
          ),
          GoRoute(
            path: catalogPath,
            builder: (context, state) => ProviderScope(
              overrides: [
                if (catalogRepository != null)
                  storefrontCatalogRepositoryProvider.overrideWithValue(
                    catalogRepository,
                  ),
              ],
              child: StorefrontCatalogScreen(
                onOpenCart: () => context.go(cartPath),
              ),
            ),
          ),
          GoRoute(
            path: cartPath,
            builder: (context, state) => ProviderScope(
              overrides: [
                if (catalogRepository != null)
                  storefrontCatalogRepositoryProvider.overrideWithValue(
                    catalogRepository,
                  ),
              ],
              child: StorefrontCartScreen(
                onContinueShopping: () => context.go(catalogPath),
              ),
            ),
          ),
          GoRoute(
            path: profilePath,
            builder: (context, state) => const StorefrontPlaceholderPage(
              title: 'Profile',
              description: 'Customer account surfaces will mount here.',
              icon: Icons.person_outline,
            ),
          ),
          GoRoute(
            path: '$storefrontModulesRootPath/:routeSegment',
            builder: (context, state) {
              final routeSegment = state.pathParameters['routeSegment'] ?? '';
              final surface = storefrontSurfaceRegistry.resolve(
                routeSegment,
              );
              return StorefrontModuleSurfacePage(
                surface: surface,
                catalogRepository: catalogRepository,
              );
            },
          ),
        ],
      ),
    ],
  );
}

class StorefrontHomePage extends ConsumerWidget {
  const StorefrontHomePage({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final runtime = ref.watch(storefrontRuntimeContextProvider);
    final config = ref.watch(storefrontGraphQlConfigProvider);
    return ListView(
      children: [
        const ListTile(
          title: Text('Mobile storefront host'),
          subtitle: Text(
            'Customer-facing Flutter shell aligned with the existing storefront contract.',
          ),
        ),
        _RuntimeContextCard(runtime: runtime, graphqlEndpoint: config.httpUri),
        const _SurfaceLinkCard(
          title: 'Catalog',
          subtitle: 'Browse products through the storefront route contract.',
          icon: Icons.category_outlined,
          path: catalogPath,
        ),
        const _SurfaceLinkCard(
          title: 'Cart',
          subtitle: 'Prepare checkout without admin/operator affordances.',
          icon: Icons.shopping_cart_outlined,
          path: cartPath,
        ),
        const _StorefrontModuleLinksCard(registry: storefrontSurfaceRegistry),
      ],
    );
  }
}

class _RuntimeContextCard extends StatelessWidget {
  const _RuntimeContextCard({
    required this.runtime,
    required this.graphqlEndpoint,
  });

  final StorefrontRuntimeContext runtime;
  final Uri graphqlEndpoint;

  @override
  Widget build(BuildContext context) {
    return Card(
      margin: const EdgeInsets.all(12),
      child: ListTile(
        leading: const Icon(Icons.public),
        title: Text('tenant: ${runtime.tenantSlug} · locale: ${runtime.locale}'),
        subtitle: Text('GraphQL: $graphqlEndpoint'),
      ),
    );
  }
}

class _StorefrontModuleLinksCard extends StatelessWidget {
  const _StorefrontModuleLinksCard({required this.registry});

  final StorefrontSurfaceRegistry registry;

  @override
  Widget build(BuildContext context) {
    final entries = registry.entries;
    if (entries.isEmpty) {
      return const _SurfaceLinkCard(
        title: 'Storefront modules',
        subtitle: 'No generated storefront module routes are available.',
        icon: Icons.extension_outlined,
        path: storefrontModulesRootPath,
      );
    }

    return Card(
      margin: const EdgeInsets.symmetric(horizontal: 12, vertical: 6),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          const ListTile(
            leading: Icon(Icons.extension_outlined),
            title: Text('Storefront modules'),
            subtitle: Text(
              'Generated module routes from the storefront mobile manifest.',
            ),
          ),
          const Divider(height: 1),
          for (final entry in entries)
            _StorefrontModuleLinkTile(
              entry: entry,
              surfaceKind: registry.resolve(entry.routeSegment).kind,
            ),
        ],
      ),
    );
  }
}

class _StorefrontModuleLinkTile extends StatelessWidget {
  const _StorefrontModuleLinkTile({
    required this.entry,
    required this.surfaceKind,
  });

  final MobileModuleEntry entry;
  final StorefrontMountedSurfaceKind surfaceKind;

  @override
  Widget build(BuildContext context) {
    return ListTile(
      leading: Icon(_iconFor(surfaceKind)),
      title: Text(entry.nav.title),
      subtitle: Text('/modules/${entry.routeSegment}'),
      trailing: Text(_labelFor(surfaceKind)),
      onTap: () => context.go(
        '$storefrontModulesRootPath/${entry.routeSegment}',
      ),
    );
  }
}

IconData _iconFor(StorefrontMountedSurfaceKind kind) {
  return switch (kind) {
    StorefrontMountedSurfaceKind.catalog => Icons.category_outlined,
    StorefrontMountedSurfaceKind.cart => Icons.shopping_cart_outlined,
    StorefrontMountedSurfaceKind.generic => Icons.extension_outlined,
  };
}

String _labelFor(StorefrontMountedSurfaceKind kind) {
  return switch (kind) {
    StorefrontMountedSurfaceKind.catalog => 'package',
    StorefrontMountedSurfaceKind.cart => 'package',
    StorefrontMountedSurfaceKind.generic => 'manifest',
  };
}

class _SurfaceLinkCard extends StatelessWidget {
  const _SurfaceLinkCard({
    required this.title,
    required this.subtitle,
    required this.icon,
    required this.path,
  });

  final String title;
  final String subtitle;
  final IconData icon;
  final String path;

  @override
  Widget build(BuildContext context) {
    return Card(
      margin: const EdgeInsets.symmetric(horizontal: 12, vertical: 6),
      child: ListTile(
        leading: Icon(icon),
        title: Text(title),
        subtitle: Text(subtitle),
        trailing: const Icon(Icons.chevron_right),
        onTap: () => context.go(path),
      ),
    );
  }
}

class StorefrontPlaceholderPage extends StatelessWidget {
  const StorefrontPlaceholderPage({
    super.key,
    required this.title,
    required this.description,
    required this.icon,
  });

  final String title;
  final String description;
  final IconData icon;

  @override
  Widget build(BuildContext context) {
    return Center(
      child: Padding(
        padding: const EdgeInsets.all(24),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(icon, size: 48),
            const SizedBox(height: 12),
            Text(title, style: Theme.of(context).textTheme.headlineSmall),
            const SizedBox(height: 8),
            Text(description, textAlign: TextAlign.center),
          ],
        ),
      ),
    );
  }
}

class StorefrontModuleSurfacePage extends StatelessWidget {
  const StorefrontModuleSurfacePage({
    super.key,
    required this.surface,
    this.catalogRepository,
  });

  final StorefrontSurfaceMatch surface;
  final StorefrontCatalogRepository? catalogRepository;

  @override
  Widget build(BuildContext context) {
    return switch (surface.kind) {
      StorefrontMountedSurfaceKind.catalog => ProviderScope(
        overrides: [
          if (catalogRepository != null)
            storefrontCatalogRepositoryProvider.overrideWithValue(
              catalogRepository!,
            ),
        ],
        child: StorefrontCatalogScreen(
          onOpenCart: () => context.go('$storefrontModulesRootPath/cart'),
        ),
      ),
      StorefrontMountedSurfaceKind.cart => ProviderScope(
        overrides: [
          if (catalogRepository != null)
            storefrontCatalogRepositoryProvider.overrideWithValue(
              catalogRepository!,
            ),
        ],
        child: StorefrontCartScreen(
          onContinueShopping: () => context.go(
            '$storefrontModulesRootPath/products',
          ),
        ),
      ),
      StorefrontMountedSurfaceKind.generic => StorefrontPlaceholderPage(
        title: surface.title ?? 'Module: ${surface.routeSegment ?? 'unknown'}',
        description: 'Manifest-driven storefront mobile surface.',
        icon: Icons.extension_outlined,
      ),
    };
  }
}
