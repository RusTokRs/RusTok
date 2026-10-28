//! Drag handle rendered on the trailing edge of a resizable column.

use dioxus::html::input_data::MouseButton;
use dioxus::html::point_interaction::{InteractionLocation, PointerInteraction};
use dioxus::prelude::*;

use rustok_grid::{KEYBOARD_RESIZE_STEP, calculate_resized_width, step_resized_width};

/// Pointer position and column width captured when a drag starts.
#[derive(Clone, Copy, Debug, PartialEq)]
struct DragOrigin {
    start_x: f64,
    start_width: u32,
}

/// Drag handle on a column edge.
///
/// Implements the WAI-ARIA *window splitter* pattern: it is focusable, exposes
/// `role="separator"` with `aria-valuenow`, and can be resized with the arrow
/// keys (Home/End jump to the bounds) — not only with a pointer.
///
/// While a drag is in progress a transparent, full-viewport overlay is
/// rendered. It keeps receiving `pointermove`/`pointerup` even when the cursor
/// runs far ahead of the 8px handle, which is what pointer capture would do —
/// except that it needs no `web-sys` and therefore also works in the desktop
/// and native renderers.
#[component]
pub fn ColumnResizeHandle(
    /// Identifier of the column being resized.
    column_id: String,
    /// Column title, used to build an accessible name for the handle.
    #[props(default)]
    column_title: Option<String>,
    /// Width the column is rendered with right now.
    current_width: u32,
    /// Lower bound of the column width.
    min_width: u32,
    /// Upper bound of the column width.
    max_width: u32,
    /// Emitted with `(column_id, new_width)` on every step of the resize.
    on_resize: EventHandler<(String, u32)>,
) -> Element {
    let mut drag = use_signal(|| None::<DragOrigin>);
    // Read once, up front: no read guard is alive while the tree is built.
    let drag_origin: Option<DragOrigin> = *drag.read();
    let is_dragging = drag_origin.is_some();

    let label = match column_title {
        Some(ref title) if !title.trim().is_empty() => format!("Resize column {title}"),
        _ => "Resize column".to_string(),
    };

    // The core never trusts the order of the bounds, and neither do we.
    let low = min_width.min(max_width);
    let high = min_width.max(max_width);

    let handle_class = if is_dragging {
        "absolute right-0 top-0 bottom-0 w-2 cursor-col-resize select-none touch-none z-10 transition-colors flex justify-center items-center group focus:outline-none focus-visible:ring-1 focus-visible:ring-primary bg-primary/40"
    } else {
        "absolute right-0 top-0 bottom-0 w-2 cursor-col-resize select-none touch-none z-10 transition-colors flex justify-center items-center group focus:outline-none focus-visible:ring-1 focus-visible:ring-primary hover:bg-primary/20"
    };
    let grip_class = if is_dragging {
        "w-0.5 h-full bg-primary"
    } else {
        "w-0.5 h-3 bg-border group-hover:bg-primary group-hover:h-full transition-all"
    };

    let id_for_move = column_id.clone();
    let id_for_keys = column_id;

    rsx! {
        div {
            role: "separator",
            "aria-orientation": "vertical",
            tabindex: "0",
            "aria-label": "{label}",
            "aria-valuemin": "{low}",
            "aria-valuemax": "{high}",
            "aria-valuenow": "{current_width}",
            class: handle_class,
            onpointerdown: move |ev| {
                // Only the primary button starts a drag; keep text selection
                // and native drag behaviour out of the way.
                if ev.trigger_button() != Some(MouseButton::Primary) {
                    return;
                }
                ev.stop_propagation();
                ev.prevent_default();
                drag.set(Some(DragOrigin {
                    start_x: ev.client_coordinates().x,
                    start_width: current_width,
                }));
            },
            onkeydown: move |ev| {
                // `Key` renders as the DOM key value ("ArrowLeft", "Home", …),
                // which keeps this free of renderer-specific key codes.
                let next = match ev.key().to_string().as_str() {
                    "ArrowLeft" => {
                        step_resized_width(current_width, -KEYBOARD_RESIZE_STEP, min_width, max_width)
                    }
                    "ArrowRight" => {
                        step_resized_width(current_width, KEYBOARD_RESIZE_STEP, min_width, max_width)
                    }
                    "Home" => low,
                    "End" => high,
                    _ => return,
                };
                ev.prevent_default();
                ev.stop_propagation();
                if next != current_width {
                    on_resize.call((id_for_keys.clone(), next));
                }
            },
            onclick: move |ev| ev.stop_propagation(),

            div { class: grip_class }

            if let Some(origin) = drag_origin {
                // Transparent drag surface: it swallows pointer events for the
                // whole viewport, so the drag survives fast cursor movement and
                // ends reliably even outside the table.
                div {
                    class: "fixed inset-0 z-50 cursor-col-resize select-none touch-none",
                    "aria-hidden": "true",
                    onpointermove: move |ev| {
                        ev.prevent_default();
                        let delta = ev.client_coordinates().x - origin.start_x;
                        let next = calculate_resized_width(
                            origin.start_width,
                            delta,
                            min_width,
                            max_width,
                        );
                        if next != current_width {
                            on_resize.call((id_for_move.clone(), next));
                        }
                    },
                    onpointerup: move |_| drag.set(None),
                    onpointercancel: move |_| drag.set(None),
                    onpointerleave: move |_| drag.set(None),
                }
            }
        }
    }
}
