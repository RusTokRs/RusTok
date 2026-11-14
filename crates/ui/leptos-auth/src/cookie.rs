//! Strict inspection of the BFF refresh cookie without exposing its value to diagnostics.
//!
//! Absence is distinct from a malformed or ambiguous credential: only the former can
//! establish an anonymous browser scope. The caller still has to authenticate any token.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RefreshCookieError {
    Malformed,
    Duplicate,
}

pub fn inspect_refresh_cookie_headers<'a>(
    headers: impl IntoIterator<Item = &'a str>,
) -> Result<Option<(String, String)>, RefreshCookieError> {
    let mut found = None;
    for header in headers {
        for part in header.split(';').map(str::trim) {
            let Some((name, value)) = part.split_once('=') else {
                if part == crate::ADMIN_REFRESH_COOKIE {
                    return Err(RefreshCookieError::Malformed);
                }
                continue;
            };
            if name.trim() != crate::ADMIN_REFRESH_COOKIE {
                continue;
            }
            if found.is_some() {
                return Err(RefreshCookieError::Duplicate);
            }
            let (tenant, token) =
                crate::parse_refresh_cookie_value(value).ok_or(RefreshCookieError::Malformed)?;
            if tenant.chars().any(char::is_whitespace) || token.chars().any(char::is_whitespace) {
                return Err(RefreshCookieError::Malformed);
            }
            found = Some((tenant.to_owned(), token.to_owned()));
        }
    }
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::{RefreshCookieError, inspect_refresh_cookie_headers};

    #[test]
    fn only_an_absent_cookie_is_an_anonymous_observation() {
        assert_eq!(inspect_refresh_cookie_headers([]), Ok(None));
        assert_eq!(
            inspect_refresh_cookie_headers(["other=1", "theme=dark; language=en"]),
            Ok(None)
        );
        assert_eq!(
            inspect_refresh_cookie_headers(["theme=dark; rustok-admin-refresh-v1=shop.secret"]),
            Ok(Some(("shop".into(), "secret".into())))
        );
    }

    #[test]
    fn malformed_cookie_never_becomes_anonymous() {
        for header in [
            "rustok-admin-refresh-v1",
            "rustok-admin-refresh-v1=",
            "rustok-admin-refresh-v1=shop",
            "rustok-admin-refresh-v1=.token",
            "rustok-admin-refresh-v1=shop.",
            "rustok-admin-refresh-v1=shop.bad token",
        ] {
            assert_eq!(
                inspect_refresh_cookie_headers([header]),
                Err(RefreshCookieError::Malformed),
                "{header}"
            );
        }
    }

    #[test]
    fn duplicate_cookies_are_ambiguous_even_when_values_match() {
        assert_eq!(
            inspect_refresh_cookie_headers([
                "rustok-admin-refresh-v1=shop.one",
                "rustok-admin-refresh-v1=shop.one"
            ]),
            Err(RefreshCookieError::Duplicate)
        );
        assert_eq!(
            inspect_refresh_cookie_headers([
                "rustok-admin-refresh-v1=shop.one; rustok-admin-refresh-v1=other.two"
            ]),
            Err(RefreshCookieError::Duplicate)
        );
        assert_eq!(
            inspect_refresh_cookie_headers([
                "rustok-admin-refresh-v1=bad",
                "rustok-admin-refresh-v1=shop.valid"
            ]),
            Err(RefreshCookieError::Malformed)
        );
    }
}
