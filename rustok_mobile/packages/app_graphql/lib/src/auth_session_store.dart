class AuthSession {
  const AuthSession({
    required this.accessToken,
    this.refreshToken,
    this.expiresAt,
  });

  final String accessToken;
  final String? refreshToken;
  final DateTime? expiresAt;

  bool get isExpired => isExpiredAt(DateTime.now());

  bool isExpiredAt(DateTime now) {
    return expiresAt != null && now.isAfter(expiresAt!);
  }
}

abstract class AuthSessionStore {
  Future<AuthSession?> read();

  Future<void> write(AuthSession session);

  Future<void> clear();
}

class InMemoryAuthSessionStore implements AuthSessionStore {
  AuthSession? _session;

  @override
  Future<void> clear() async {
    _session = null;
  }

  @override
  Future<AuthSession?> read() async => _session;

  @override
  Future<void> write(AuthSession session) async {
    _session = session;
  }
}
