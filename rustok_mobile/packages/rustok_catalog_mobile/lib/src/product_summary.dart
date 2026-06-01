class StorefrontProductSummary {
  const StorefrontProductSummary({
    required this.id,
    required this.title,
    required this.description,
    required this.priceLabel,
    this.variantId,
    this.badge,
  });

  final String id;
  final String title;
  final String description;
  final String priceLabel;
  final String? variantId;
  final String? badge;

  bool get canAddToCart => variantId != null && variantId!.trim().isNotEmpty;
}

class StorefrontCartLine {
  const StorefrontCartLine({
    required this.lineId,
    required this.productId,
    required this.title,
    required this.quantity,
    required this.priceLabel,
  });

  final String lineId;
  final String productId;
  final String title;
  final int quantity;
  final String priceLabel;
}
