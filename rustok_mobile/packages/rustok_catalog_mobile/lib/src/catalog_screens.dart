import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'catalog_repository.dart';
import 'product_summary.dart';

class StorefrontCatalogScreen extends ConsumerWidget {
  const StorefrontCatalogScreen({super.key, this.onOpenCart});

  final VoidCallback? onOpenCart;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final products = ref.watch(featuredProductsProvider);
    return products.when(
      loading: () => const _CenteredProgress(label: 'Loading catalog…'),
      error: (error, _) => _StorefrontErrorView(
        title: 'Catalog is unavailable',
        message: error.toString(),
      ),
      data: (items) {
        if (items.isEmpty) {
          return const _EmptyStorefrontSurface(
            icon: Icons.category_outlined,
            title: 'Catalog is empty',
            message: 'No storefront products are available yet.',
          );
        }

        return ListView.separated(
          padding: const EdgeInsets.all(16),
          itemCount: items.length + 1,
          separatorBuilder: (_, __) => const SizedBox(height: 12),
          itemBuilder: (context, index) {
            if (index == 0) {
              return _CatalogHeader(onOpenCart: onOpenCart);
            }
            return _ProductCard(product: items[index - 1]);
          },
        );
      },
    );
  }
}

class StorefrontCartScreen extends ConsumerWidget {
  const StorefrontCartScreen({super.key, this.onContinueShopping});

  final VoidCallback? onContinueShopping;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final lines = ref.watch(cartLinesProvider);
    return lines.when(
      loading: () => const _CenteredProgress(label: 'Loading cart…'),
      error: (error, _) => _StorefrontErrorView(
        title: 'Cart is unavailable',
        message: error.toString(),
      ),
      data: (items) {
        if (items.isEmpty) {
          return _EmptyStorefrontSurface(
            icon: Icons.shopping_cart_outlined,
            title: 'Cart is empty',
            message: 'Add products from the catalog to prepare checkout.',
            actionLabel: 'Open catalog',
            onAction: onContinueShopping,
          );
        }

        return ListView.separated(
          padding: const EdgeInsets.all(16),
          itemCount: items.length + 1,
          separatorBuilder: (_, __) => const SizedBox(height: 12),
          itemBuilder: (context, index) {
            if (index == 0) {
              return _CartHeader(onContinueShopping: onContinueShopping);
            }
            return _CartLineTile(line: items[index - 1]);
          },
        );
      },
    );
  }
}

class _CatalogHeader extends StatelessWidget {
  const _CatalogHeader({this.onOpenCart});

  final VoidCallback? onOpenCart;

  @override
  Widget build(BuildContext context) {
    return Card(
      child: ListTile(
        leading: const Icon(Icons.storefront_outlined),
        title: const Text('Catalog'),
        subtitle: const Text(
          'Module-owned mobile surface mounted by the storefront host.',
        ),
        trailing: FilledButton.icon(
          onPressed: onOpenCart,
          icon: const Icon(Icons.shopping_cart_outlined),
          label: const Text('Cart'),
        ),
      ),
    );
  }
}

class _CartHeader extends StatelessWidget {
  const _CartHeader({this.onContinueShopping});

  final VoidCallback? onContinueShopping;

  @override
  Widget build(BuildContext context) {
    return Card(
      child: ListTile(
        leading: const Icon(Icons.shopping_cart_outlined),
        title: const Text('Cart'),
        subtitle: const Text(
          'Customer checkout preview without admin affordances.',
        ),
        trailing: TextButton(
          onPressed: onContinueShopping,
          child: const Text('Continue shopping'),
        ),
      ),
    );
  }
}

class _ProductCard extends StatelessWidget {
  const _ProductCard({required this.product});

  final StorefrontProductSummary product;

  @override
  Widget build(BuildContext context) {
    return Card(
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Row(
              children: [
                Expanded(
                  child: Text(
                    product.title,
                    style: Theme.of(context).textTheme.titleMedium,
                  ),
                ),
                if (product.badge != null) Chip(label: Text(product.badge!)),
              ],
            ),
            const SizedBox(height: 8),
            Text(product.description),
            const SizedBox(height: 12),
            Text(
              product.priceLabel,
              style: Theme.of(context).textTheme.titleSmall,
            ),
          ],
        ),
      ),
    );
  }
}

class _CartLineTile extends StatelessWidget {
  const _CartLineTile({required this.line});

  final StorefrontCartLine line;

  @override
  Widget build(BuildContext context) {
    return Card(
      child: ListTile(
        leading: CircleAvatar(child: Text('${line.quantity}×')),
        title: Text(line.title),
        subtitle: Text('Product: ${line.productId}'),
        trailing: Text(line.priceLabel),
      ),
    );
  }
}

class _CenteredProgress extends StatelessWidget {
  const _CenteredProgress({required this.label});

  final String label;

  @override
  Widget build(BuildContext context) {
    return Center(
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          const CircularProgressIndicator(),
          const SizedBox(height: 12),
          Text(label),
        ],
      ),
    );
  }
}

class _EmptyStorefrontSurface extends StatelessWidget {
  const _EmptyStorefrontSurface({
    required this.icon,
    required this.title,
    required this.message,
    this.actionLabel,
    this.onAction,
  });

  final IconData icon;
  final String title;
  final String message;
  final String? actionLabel;
  final VoidCallback? onAction;

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
            Text(message, textAlign: TextAlign.center),
            if (actionLabel != null && onAction != null) ...[
              const SizedBox(height: 16),
              FilledButton(onPressed: onAction, child: Text(actionLabel!)),
            ],
          ],
        ),
      ),
    );
  }
}

class _StorefrontErrorView extends StatelessWidget {
  const _StorefrontErrorView({required this.title, required this.message});

  final String title;
  final String message;

  @override
  Widget build(BuildContext context) {
    return Center(
      child: Padding(
        padding: const EdgeInsets.all(24),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            const Icon(Icons.error_outline, size: 48),
            const SizedBox(height: 12),
            Text(title, style: Theme.of(context).textTheme.headlineSmall),
            const SizedBox(height: 8),
            Text(message, textAlign: TextAlign.center),
          ],
        ),
      ),
    );
  }
}
