use leptos::ev::KeyboardEvent;
use leptos::prelude::*;
use web_sys::PointerEvent;
use web_sys::wasm_bindgen::JsCast;

use rustok_grid::{KEYBOARD_RESIZE_STEP, calculate_resized_width, step_resized_width};

/// Drag handle on a column edge.
///
/// Implements the WAI-ARIA *window splitter* pattern: it is focusable, exposes
/// `role="separator"` with `aria-valuenow`, and can be resized with the arrow
/// keys (Home/End jump to the bounds) — not only with a pointer.
#[component]
pub fn ColumnResizeHandle(
    column_id: String,
    /// Column title, used to build an accessible name for the handle.
    #[prop(optional)]
    column_title: Option<String>,
    current_width: Signal<u32>,
    min_width: u32,
    max_width: u32,
    on_resize: Callback<(String, u32)>,
) -> impl IntoView {
    let column_id = StoredValue::new(column_id);
    let (is_dragging, set_is_dragging) = signal(false);
    let (start_x, set_start_x) = signal(0.0_f64);
    let (initial_width, set_initial_width) = signal(current_width.get_untracked());

    let label = match column_title {
        Some(title) if !title.trim().is_empty() => format!("Resize column {title}"),
        _ => "Resize column".to_string(),
    };

    let apply = move |width: u32| {
        on_resize.run((column_id.get_value(), width));
    };

    let on_pointer_down = move |ev: PointerEvent| {
        // Only the primary button starts a drag; keep text selection and
        // native drag behaviour out of the way.
        if ev.button() != 0 {
            return;
        }
        ev.stop_propagation();
        ev.prevent_default();
        if let Some(target) = ev.current_target()
            && let Ok(element) = target.dyn_into::<web_sys::Element>()
        {
            // Pointer capture keeps move/up events coming to this element
            // even when the cursor leaves it mid-drag.
            let _ = element.set_pointer_capture(ev.pointer_id());
        }
        set_start_x.set(ev.client_x() as f64);
        set_initial_width.set(current_width.get_untracked());
        set_is_dragging.set(true);
    };

    let on_pointer_move = move |ev: PointerEvent| {
        if !is_dragging.get_untracked() {
            return;
        }
        ev.prevent_default();
        let delta = ev.client_x() as f64 - start_x.get_untracked();
        apply(calculate_resized_width(
            initial_width.get_untracked(),
            delta,
            min_width,
            max_width,
        ));
    };

    let on_pointer_up = move |ev: PointerEvent| {
        if !is_dragging.get_untracked() {
            return;
        }
        if let Some(target) = ev.current_target()
            && let Ok(element) = target.dyn_into::<web_sys::Element>()
        {
            let _ = element.release_pointer_capture(ev.pointer_id());
        }
        set_is_dragging.set(false);
    };

    let on_key_down = move |ev: KeyboardEvent| {
        let width = current_width.get_untracked();
        let next = match ev.key().as_str() {
            "ArrowLeft" => step_resized_width(width, -KEYBOARD_RESIZE_STEP, min_width, max_width),
            "ArrowRight" => step_resized_width(width, KEYBOARD_RESIZE_STEP, min_width, max_width),
            "Home" => min_width.min(max_width),
            "End" => min_width.max(max_width),
            _ => return,
        };
        ev.prevent_default();
        ev.stop_propagation();
        if next != width {
            apply(next);
        }
    };

    view! {
        <div
            role="separator"
            aria-orientation="vertical"
            tabindex="0"
            aria-label=label
            aria-valuemin=min_width.min(max_width)
            aria-valuemax=min_width.max(max_width)
            aria-valuenow=move || current_width.get()
            class=move || {
                let base = "absolute right-0 top-0 bottom-0 w-2 cursor-col-resize select-none touch-none z-10 transition-colors flex justify-center items-center group focus:outline-none focus-visible:ring-1 focus-visible:ring-primary";
                if is_dragging.get() {
                    format!("{base} bg-primary/40")
                } else {
                    format!("{base} hover:bg-primary/20")
                }
            }
            on:pointerdown=on_pointer_down
            on:pointermove=on_pointer_move
            on:pointerup=on_pointer_up
            on:pointercancel=on_pointer_up
            on:keydown=on_key_down
            on:click=move |ev| ev.stop_propagation()
        >
            <div class=move || {
                if is_dragging.get() {
                    "w-0.5 h-full bg-primary"
                } else {
                    "w-0.5 h-3 bg-border group-hover:bg-primary group-hover:h-full transition-all"
                }
            } />
        </div>
    }
}
