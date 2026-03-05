use crate::shared::ui::ui_button;
use leptos::prelude::*;

#[component]
pub fn Header(
    nav_home: &'static str,
    nav_catalog: &'static str,
    nav_about: &'static str,
    nav_contact: &'static str,
    nav_language: &'static str,
    cta_primary: &'static str,
) -> impl IntoView {
    view! {
        <header class="sticky top-0 z-40 border-b border-border bg-background/95 backdrop-blur">
            <div class="container-app flex h-14 w-full items-center px-4">
                <div class="flex-1">
                    <a class="text-xl font-bold text-foreground hover:text-primary transition-colors" href="/">
                        "RusToK"
                    </a>
                </div>
                <nav class="hidden lg:flex items-center gap-6">
                    <a class="text-sm text-muted-foreground hover:text-foreground transition-colors" href="#home">{nav_home}</a>
                    <a class="text-sm text-muted-foreground hover:text-foreground transition-colors" href="#catalog">{nav_catalog}</a>
                    <a class="text-sm text-muted-foreground hover:text-foreground transition-colors" href="#about">{nav_about}</a>
                    <a class="text-sm text-muted-foreground hover:text-foreground transition-colors" href="#contact">{nav_contact}</a>
                </nav>
                <div class="flex items-center gap-3 ml-6">
                    <div class="relative">
                        <details class="group">
                            <summary class="inline-flex items-center gap-1 rounded-md border border-input bg-background px-3 py-1.5 text-sm text-foreground cursor-pointer hover:bg-accent hover:text-accent-foreground transition-colors list-none">
                                {nav_language}
                            </summary>
                            <ul class="absolute right-0 mt-1 w-32 rounded-md border border-border bg-popover p-1 shadow-md z-50">
                                <li>
                                    <a class="block rounded px-3 py-1.5 text-sm text-popover-foreground hover:bg-accent hover:text-accent-foreground transition-colors" href="/?lang=en">
                                        "English"
                                    </a>
                                </li>
                                <li>
                                    <a class="block rounded px-3 py-1.5 text-sm text-popover-foreground hover:bg-accent hover:text-accent-foreground transition-colors" href="/?lang=ru">
                                        "Русский"
                                    </a>
                                </li>
                            </ul>
                        </details>
                    </div>
                    <ui_button class="px-4 py-1.5 text-sm" attr:href="#catalog">
                        {cta_primary}
                    </ui_button>
                </div>
            </div>
        </header>
    }
}
