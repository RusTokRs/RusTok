class GraphQlRequestContext {
  const GraphQlRequestContext({
    required this.tenantSlug,
    required this.locale,
    this.tenantId,
    this.accessToken,
  }) : assert(tenantSlug.trim().isNotEmpty),
       assert(locale.trim().isNotEmpty);

  final String tenantSlug;
  final String locale;
  final String? tenantId;
  final String? accessToken;
}
