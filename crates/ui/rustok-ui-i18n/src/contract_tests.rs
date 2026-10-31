use super::*;

#[test]
fn test_lib_contract_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<UiMessages>();
    assert_send_sync::<LazyUiMessages>();
}

#[test]
fn test_lib_contract_locale_candidates() {
    let candidates = locale_candidates(Some("ru-RU"), "en");
    assert_eq!(candidates, vec!["ru-RU", "ru", "en"]);

    let empty = locale_candidates(None, "en");
    assert_eq!(empty, vec!["en"]);
}

#[test]
fn test_lib_contract_admin_locale() {
    assert_eq!(normalize_admin_locale(Some("ru")), "ru");
    assert_eq!(normalize_admin_locale(Some("en")), "en");
    assert_eq!(normalize_admin_locale(None), "en");
}

#[test]
fn test_lib_contract_accept_language() {
    let header = "ru-RU,ru;q=0.9,en;q=0.8";
    let preferred = preferred_locale_from_accept_language(Some(header));
    assert_eq!(preferred, Some("ru-RU".to_string()));
}
