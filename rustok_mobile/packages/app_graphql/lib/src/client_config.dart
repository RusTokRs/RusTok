import 'endpoints.dart';
import 'headers_builder.dart';
import 'request_context.dart';

class GraphQlClientConfig {
  const GraphQlClientConfig({
    required this.baseUri,
    required this.context,
    this.endpoints = const GraphQlEndpoints(),
    this.connectTimeoutMs = 10000,
    this.requestTimeoutMs = 30000,
  });

  final Uri baseUri;
  final GraphQlRequestContext context;
  final GraphQlEndpoints endpoints;
  final int connectTimeoutMs;
  final int requestTimeoutMs;

  Uri get httpUri => endpoints.httpUri(baseUri);
  Uri get wsUri => endpoints.wsUri(baseUri);

  Map<String, String> get headers =>
      const GraphQlHeadersBuilder().build(context);

  Map<String, Object?> get wsInitPayload =>
      const GraphQlHeadersBuilder().buildWsInitPayload(context);
}
