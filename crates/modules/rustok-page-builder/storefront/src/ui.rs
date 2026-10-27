use leptos::prelude::*;
use rustok_ui_core::UiRouteContext;

use crate::core::page_builder_view_model;
use crate::PageBuilderStorefront;

/// Standard storefront entry point for Page Builder routes and slots.
#[component]
pub fn PageBuilderView(
    #[prop(optional)] project_data: Option<serde_json::Value>,
    #[prop(optional)] class: Option<String>,
) -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let locale = route_context.locale.clone();
    let class = class.unwrap_or_else(|| "rustok-page-builder-storefront".to_string());

    if let Some(data) = project_data {
        return view! {
            <PageBuilderStorefront
                project_data=data
                class=class
            />
        }
        .into_any();
    }

    let vm = page_builder_view_model(locale.as_deref(), None, false);

    view! {
        <section
            class=class
            data-rustok-page-builder-storefront="true"
            data-page-builder-view="ready"
        >
            <div class="rounded-2xl border border-dashed border-border p-6 text-center">
                <h3 class="text-lg font-semibold text-card-foreground">
                    {vm.title}
                </h3>
                {(!vm.description.is_empty()).then(|| view! {
                    <p class="mt-2 text-sm text-muted-foreground">
                        {vm.description}
                    </p>
                })}
            </div>
        </section>
    }
    .into_any()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_builder_view_component_constructs_view() {
        let _view = view! {
            <PageBuilderView />
        };
    }
}
