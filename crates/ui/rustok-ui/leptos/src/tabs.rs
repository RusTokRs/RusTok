/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use leptos::children::Children;
use leptos::prelude::*;
use rustok_ui::{
    Orientation, merge_classes, tabs_content_classes, tabs_list_classes, tabs_trigger_classes,
};

/// Context shared across a `<Tabs>` subtree for declarative selection and orientation.
#[derive(Clone, Copy)]
pub struct TabsContext {
    /// Active tab identifier signal.
    pub value: Signal<String>,
    /// Callback to update the active tab identifier.
    pub set_value: Callback<String>,
    /// Axis of the tab bar.
    pub orientation: Orientation,
}

/// Retrieves the current [`TabsContext`] if rendered inside a `<Tabs>` container.
pub fn use_tabs_context() -> Option<TabsContext> {
    use_context::<TabsContext>()
}

/// Tab container providing state and context to [`TabsList`], [`TabsTrigger`], and [`TabsContent`].
///
/// Supports both controlled mode via `value` + `on_value_change` and uncontrolled mode
/// via `default_value`.
#[component]
pub fn Tabs(
    #[prop(optional, into)] value: Option<Signal<String>>,
    #[prop(optional)] on_value_change: Option<Callback<String>>,
    #[prop(optional, into)] default_value: Option<String>,
    #[prop(default = Orientation::Horizontal)] orientation: Orientation,
    #[prop(optional, into)] class: String,
    children: Children,
) -> impl IntoView {
    let custom = (!class.is_empty()).then_some(class.as_str());
    let full_class = merge_classes(&["w-full", custom.unwrap_or("")]);

    let (internal_value, set_internal_value) = signal(default_value.unwrap_or_default());
    let resolved_value = Signal::derive(move || {
        if let Some(v) = value {
            v.get()
        } else {
            internal_value.get()
        }
    });

    let set_value = Callback::new(move |val: String| {
        set_internal_value.set(val.clone());
        if let Some(cb) = on_value_change {
            cb.run(val);
        }
    });

    provide_context(TabsContext {
        value: resolved_value,
        set_value,
        orientation,
    });

    view! {
        <div class=full_class data-orientation=orientation.as_str()>
            {children()}
        </div>
    }
}

/// Tab list container with `role="tablist"`, `aria-orientation`, and keyboard arrow navigation.
#[component]
pub fn TabsList(
    #[prop(optional)] orientation: Option<Orientation>,
    #[prop(optional, into)] class: String,
    children: Children,
) -> impl IntoView {
    let ctx = use_tabs_context();
    let effective_orientation = orientation.or_else(|| ctx.map(|c| c.orientation)).unwrap_or(Orientation::Horizontal);
    let custom = (!class.is_empty()).then_some(class.as_str());
    let full_class = tabs_list_classes(effective_orientation, custom);

    let on_keydown = move |event: leptos::ev::KeyboardEvent| {
        #[cfg(target_arch = "wasm32")]
        {
            use leptos::wasm_bindgen::JsCast;
            let key = event.key();
            let is_horizontal = effective_orientation == Orientation::Horizontal;
            let is_next = (is_horizontal && key == "ArrowRight") || (!is_horizontal && key == "ArrowDown");
            let is_prev = (is_horizontal && key == "ArrowLeft") || (!is_horizontal && key == "ArrowUp");
            let is_home = key == "Home";
            let is_end = key == "End";

            if is_next || is_prev || is_home || is_end {
                let Some(target) = event.target().and_then(|t| t.dyn_into::<web_sys::Element>().ok()) else { return; };
                let Ok(Some(list)) = target.closest("[role='tablist']") else { return; };
                let Ok(tabs) = list.query_selector_all("[role='tab']:not(:disabled)") else { return; };
                let len = tabs.length() as usize;
                if len == 0 { return; }

                let current_index = (0..len).find(|&i| {
                    tabs.item(i as u32).map_or(false, |node| node.is_same_node(Some(&target)))
                });

                let target_index = if is_home {
                    0
                } else if is_end {
                    len - 1
                } else if is_next {
                    current_index.map(|i| (i + 1) % len).unwrap_or(0)
                } else {
                    current_index.map(|i| if i == 0 { len - 1 } else { i - 1 }).unwrap_or(len - 1)
                };

                if let Some(next_node) = tabs.item(target_index as u32) {
                    if let Ok(next_el) = next_node.dyn_into::<web_sys::HtmlElement>() {
                        event.prevent_default();
                        let _ = next_el.focus();
                        let _ = next_el.click();
                    }
                }
            }
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = event;
        }
    };

    view! {
        <div
            role="tablist"
            aria-orientation=effective_orientation.as_str()
            class=full_class
            on:keydown=on_keydown
        >
            {children()}
        </div>
    }
}

/// Tab trigger button.
///
/// Can be driven declaratively via `value` when rendered inside [`Tabs`], or directly
/// via `active` for low-level usage. Supports WAI-ARIA roving tabindex.
#[component]
pub fn TabsTrigger(
    #[prop(optional, into)] value: Option<String>,
    #[prop(default = false)] active: bool,
    #[prop(default = false)] disabled: bool,
    #[prop(optional)] on_select: Option<Callback<()>>,
    #[prop(optional, into)] class: String,
    #[prop(optional, into)] id: String,
    #[prop(optional, into)] panel_id: String,
    children: Children,
) -> impl IntoView {
    let ctx = use_tabs_context();
    let val_clone = value.clone();
    let is_active = Signal::derive(move || {
        if let (Some(ctx), Some(val)) = (ctx, val_clone.as_ref()) {
            ctx.value.get() == *val
        } else {
            active
        }
    });

    let id = (!id.is_empty()).then_some(id);
    let panel_id = (!panel_id.is_empty()).then_some(panel_id);
    let custom = StoredValue::new(class);

    let full_class = move || {
        let c = custom.get_value();
        tabs_trigger_classes(is_active.get(), disabled, (!c.is_empty()).then_some(c.as_str()))
    };

    let val_for_click = value.clone();
    view! {
        <button
            type="button"
            id=id
            role="tab"
            aria-selected=move || is_active.get().to_string()
            aria-controls=panel_id
            tabindex=move || if is_active.get() { "0" } else { "-1" }
            disabled=disabled
            class=full_class
            on:click=move |_| {
                if !disabled {
                    if let (Some(ctx), Some(val)) = (ctx, val_for_click.as_ref()) {
                        ctx.set_value.run(val.clone());
                    }
                    if let Some(cb) = on_select {
                        cb.run(());
                    }
                }
            }
        >
            {children()}
        </button>
    }
}

/// Tab panel container.
///
/// Renders `role="tabpanel"` and `tabindex="0"`. Activates automatically when `value`
/// matches [`TabsContext`], or based on the direct `active` boolean prop.
#[component]
pub fn TabsContent(
    #[prop(optional, into)] value: Option<String>,
    #[prop(default = false)] active: bool,
    #[prop(optional, into)] class: String,
    #[prop(optional, into)] id: String,
    #[prop(optional, into)] tab_id: String,
    children: Children,
) -> impl IntoView {
    let ctx = use_tabs_context();
    let val_clone = value.clone();
    let is_active = Signal::derive(move || {
        if let (Some(ctx), Some(val)) = (ctx, val_clone.as_ref()) {
            ctx.value.get() == *val
        } else {
            active
        }
    });

    let id = (!id.is_empty()).then_some(id);
    let tab_id = (!tab_id.is_empty()).then_some(tab_id);
    let custom = StoredValue::new(class);

    let full_class = move || {
        let c = custom.get_value();
        tabs_content_classes(is_active.get(), (!c.is_empty()).then_some(c.as_str()))
    };

    view! {
        <div
            id=id
            role="tabpanel"
            aria-labelledby=tab_id
            tabindex="0"
            class=full_class
        >
            {children()}
        </div>
    }
}
