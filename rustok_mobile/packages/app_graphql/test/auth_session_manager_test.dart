import 'package:app_graphql/app_graphql.dart';
import 'package:test/test.dart';

void main() {
  group('AuthSessionManager', () {
    test('returns current session when it is not expired', () async {
      final store = InMemoryAuthSessionStore();
      const session = AuthSession(accessToken: 'token');
      await store.write(session);
      final manager = AuthSessionManager(
        store: store,
        refreshTokenService: _NoopRefreshService(),
      );

      final resolved = await manager.readValidSession();

      expect(resolved?.accessToken, 'token');
    });

    test('refreshes expired session and persists refreshed session', () async {
      final store = InMemoryAuthSessionStore();
      final expired = AuthSession(
        accessToken: 'old-token',
        refreshToken: 'refresh',
        expiresAt: DateTime.now().subtract(const Duration(minutes: 1)),
      );
      await store.write(expired);
      final manager = AuthSessionManager(
        store: store,
        refreshTokenService: _StubRefreshService(
          AuthSession(accessToken: 'new-token', refreshToken: 'refresh-2'),
        ),
      );

      final resolved = await manager.readValidSession();
      final persisted = await store.read();

      expect(resolved?.accessToken, 'new-token');
      expect(persisted?.refreshToken, 'refresh-2');
    });

    test('clears session when refresh failed', () async {
      final store = InMemoryAuthSessionStore();
      final expired = AuthSession(
        accessToken: 'old-token',
        refreshToken: 'refresh',
        expiresAt: DateTime.now().subtract(const Duration(minutes: 1)),
      );
      await store.write(expired);
      final manager = AuthSessionManager(
        store: store,
        refreshTokenService: _StubRefreshService(null),
      );

      final resolved = await manager.readValidSession();
      final persisted = await store.read();

      expect(resolved, isNull);
      expect(persisted, isNull);
    });
  });
}

class _NoopRefreshService implements RefreshTokenService {
  @override
  Future<AuthSession?> refresh(AuthSession session) async => session;
}

class _StubRefreshService implements RefreshTokenService {
  _StubRefreshService(this._nextSession);

  final AuthSession? _nextSession;

  @override
  Future<AuthSession?> refresh(AuthSession session) async => _nextSession;
}
