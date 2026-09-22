import 'package:graphql/client.dart';

import 'auth_session_store.dart';

abstract class RefreshTokenService {
  Future<AuthSession?> refresh(AuthSession session);
}

class GraphQlRefreshTokenService implements RefreshTokenService {
  GraphQlRefreshTokenService({required this.client});

  final GraphQLClient client;

  static final MutationOptions _refreshMutation = MutationOptions(
    document: gql(r'''
      mutation RefreshToken($refreshToken: String!) {
        refreshToken(input: { refreshToken: $refreshToken }) {
          accessToken
          refreshToken
          expiresIn
        }
      }
    '''),
  );

  @override
  Future<AuthSession?> refresh(AuthSession session) async {
    final refreshToken = session.refreshToken;
    if (refreshToken == null || refreshToken.isEmpty) {
      return null;
    }

    final result = await client.mutate(
      _refreshMutation.copyWith(
        variables: <String, dynamic>{'refreshToken': refreshToken},
      ),
    );

    if (result.hasException) {
      return null;
    }

    final payload = result.data?['refreshToken'] as Map<String, dynamic>?;
    if (payload == null) {
      return null;
    }

    final accessToken = payload['accessToken'] as String?;
    if (accessToken == null || accessToken.isEmpty) {
      return null;
    }

    final expiresIn = payload['expiresIn'] as int?;
    return AuthSession(
      accessToken: accessToken,
      refreshToken: payload['refreshToken'] as String? ?? refreshToken,
      expiresAt: expiresIn == null
          ? null
          : DateTime.now().add(Duration(seconds: expiresIn)),
    );
  }
}
