/// Internationalization support for RusToK
///
/// Provides localized error messages and validation feedback for system strings
/// (auth errors, validation messages, OAuth errors). Content entity translations
/// (node_translations, product_translations, etc.) are handled by the DB layer.
///
/// ## Adding new keys
///
/// 1. Add a new `match` arm in `translate_inner()` for each locale.
/// 2. Use the namespace convention: `{module}.{context}.{key}`,
///    e.g. `auth.email_already_exists`, `commerce.product.not_found`.
/// 3. For Phase 2+ and pluralization support, migrate to Fluent `.ftl` files.
///
/// Supported locales
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Locale {
    #[default]
    En,
    Ru,
    Es,
    De,
    Fr,
    Zh,
}

impl Locale {
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().split('-').next()? {
            "en" => Some(Locale::En),
            "ru" => Some(Locale::Ru),
            "es" => Some(Locale::Es),
            "de" => Some(Locale::De),
            "fr" => Some(Locale::Fr),
            "zh" => Some(Locale::Zh),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Locale::En => "en",
            Locale::Ru => "ru",
            Locale::Es => "es",
            Locale::De => "de",
            Locale::Fr => "fr",
            Locale::Zh => "zh",
        }
    }
}

/// Look up a translation. Returns `Some(&'static str)` or `None` if key is unknown.
///
/// O(1) — uses `match`, no heap allocation, no HashMap iteration.
fn translate_inner(locale: Locale, key: &str) -> Option<&'static str> {
    match locale {
        Locale::En => translate_en(key),
        Locale::Ru => translate_ru(key),
        Locale::Es => translate_es(key),
        Locale::De => translate_de(key),
        Locale::Fr => translate_fr(key),
        Locale::Zh => translate_zh(key),
    }
}

// ─── English ─────────────────────────────────────────────────────────────────

fn translate_en(key: &str) -> Option<&'static str> {
    Some(match key {
        // Content validation
        "invalid_kind" => "Invalid content type",
        "invalid_format" => "Invalid body format",
        "invalid_locale_length" => "Locale must be 2-10 characters",
        "invalid_locale_format" => "Invalid locale format",
        "position_must_be_non_negative" => "Position must be non-negative",
        "position_too_large" => "Position is too large",
        "depth_must_be_non_negative" => "Depth must be non-negative",
        "depth_too_large" => "Depth is too large (max 100)",
        "reply_count_must_be_non_negative" => "Reply count must be non-negative",
        "slug_empty" => "Slug cannot be empty",
        "slug_too_long" => "Slug is too long (max 255 characters)",
        "slug_invalid_characters" => {
            "Slug can only contain lowercase letters, numbers, and hyphens"
        }
        "slug_hyphen_boundary" => "Slug cannot start or end with a hyphen",
        // Auth
        "auth.email_already_exists" => "A user with this email already exists",
        "auth.invalid_credentials" => "Invalid credentials",
        "auth.user_inactive" => "User is inactive",
        "auth.invalid_refresh_token" => "Invalid or expired refresh token",
        "auth.session_expired" => "Session expired",
        "auth.user_not_found" => "User not found",
        "auth.invalid_reset_token" => "Invalid reset token",
        "auth.invalid_invite_token" => "Invalid invite token",
        "auth.invalid_verification_token" => "Invalid verification token",
        "auth.invalid_or_expired_code" => "Invalid or expired code",
        // OAuth
        "oauth.auth_config_error" => "Authentication configuration error",
        "oauth.pkce_invalid" => "PKCE code verifier is invalid",
        "oauth.redirect_uri_mismatch" => "Redirect URI mismatch",
        "oauth.refresh_no_user" => "Refresh token has no associated user",
        _ => return None,
    })
}

// ─── Russian ──────────────────────────────────────────────────────────────────

fn translate_ru(key: &str) -> Option<&'static str> {
    Some(match key {
        // Content validation
        "invalid_kind" => "Неверный тип контента",
        "invalid_format" => "Неверный формат",
        "invalid_locale_length" => "Локаль должна быть 2-10 символов",
        "invalid_locale_format" => "Неверный формат локали",
        "position_must_be_non_negative" => "Позиция должна быть неотрицательной",
        "position_too_large" => "Позиция слишком большая",
        "depth_must_be_non_negative" => "Глубина должна быть неотрицательной",
        "depth_too_large" => "Глубина слишком большая (макс. 100)",
        "reply_count_must_be_non_negative" => "Количество ответов должно быть неотрицательным",
        "slug_empty" => "Slug не может быть пустым",
        "slug_too_long" => "Slug слишком длинный (макс. 255 символов)",
        "slug_invalid_characters" => "Slug может содержать только строчные буквы, цифры и дефисы",
        "slug_hyphen_boundary" => "Slug не может начинаться или заканчиваться дефисом",
        // Auth
        "auth.email_already_exists" => "Пользователь с таким email уже существует",
        "auth.invalid_credentials" => "Неверные учётные данные",
        "auth.user_inactive" => "Пользователь неактивен",
        "auth.invalid_refresh_token" => "Недействительный или просроченный refresh-токен",
        "auth.session_expired" => "Сессия истекла",
        "auth.user_not_found" => "Пользователь не найден",
        "auth.invalid_reset_token" => "Недействительный токен сброса пароля",
        "auth.invalid_invite_token" => "Недействительный токен приглашения",
        "auth.invalid_verification_token" => "Недействительный токен подтверждения",
        "auth.invalid_or_expired_code" => "Недействительный или просроченный код",
        // OAuth
        "oauth.auth_config_error" => "Ошибка конфигурации аутентификации",
        "oauth.pkce_invalid" => "Неверный верификатор кода PKCE",
        "oauth.redirect_uri_mismatch" => "Несоответствие redirect URI",
        "oauth.refresh_no_user" => "Refresh-токен не связан ни с одним пользователем",
        _ => return None,
    })
}

// ─── Spanish ──────────────────────────────────────────────────────────────────

fn translate_es(key: &str) -> Option<&'static str> {
    Some(match key {
        // Content validation
        "invalid_kind" => "Tipo de contenido inválido",
        "invalid_format" => "Formato de cuerpo inválido",
        "invalid_locale_length" => "La configuración regional debe tener 2-10 caracteres",
        "invalid_locale_format" => "Formato de configuración regional inválido",
        "position_must_be_non_negative" => "La posición debe ser no negativa",
        "position_too_large" => "La posición es demasiado grande",
        "depth_must_be_non_negative" => "La profundidad debe ser no negativa",
        "depth_too_large" => "La profundidad es demasiado grande (máx. 100)",
        "reply_count_must_be_non_negative" => "El recuento de respuestas debe ser no negativo",
        "slug_empty" => "El slug no puede estar vacío",
        "slug_too_long" => "El slug es demasiado largo (máx. 255 caracteres)",
        "slug_invalid_characters" => {
            "El slug solo puede contener letras minúsculas, números y guiones"
        }
        "slug_hyphen_boundary" => "El slug no puede comenzar o terminar con un guion",
        // Auth
        "auth.email_already_exists" => "Ya existe un usuario con este correo electrónico",
        "auth.invalid_credentials" => "Credenciales inválidas",
        "auth.user_inactive" => "El usuario está inactivo",
        "auth.invalid_refresh_token" => "Token de actualización inválido o expirado",
        "auth.session_expired" => "La sesión ha expirado",
        "auth.user_not_found" => "Usuario no encontrado",
        "auth.invalid_reset_token" => "Token de restablecimiento inválido",
        "auth.invalid_invite_token" => "Token de invitación inválido",
        "auth.invalid_verification_token" => "Token de verificación inválido",
        "auth.invalid_or_expired_code" => "Código inválido o expirado",
        // OAuth
        "oauth.auth_config_error" => "Error de configuración de autenticación",
        "oauth.pkce_invalid" => "El verificador de código PKCE es inválido",
        "oauth.redirect_uri_mismatch" => "Discrepancia en el URI de redirección",
        "oauth.refresh_no_user" => "El token de actualización no tiene usuario asociado",
        _ => return None,
    })
}

// ─── German ───────────────────────────────────────────────────────────────────

fn translate_de(key: &str) -> Option<&'static str> {
    Some(match key {
        // Content validation
        "invalid_kind" => "Ungültiger Inhaltstyp",
        "invalid_format" => "Ungültiges Body-Format",
        "invalid_locale_length" => "Locale muss 2-10 Zeichen lang sein",
        "invalid_locale_format" => "Ungültiges Locale-Format",
        "position_must_be_non_negative" => "Position muss nicht-negativ sein",
        "position_too_large" => "Position ist zu groß",
        "depth_must_be_non_negative" => "Tiefe muss nicht-negativ sein",
        "depth_too_large" => "Tiefe ist zu groß (max. 100)",
        "reply_count_must_be_non_negative" => "Antwortanzahl muss nicht-negativ sein",
        "slug_empty" => "Slug darf nicht leer sein",
        "slug_too_long" => "Slug ist zu lang (max. 255 Zeichen)",
        "slug_invalid_characters" => {
            "Slug darf nur Kleinbuchstaben, Zahlen und Bindestriche enthalten"
        }
        "slug_hyphen_boundary" => "Slug darf nicht mit einem Bindestrich beginnen oder enden",
        // Auth
        "auth.email_already_exists" => "Ein Benutzer mit dieser E-Mail-Adresse existiert bereits",
        "auth.invalid_credentials" => "Ungültige Anmeldedaten",
        "auth.user_inactive" => "Benutzer ist inaktiv",
        "auth.invalid_refresh_token" => "Ungültiges oder abgelaufenes Auffrischungstoken",
        "auth.session_expired" => "Sitzung abgelaufen",
        "auth.user_not_found" => "Benutzer nicht gefunden",
        "auth.invalid_reset_token" => "Ungültiges Passwort-Reset-Token",
        "auth.invalid_invite_token" => "Ungültiges Einladungstoken",
        "auth.invalid_verification_token" => "Ungültiges Verifizierungstoken",
        "auth.invalid_or_expired_code" => "Ungültiger oder abgelaufener Code",
        // OAuth
        "oauth.auth_config_error" => "Fehler in der Authentifizierungskonfiguration",
        "oauth.pkce_invalid" => "PKCE-Code-Verifizierer ist ungültig",
        "oauth.redirect_uri_mismatch" => "Redirect-URI stimmt nicht überein",
        "oauth.refresh_no_user" => "Auffrischungstoken hat keinen zugehörigen Benutzer",
        _ => return None,
    })
}

// ─── French ───────────────────────────────────────────────────────────────────

fn translate_fr(key: &str) -> Option<&'static str> {
    Some(match key {
        // Content validation
        "invalid_kind"                      => "Type de contenu invalide",
        "invalid_format"                    => "Format de corps invalide",
        "invalid_locale_length"             => "La locale doit avoir 2 à 10 caractères",
        "invalid_locale_format"             => "Format de locale invalide",
        "position_must_be_non_negative"     => "La position doit être non négative",
        "position_too_large"                => "La position est trop grande",
        "depth_must_be_non_negative"        => "La profondeur doit être non négative",
        "depth_too_large"                   => "La profondeur est trop grande (max. 100)",
        "reply_count_must_be_non_negative"  => "Le nombre de réponses doit être non négatif",
        "slug_empty"                        => "Le slug ne peut pas être vide",
        "slug_too_long"                     => "Le slug est trop long (max. 255 caractères)",
        "slug_invalid_characters"           => "Le slug ne peut contenir que des lettres minuscules, des chiffres et des traits d'union",
        "slug_hyphen_boundary"              => "Le slug ne peut pas commencer ou se terminer par un trait d'union",
        // Auth
        "auth.email_already_exists"         => "Un utilisateur avec cet email existe déjà",
        "auth.invalid_credentials"          => "Identifiants invalides",
        "auth.user_inactive"                => "L'utilisateur est inactif",
        "auth.invalid_refresh_token"        => "Jeton d'actualisation invalide ou expiré",
        "auth.session_expired"              => "La session a expiré",
        "auth.user_not_found"              => "Utilisateur introuvable",
        "auth.invalid_reset_token"          => "Jeton de réinitialisation invalide",
        "auth.invalid_invite_token"         => "Jeton d'invitation invalide",
        "auth.invalid_verification_token"   => "Jeton de vérification invalide",
        "auth.invalid_or_expired_code"      => "Code invalide ou expiré",
        // OAuth
        "oauth.auth_config_error"           => "Erreur de configuration d'authentification",
        "oauth.pkce_invalid"                => "Le vérificateur de code PKCE est invalide",
        "oauth.redirect_uri_mismatch"       => "Incompatibilité de l'URI de redirection",
        "oauth.refresh_no_user"             => "Le jeton d'actualisation n'a pas d'utilisateur associé",
        _ => return None,
    })
}

// ─── Chinese (Simplified) ─────────────────────────────────────────────────────

fn translate_zh(key: &str) -> Option<&'static str> {
    Some(match key {
        // Content validation
        "invalid_kind" => "无效的内容类型",
        "invalid_format" => "无效的正文格式",
        "invalid_locale_length" => "语言环境必须为2-10个字符",
        "invalid_locale_format" => "无效的语言环境格式",
        "position_must_be_non_negative" => "位置必须为非负数",
        "position_too_large" => "位置太大",
        "depth_must_be_non_negative" => "深度必须为非负数",
        "depth_too_large" => "深度太大（最多100）",
        "reply_count_must_be_non_negative" => "回复数必须为非负数",
        "slug_empty" => "Slug不能为空",
        "slug_too_long" => "Slug太长（最多255个字符）",
        "slug_invalid_characters" => "Slug只能包含小写字母、数字和连字符",
        "slug_hyphen_boundary" => "Slug不能以连字符开头或结尾",
        // Auth
        "auth.email_already_exists" => "该邮箱已被注册",
        "auth.invalid_credentials" => "凭据无效",
        "auth.user_inactive" => "用户已被停用",
        "auth.invalid_refresh_token" => "刷新令牌无效或已过期",
        "auth.session_expired" => "会话已过期",
        "auth.user_not_found" => "用户不存在",
        "auth.invalid_reset_token" => "密码重置令牌无效",
        "auth.invalid_invite_token" => "邀请令牌无效",
        "auth.invalid_verification_token" => "验证令牌无效",
        "auth.invalid_or_expired_code" => "验证码无效或已过期",
        // OAuth
        "oauth.auth_config_error" => "身份验证配置错误",
        "oauth.pkce_invalid" => "PKCE代码验证器无效",
        "oauth.redirect_uri_mismatch" => "重定向URI不匹配",
        "oauth.refresh_no_user" => "刷新令牌没有关联用户",
        _ => return None,
    })
}

// ─── Public API ───────────────────────────────────────────────────────────────

/// Translate a key into the given locale.
///
/// Falls back to English if the locale has no translation for this key.
/// Returns the key itself if no translation exists in any supported locale.
pub fn translate(locale: Locale, key: &str) -> String {
    translate_inner(locale, key)
        .or_else(|| translate_inner(Locale::En, key))
        .unwrap_or(key)
        .to_string()
}

/// Extract the best `Locale` from an `Accept-Language` HTTP header value.
///
/// Returns `Locale::En` if the header is missing or unrecognised.
pub fn extract_locale_from_header(accept_language: Option<&str>) -> Locale {
    accept_language
        .and_then(|header| {
            // Parse Accept-Language header (e.g., "en-US,en;q=0.9,ru;q=0.8")
            header
                .split(',')
                .next()
                .and_then(|lang| lang.split(';').next())
                .and_then(|lang| Locale::parse(lang.trim()))
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_locale_from_str() {
        assert_eq!(Locale::parse("en"), Some(Locale::En));
        assert_eq!(Locale::parse("en-US"), Some(Locale::En));
        assert_eq!(Locale::parse("ru"), Some(Locale::Ru));
        assert_eq!(Locale::parse("ru-RU"), Some(Locale::Ru));
        assert_eq!(Locale::parse("invalid"), None);
    }

    #[test]
    fn test_translate_english() {
        assert_eq!(
            translate(Locale::En, "invalid_kind"),
            "Invalid content type"
        );
        assert_eq!(translate(Locale::En, "slug_empty"), "Slug cannot be empty");
    }

    #[test]
    fn test_translate_russian() {
        assert_eq!(
            translate(Locale::Ru, "invalid_kind"),
            "Неверный тип контента"
        );
        assert_eq!(
            translate(Locale::Ru, "slug_empty"),
            "Slug не может быть пустым"
        );
    }

    #[test]
    fn test_translate_auth_keys() {
        assert_eq!(
            translate(Locale::En, "auth.email_already_exists"),
            "A user with this email already exists",
        );
        assert_eq!(
            translate(Locale::Ru, "auth.invalid_credentials"),
            "Неверные учётные данные",
        );
        assert_eq!(
            translate(Locale::De, "auth.session_expired"),
            "Sitzung abgelaufen",
        );
        assert_eq!(
            translate(Locale::Zh, "oauth.pkce_invalid"),
            "PKCE代码验证器无效",
        );
    }

    #[test]
    fn test_translate_fallback_to_english() {
        // Russian missing key falls back to English
        assert_eq!(translate(Locale::Ru, "unknown_key"), "unknown_key");
    }

    #[test]
    fn test_translate_unknown_key_returns_key() {
        assert_eq!(translate(Locale::En, "no.such.key"), "no.such.key");
    }

    #[test]
    fn test_extract_locale_from_header() {
        assert_eq!(
            extract_locale_from_header(Some("en-US,en;q=0.9")),
            Locale::En
        );
        assert_eq!(
            extract_locale_from_header(Some("ru-RU,ru;q=0.9")),
            Locale::Ru
        );
        assert_eq!(extract_locale_from_header(Some("es")), Locale::Es);
        assert_eq!(extract_locale_from_header(None), Locale::En);
    }
}
