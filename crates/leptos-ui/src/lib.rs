pub use iu_leptos::badge::Badge;
pub use iu_leptos::button::Button;
pub use iu_leptos::checkbox::Checkbox;
pub use iu_leptos::input::Input;
pub use iu_leptos::select::{Select, SelectOption};
pub use iu_leptos::spinner::Spinner;
pub use iu_leptos::switch::{Switch, SwitchSize};
pub use iu_leptos::textarea::Textarea;
pub use iu_leptos::types::{BadgeVariant, ButtonVariant, Size};

pub mod card;
pub mod label;
pub mod language_toggle;
pub mod separator;
pub mod success_message;

pub use card::{Card, CardContent, CardFooter, CardHeader};
pub use label::Label;
pub use language_toggle::LanguageToggle as ui_language_toggle;
pub use separator::Separator;
pub use success_message::SuccessMessage as ui_success_message;

// Re-exports with ui_ prefix for consistency across apps
pub use iu_leptos::badge::Badge as ui_badge;
pub use iu_leptos::button::Button as ui_button;
pub use iu_leptos::checkbox::Checkbox as ui_checkbox;
pub use iu_leptos::input::Input as ui_input;
pub use iu_leptos::switch::Switch as ui_switch;
pub use iu_leptos::textarea::Textarea as ui_textarea;
