import 'request_context.dart';

class GraphQlHeadersBuilder {
  const GraphQlHeadersBuilder();

  Map<String, String> build(GraphQlRequestContext context) {
    final headers = <String, String>{
      'X-Tenant-Slug': context.tenantSlug,
      'Accept-Language': context.locale,
    };

    if (context.tenantId != null && context.tenantId!.isNotEmpty) {
      headers['X-Tenant-ID'] = context.tenantId!;
    }
    if (context.accessToken != null && context.accessToken!.isNotEmpty) {
      headers['Authorization'] = 'Bearer ${context.accessToken!}';
    }
    return Map.unmodifiable(headers);
  }

  Map<String, Object?> buildWsInitPayload(GraphQlRequestContext context) {
    return {
      'tenantSlug': context.tenantSlug,
      'locale': context.locale,
      if (context.accessToken != null && context.accessToken!.isNotEmpty)
        'token': context.accessToken,
    };
  }
}
