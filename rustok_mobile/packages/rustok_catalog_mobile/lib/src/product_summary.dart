class StorefrontProductSummary {
  const StorefrontProductSummary({
    required this.id,
    required this.title,
    required this.description,
    required this.priceLabel,
    this.badge,
  });

  final String id;
  final String title;
  final String description;
  final String priceLabel;
  final String? badge;
}

class StorefrontCartLine {
  const StorefrontCartLine({
    required this.productId,
    required this.title,
    required this.quantity,
    required this.priceLabel,
  });

  final String productId;
  final String title;
  final int quantity;
  final String priceLabel;
}
