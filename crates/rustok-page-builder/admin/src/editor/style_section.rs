use crate::editor::AdminEditorRuntime;
use crate::editor::property_helpers::{selected_patch, selected_style_value};
use crate::i18n::t;
use fly::ComponentPatch;
use fly_ui::{StyleEntry, builtin_style_properties, style_patch};
use leptos::prelude::*;
use rustok_ui_core::UiRouteContext;

#[component]
pub(crate) fn StyleSection(runtime: AdminEditorRuntime) -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let locale = route_context.locale;
    let styles_label = t(locale.as_deref(), "page_builder.panel.styles", "Styles");
    let apply_label = t(locale.as_deref(), "page_builder.action.apply", "Apply");
    let clear_label = t(locale.as_deref(), "page_builder.action.clear", "Clear");
    let property_label = t(
        locale.as_deref(),
        "page_builder.field.styleProperty",
        "Style property",
    );
    let value_label = t(
        locale.as_deref(),
        "page_builder.field.styleValue",
        "Style value",
    );
    let style_property = RwSignal::new("padding".to_string());
    let style_value = RwSignal::new(String::new());
    let observed_selection = RwSignal::new(None::<String>);

    Effect::new({
        let runtime = runtime.clone();
        move |_| {
            let selected_id = runtime
                .controller
                .with(|controller| controller.ui().state.selection.component_id.clone());
            if observed_selection.get_untracked() == selected_id {
                return;
            }
            observed_selection.set(selected_id);
            style_value.set(selected_style_value(
                &runtime,
                &style_property.get_untracked(),
            ));
        }
    });

    let change_runtime = runtime.clone();
    let apply_runtime = runtime.clone();
    let clear_runtime = runtime;
    let descriptors = builtin_style_properties();

    view! {
        <section class="space-y-2 border-t border-border pt-3">
            <h2 class="font-semibold">{styles_label}</h2>
            <label class="block text-sm font-medium">{property_label}</label>
            <select
                class="w-full rounded border border-input bg-background px-2 py-1 text-sm"
                prop:value=move || style_property.get()
                on:change=move |event| {
                    let property = event_target_value(&event);
                    style_property.set(property.clone());
                    style_value.set(selected_style_value(&change_runtime, &property));
                }
            >
                {descriptors.into_iter().map(|descriptor| view! {
                    <option value=descriptor.property>{format!("{:?} · {}", descriptor.group, descriptor.label)}</option>
                }).collect_view()}
            </select>
            <label class="block text-sm font-medium">{value_label}</label>
            <input
                class="w-full rounded border border-input bg-background px-2 py-1 text-sm"
                prop:value=move || style_value.get()
                on:input=move |event| style_value.set(event_target_value(&event))
            />
            <div class="flex gap-2">
                <button
                    type="button"
                    class="rounded border border-border px-2 py-1 text-xs"
                    on:click=move |_| match style_patch([StyleEntry {
                        property: style_property.get_untracked(),
                        value: style_value.get_untracked(),
                    }]) {
                        Ok(patch) => apply_runtime.dispatch_result(selected_patch(&apply_runtime, patch)),
                        Err(errors) => apply_runtime.fail(
                            errors
                                .into_iter()
                                .map(|error| format!("{}: {}", error.property, error.message))
                                .collect::<Vec<_>>()
                                .join("; "),
                        ),
                    }
                >{apply_label}</button>
                <button
                    type="button"
                    class="rounded border border-border px-2 py-1 text-xs"
                    on:click=move |_| clear_runtime.dispatch_result(selected_patch(
                        &clear_runtime,
                        ComponentPatch {
                            remove_style_properties: vec![style_property.get_untracked()],
                            ..ComponentPatch::default()
                        },
                    ))
                >{clear_label}</button>
            </div>
        </section>
    }
}
