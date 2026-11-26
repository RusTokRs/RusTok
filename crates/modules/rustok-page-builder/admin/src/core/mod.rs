pub mod session;

pub use session::{
    EditorSessionAction, EditorSessionEffect, EditorSessionState, gate_error_message,
};

/// One resize policy for the Leptos and Dioxus Page Builder authoring surfaces.
#[cfg(any(target_arch = "wasm32", feature = "dioxus-web", test))]
pub(crate) fn page_builder_resize_policy() -> fly_ui::ResizePolicy {
    fly_ui::ResizePolicy {
        grid_size: Some(4.0),
        maximum_width: Some(fly_ui::MAX_RESIZE_DIMENSION_PX),
        maximum_height: Some(fly_ui::MAX_RESIZE_DIMENSION_PX),
        ..fly_ui::ResizePolicy::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fly_ui::{CanvasRect, MAX_RESIZE_DIMENSION_PX, ResizeHandle, ResizeSession};

    #[test]
    fn page_builder_resize_caps_pointer_and_keyboard_dimensions() {
        let resize = ResizeSession {
            component_id: "hero".to_string(),
            handle: ResizeHandle::SouthEast,
            start_rect: CanvasRect {
                x: 0.0,
                y: 0.0,
                width: MAX_RESIZE_DIMENSION_PX - 4.0,
                height: MAX_RESIZE_DIMENSION_PX - 4.0,
            },
            start_x: 0.0,
            start_y: 0.0,
            policy: page_builder_resize_policy(),
        };
        let pointer = resize.update(100.0, 100.0);
        assert_eq!(pointer.rect.width, MAX_RESIZE_DIMENSION_PX);
        assert_eq!(pointer.rect.height, MAX_RESIZE_DIMENSION_PX);
        let keyboard = resize
            .update_by_arrow_key("ArrowRight", true)
            .expect("arrow");
        assert_eq!(keyboard.rect.width, MAX_RESIZE_DIMENSION_PX);
        assert_eq!(keyboard.rect.height, resize.start_rect.height);
    }
}
