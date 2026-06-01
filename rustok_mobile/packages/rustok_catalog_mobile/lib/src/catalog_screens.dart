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
          return _EmptyCartSurface(onContinueShopping: onContinueShopping);
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

class _ProductCard extends ConsumerStatefulWidget {
  const _ProductCard({required this.product});

  final StorefrontProductSummary product;

  @override
  ConsumerState<_ProductCard> createState() => _ProductCardState();
}

class _ProductCardState extends ConsumerState<_ProductCard> {
  bool _busy = false;

  @override
  Widget build(BuildContext context) {
    final product = widget.product;
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
            Row(
              children: [
                Expanded(
                  child: Text(
                    product.priceLabel,
                    style: Theme.of(context).textTheme.titleSmall,
                  ),
                ),
                FilledButton.icon(
                  onPressed: _busy || !product.canAddToCart
                      ? null
                      : () => _addToCart(product),
                  icon: _busy
                      ? const SizedBox.square(
                          dimension: 16,
                          child: CircularProgressIndicator(strokeWidth: 2),
                        )
                      : const Icon(Icons.add_shopping_cart_outlined),
                  label: Text(
                    product.canAddToCart ? 'Add to cart' : 'Select option',
                  ),
                ),
              ],
            ),
          ],
        ),
      ),
    );
  }

  Future<void> _addToCart(StorefrontProductSummary product) async {
    setState(() => _busy = true);
    try {
      final variantId = product.variantId?.trim();
      if (variantId == null || variantId.isEmpty) {
        throw StateError('Product option is required before adding to cart.');
      }
      await ref.read(storefrontCatalogRepositoryProvider).addCartLine(
            StorefrontAddCartLineDraft(variantId: variantId),
          );
      ref.invalidate(cartLinesProvider);
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text('${product.title} added to cart')),
        );
      }
    } catch (error) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text('Unable to add to cart: $error')),
        );
      }
    } finally {
      if (mounted) {
        setState(() => _busy = false);
      }
    }
  }
}

class _CartLineTile extends ConsumerStatefulWidget {
  const _CartLineTile({required this.line});

  final StorefrontCartLine line;

  @override
  ConsumerState<_CartLineTile> createState() => _CartLineTileState();
}

class _CartLineTileState extends ConsumerState<_CartLineTile> {
  bool _busy = false;

  @override
  Widget build(BuildContext context) {
    final line = widget.line;
    return Card(
      child: ListTile(
        leading: CircleAvatar(child: Text('${line.quantity}×')),
        title: Text(line.title),
        subtitle: Text('Product: ${line.productId}'),
        trailing: Wrap(
          spacing: 4,
          crossAxisAlignment: WrapCrossAlignment.center,
          children: [
            Text(line.priceLabel),
            IconButton(
              tooltip: 'Decrease quantity',
              onPressed: _busy
                  ? null
                  : () => _updateQuantity(line, line.quantity - 1),
              icon: const Icon(Icons.remove_circle_outline),
            ),
            IconButton(
              tooltip: 'Increase quantity',
              onPressed: _busy
                  ? null
                  : () => _updateQuantity(line, line.quantity + 1),
              icon: const Icon(Icons.add_circle_outline),
            ),
            IconButton(
              tooltip: 'Remove item',
              onPressed: _busy ? null : () => _remove(line),
              icon: const Icon(Icons.delete_outline),
            ),
          ],
        ),
      ),
    );
  }

  Future<void> _updateQuantity(StorefrontCartLine line, int quantity) async {
    if (quantity <= 0) {
      await _remove(line);
      return;
    }
    setState(() => _busy = true);
    try {
      await ref.read(storefrontCatalogRepositoryProvider).updateCartLine(
            StorefrontUpdateCartLineDraft(
              lineId: line.lineId,
              quantity: quantity,
            ),
          );
      ref.invalidate(cartLinesProvider);
    } catch (error) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text('Unable to update cart: $error')),
        );
      }
    } finally {
      if (mounted) {
        setState(() => _busy = false);
      }
    }
  }

  Future<void> _remove(StorefrontCartLine line) async {
    setState(() => _busy = true);
    try {
      await ref.read(storefrontCatalogRepositoryProvider).removeCartLine(
            line.lineId,
          );
      ref.invalidate(cartLinesProvider);
    } catch (error) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text('Unable to remove cart item: $error')),
        );
      }
    } finally {
      if (mounted) {
        setState(() => _busy = false);
      }
    }
  }
}

class _EmptyCartSurface extends ConsumerStatefulWidget {
  const _EmptyCartSurface({this.onContinueShopping});

  final VoidCallback? onContinueShopping;

  @override
  ConsumerState<_EmptyCartSurface> createState() => _EmptyCartSurfaceState();
}

class _EmptyCartSurfaceState extends ConsumerState<_EmptyCartSurface> {
  bool _busy = false;

  @override
  Widget build(BuildContext context) {
    return _EmptyStorefrontSurface(
      icon: Icons.shopping_cart_outlined,
      title: 'Cart is empty',
      message: 'Add products from the catalog to prepare checkout.',
      actionLabel: _busy ? 'Starting cart…' : 'Start cart',
      onAction: _busy ? null : _createCart,
      secondaryActionLabel: 'Open catalog',
      onSecondaryAction: widget.onContinueShopping,
    );
  }

  Future<void> _createCart() async {
    setState(() => _busy = true);
    try {
      await ref.read(storefrontCatalogRepositoryProvider).createCart(
            const StorefrontCreateCartDraft(),
          );
      ref.invalidate(cartLinesProvider);
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(content: Text('Cart started')),
        );
      }
    } catch (error) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text('Unable to start cart: $error')),
        );
      }
    } finally {
      if (mounted) {
        setState(() => _busy = false);
      }
    }
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
    this.secondaryActionLabel,
    this.onSecondaryAction,
  });

  final IconData icon;
  final String title;
  final String message;
  final String? actionLabel;
  final VoidCallback? onAction;
  final String? secondaryActionLabel;
  final VoidCallback? onSecondaryAction;

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
            if (actionLabel != null) ...[
              const SizedBox(height: 16),
              FilledButton(onPressed: onAction, child: Text(actionLabel!)),
            ],
            if (secondaryActionLabel != null && onSecondaryAction != null) ...[
              const SizedBox(height: 8),
              TextButton(
                onPressed: onSecondaryAction,
                child: Text(secondaryActionLabel!),
              ),
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
