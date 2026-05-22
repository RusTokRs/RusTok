import 'package:app_graphql/app_graphql.dart';
import 'package:test/test.dart';

void main() {
  group('AuthSession', () {
    test('isExpiredAt returns false when expiresAt is null', () {
      const session = AuthSession(accessToken: 'token');
      expect(session.isExpiredAt(DateTime.utc(2026, 1, 1)), isFalse);
    });

    test('isExpiredAt returns false when now equals expiresAt', () {
      final expiresAt = DateTime.utc(2026, 1, 1, 0, 0, 0);
      final session = AuthSession(accessToken: 'token', expiresAt: expiresAt);
      expect(session.isExpiredAt(expiresAt), isFalse);
    });

    test('isExpiredAt returns true when now is after expiresAt', () {
      final expiresAt = DateTime.utc(2026, 1, 1, 0, 0, 0);
      final session = AuthSession(accessToken: 'token', expiresAt: expiresAt);
      expect(session.isExpiredAt(expiresAt.add(const Duration(seconds: 1))), isTrue);
    });
  });
}
