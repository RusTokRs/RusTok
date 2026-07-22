use rustok_ui_i18n_leptos::LeptosUiMessages;

use crate::core::{AuthTransportErrorKind, classify_auth_transport_error};

static MESSAGES: LeptosUiMessages = LeptosUiMessages::new(
    "en",
    &[
        ("en", include_str!("../locales/en.json")),
        ("ru", include_str!("../locales/ru.json")),
    ],
);

pub fn t(locale: Option<&str>, key: &str, fallback: &str) -> String {
    MESSAGES.t_for_locale(locale, key, fallback)
}

pub fn auth_transport_error_message(locale: Option<&str>, error: &str) -> String {
    let (key, fallback) = match classify_auth_transport_error(error) {
        AuthTransportErrorKind::Unauthorized => (
            "errors.auth.unauthorized",
            "You are not authorized to perform this action.",
        ),
        AuthTransportErrorKind::Http => ("errors.http", "Server error. Please try again."),
        AuthTransportErrorKind::Network => {
            ("errors.network", "Network error. Check your connection.")
        }
        AuthTransportErrorKind::Unknown => {
            ("errors.unknown", "Something went wrong. Please try again.")
        }
    };
    t(locale, key, fallback)
}

#[cfg(test)]
mod tests {
    use super::{auth_transport_error_message, t};

    #[test]
    fn resolves_host_locale_and_falls_back_to_english() {
        assert_eq!(t(Some("ru-RU"), "users.title", "Users"), "Пользователи");
        assert_eq!(t(Some("fr"), "users.title", "Users"), "Users");
        assert_eq!(
            auth_transport_error_message(Some("ru"), "Unauthorized"),
            "Недостаточно прав для выполнения действия."
        );
        assert_eq!(
            auth_transport_error_message(Some("en"), "GraphQL failed"),
            "Something went wrong. Please try again."
        );
    }
}
