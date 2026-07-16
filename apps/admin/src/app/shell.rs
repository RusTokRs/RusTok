use crate::app::App;
use leptos::prelude::*;

/// Classic server-rendered admin document shell.
///
/// The default SSR profile intentionally omits `HydrationScripts`: interactivity that requires the
/// browser is progressively enhanced by small standalone JavaScript adapters such as
/// `fly-browser`, while routing and page HTML remain server-rendered Rust.
pub fn shell(_options: LeptosOptions) -> impl IntoView {
    view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8"/>
                <meta name="viewport" content="width=device-width, initial-scale=1"/>
                <meta name="color-scheme" content="light dark"/>
                <title>"RusTok Admin"</title>
                <link rel="stylesheet" href="/pkg/rustok-admin.css"/>
            </head>
            <body>
                <App/>
            </body>
        </html>
    }
}
