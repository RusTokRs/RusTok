use leptos::prelude::*;
use web_sys::PointerEvent;

use crate::core::calculate_resized_width;

#[component]
pub fn ColumnResizeHandle(
    column_id: String,
    current_width: u32,
    min_width: u32,
    max_width: u32,
    on_resize: Callback<(String, u32)>,
) -> impl IntoView {
    let (is_dragging, set_is_dragging) = signal(false);
    let (start_x, set_start_x) = signal(0.0);
    let (initial_width, set_initial_width) = signal(current_width);

    let id_for_move = column_id.clone();
    let on_pointer_down = move |ev: PointerEvent| {
        ev.stop_propagation();
        if let Some(target) = ev.current_target() {
            if let Ok(element) = target.dyn_into::<web_sys::Element>() {
                let _ = element.set_pointer_capture(ev.pointer_id());
            }
        }
        set_start_x.set(ev.client_x() as f64);
        set_initial_width.set(current_width);
        set_is_dragging.set(true);
    };

    let on_pointer_move = move |ev: PointerEvent| {
        if is_dragging.get() {
            let delta = ev.client_x() as f64 - start_x.get();
            let new_width = calculate_resized_width(
                initial_width.get(),
                delta,
                min_width,
                max_width,
            );
            on_resize.run((id_for_move.clone(), new_width));
        }
    };

    let on_pointer_up = move |ev: PointerEvent| {
        if is_dragging.get() {
            if let Some(target) = ev.current_target() {
                if let Ok(element) = target.dyn_into::<web_sys::Element>() {
                    let _ = element.release_pointer_capture(ev.pointer_id());
                }
            }
            set_is_dragging.set(false);
        }
    };

    view! {
        <div
            class=move || {
                let base = "absolute right-0 top-0 bottom-0 w-2 cursor-col-resize select-none touch-none z-10 transition-colors flex justify-center items-center group";
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
        >
            <div class=move || {
                let active = is_dragging.get();
                if active {
                    "w-0.5 h-full bg-primary"
                } else {
                    "w-0.5 h-3 bg-border group-hover:bg-primary group-hover:h-full transition-all"
                }
            }/>
        </div>
    }
}
