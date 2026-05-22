import 'package:graphql/client.dart';

import 'client_config.dart';

class GraphQlClientFactory {
  const GraphQlClientFactory();

  GraphQLClient create(GraphQlClientConfig config) {
    return _createClient(config, includeSubscriptions: true);
  }

  GraphQLClient createHttpOnly(GraphQlClientConfig config) {
    return _createClient(config, includeSubscriptions: false);
  }

  GraphQLClient _createClient(
    GraphQlClientConfig config, {
    required bool includeSubscriptions,
  }) {
    final authLink = AuthLink(getToken: () async {
      final token = config.context.accessToken;
      if (token == null || token.isEmpty) {
        return null;
      }
      return 'Bearer $token';
    });

    final headers = Map<String, String>.from(config.headers)
      ..remove('Authorization');

    final httpLink = HttpLink(
      config.httpUri.toString(),
      defaultHeaders: headers,
    );

    final Link transportLink;
    if (includeSubscriptions) {
      final wsLink = WebSocketLink(
        config.wsUri.toString(),
        config: SocketClientConfig(
          initialPayload: () => config.wsInitPayload,
        ),
      );

      transportLink = authLink.concat(
        Link.split((request) => request.isSubscription, wsLink, httpLink),
      );
    } else {
      transportLink = authLink.concat(httpLink);
    }

    return GraphQLClient(
      cache: GraphQLCache(),
      link: transportLink,
      queryRequestTimeout: Duration(milliseconds: config.requestTimeoutMs),
    );
  }
}
