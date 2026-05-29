import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import '../app_shell/storefront_context.dart';
import '../app_shell/storefront_shell_page.dart';

const homePath = '/';
const catalogPath = '/catalog';
const cartPath = '/cart';
const profilePath = '/profile';
const storefrontModulesRootPath = '/modules';

final storefrontRouterProvider = Provider<GoRouter>((ref) {
  ref.watch(storefrontGraphQlClientProvider);
  return buildStorefrontRouter();
});

GoRouter buildStorefrontRouter() {
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
            builder: (context, state) => const StorefrontPlaceholderPage(
              title: 'Catalog',
              description: 'Product listing surface will mount here.',
              icon: Icons.category_outlined,
            ),
          ),
          GoRoute(
            path: cartPath,
            builder: (context, state) => const StorefrontPlaceholderPage(
              title: 'Cart',
              description: 'Cart and checkout surfaces will mount here.',
              icon: Icons.shopping_cart_outlined,
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
            builder: (context, state) => StorefrontModulePlaceholderPage(
              routeSegment: state.pathParameters['routeSegment'] ?? '',
            ),
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
        const _SurfaceLinkCard(
          title: 'Storefront module route',
          subtitle: 'Reserved for manifest-driven module storefront surfaces.',
          icon: Icons.extension_outlined,
          path: '$storefrontModulesRootPath/blog',
        ),
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

class StorefrontModulePlaceholderPage extends StatelessWidget {
  const StorefrontModulePlaceholderPage({super.key, required this.routeSegment});

  final String routeSegment;

  @override
  Widget build(BuildContext context) {
    return StorefrontPlaceholderPage(
      title: 'Module: $routeSegment',
      description: 'Future module-owned storefront mobile surface.',
      icon: Icons.extension_outlined,
    );
  }
}
