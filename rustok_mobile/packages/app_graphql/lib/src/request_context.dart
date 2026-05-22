class GraphQlRequestContext {
  const GraphQlRequestContext({
    required this.tenantSlug,
    required this.locale,
    this.tenantId,
    this.accessToken,
  });

  final String tenantSlug;
  final String locale;
  final String? tenantId;
  final String? accessToken;
}
