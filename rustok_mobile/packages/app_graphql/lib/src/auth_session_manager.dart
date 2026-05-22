import 'auth_session_store.dart';
import 'refresh_token_service.dart';

class AuthSessionManager {
  AuthSessionManager({
    required AuthSessionStore store,
    required RefreshTokenService refreshTokenService,
  }) : _store = store,
       _refreshTokenService = refreshTokenService;

  final AuthSessionStore _store;
  final RefreshTokenService _refreshTokenService;

  Future<AuthSession?> readValidSession() async {
    final current = await _store.read();
    if (current == null) {
      return null;
    }

    if (!current.isExpired) {
      return current;
    }

    final refreshed = await _refreshTokenService.refresh(current);
    if (refreshed == null) {
      await _store.clear();
      return null;
    }

    await _store.write(refreshed);
    return refreshed;
  }
}
