use std::{env, time::Duration};

use super::{
    PageInlineEditConfigError, PageInlineEditKeyId, PageInlineEditKeyring, PageInlineEditSecret,
};

pub const PAGES_INLINE_EDIT_HMAC_KEY_ENV: &str = "RUSTOK_PAGES_INLINE_EDIT_HMAC_KEY";
pub const PAGES_INLINE_EDIT_HMAC_KEY_ID_ENV: &str = "RUSTOK_PAGES_INLINE_EDIT_HMAC_KEY_ID";
pub const PAGES_INLINE_EDIT_GRANT_TTL_MS_ENV: &str = "RUSTOK_PAGES_INLINE_EDIT_GRANT_TTL_MS";

pub fn page_inline_edit_keyring_from_environment()
-> Result<Option<PageInlineEditKeyring>, PageInlineEditConfigError> {
    let secret = match env::var(PAGES_INLINE_EDIT_HMAC_KEY_ENV) {
        Ok(secret) => PageInlineEditSecret::new(secret)?,
        Err(env::VarError::NotPresent) => return Ok(None),
        Err(env::VarError::NotUnicode(_)) => {
            return Err(PageInlineEditConfigError::InvalidSecret);
        }
    };
    let key_id = match env::var(PAGES_INLINE_EDIT_HMAC_KEY_ID_ENV) {
        Ok(key_id) => PageInlineEditKeyId::new(key_id)?,
        Err(env::VarError::NotPresent) => PageInlineEditKeyId::new("active")?,
        Err(env::VarError::NotUnicode(_)) => {
            return Err(PageInlineEditConfigError::InvalidKeyId);
        }
    };
    let keyring = PageInlineEditKeyring::new(key_id.clone(), vec![(key_id, secret)])?;
    match env::var(PAGES_INLINE_EDIT_GRANT_TTL_MS_ENV) {
        Ok(raw_ttl) => {
            let ttl_ms = raw_ttl
                .parse::<u64>()
                .map_err(|_| PageInlineEditConfigError::InvalidTtl)?;
            keyring.with_ttl(Duration::from_millis(ttl_ms)).map(Some)
        }
        Err(env::VarError::NotPresent) => Ok(Some(keyring)),
        Err(env::VarError::NotUnicode(_)) => Err(PageInlineEditConfigError::InvalidTtl),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn environment_names_are_pages_owned_and_specific() {
        assert_eq!(
            PAGES_INLINE_EDIT_HMAC_KEY_ENV,
            "RUSTOK_PAGES_INLINE_EDIT_HMAC_KEY"
        );
        assert_eq!(
            PAGES_INLINE_EDIT_HMAC_KEY_ID_ENV,
            "RUSTOK_PAGES_INLINE_EDIT_HMAC_KEY_ID"
        );
        assert_eq!(
            PAGES_INLINE_EDIT_GRANT_TTL_MS_ENV,
            "RUSTOK_PAGES_INLINE_EDIT_GRANT_TTL_MS"
        );
    }
}
