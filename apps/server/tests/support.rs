use rustok_auth::{AuthConfig, build_auth_config_from_document};
use rustok_test_utils::JwtKeyringFixture;

/// Build a fresh four-purpose RSA runtime for one isolated server test store.
/// The MAC root is fixed test-only material and is independent of all RSA keys.
pub fn test_auth_config() -> AuthConfig {
    let fixture = JwtKeyringFixture::generate().expect("generate isolated RSA JWT fixture");
    let document = fixture.document(1).to_string();
    let (_, config) = build_auth_config_from_document(
        "test-only-independent-mac-root-with-at-least-32-bytes".to_string(),
        900,
        Default::default(),
        document.as_bytes(),
    )
    .expect("prepare four independent RS256 purpose rings");
    config
}
