use crate::ViewportState;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum DeviceClass {
    Desktop,
    Laptop,
    Tablet,
    Mobile,
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ViewportPreset {
    pub id: String,
    pub label: String,
    pub device_class: DeviceClass,
    pub width: u32,
    pub height: u32,
    pub default_zoom: f32,
}

impl ViewportPreset {
    pub fn apply(&self, current: ViewportState) -> ViewportState {
        ViewportState {
            width: self.width,
            height: self.height,
            zoom: normalize_zoom(self.default_zoom),
            scroll_x: current.scroll_x.min(f64::from(self.width)),
            scroll_y: current.scroll_y,
        }
    }
}

pub fn builtin_viewport_presets() -> Vec<ViewportPreset> {
    vec![
        ViewportPreset {
            id: "desktop-wide".to_string(),
            label: "Desktop 1440".to_string(),
            device_class: DeviceClass::Desktop,
            width: 1440,
            height: 900,
            default_zoom: 1.0,
        },
        ViewportPreset {
            id: "desktop".to_string(),
            label: "Desktop 1280".to_string(),
            device_class: DeviceClass::Desktop,
            width: 1280,
            height: 800,
            default_zoom: 1.0,
        },
        ViewportPreset {
            id: "laptop".to_string(),
            label: "Laptop 1024".to_string(),
            device_class: DeviceClass::Laptop,
            width: 1024,
            height: 768,
            default_zoom: 0.9,
        },
        ViewportPreset {
            id: "tablet".to_string(),
            label: "Tablet 768".to_string(),
            device_class: DeviceClass::Tablet,
            width: 768,
            height: 1024,
            default_zoom: 0.85,
        },
        ViewportPreset {
            id: "mobile-large".to_string(),
            label: "Mobile 430".to_string(),
            device_class: DeviceClass::Mobile,
            width: 430,
            height: 932,
            default_zoom: 0.8,
        },
        ViewportPreset {
            id: "mobile".to_string(),
            label: "Mobile 390".to_string(),
            device_class: DeviceClass::Mobile,
            width: 390,
            height: 844,
            default_zoom: 0.8,
        },
    ]
}

pub fn viewport_preset(id: &str) -> Option<ViewportPreset> {
    builtin_viewport_presets()
        .into_iter()
        .find(|preset| preset.id == id)
}

pub fn custom_viewport(width: u32, height: u32, zoom: f32) -> ViewportPreset {
    ViewportPreset {
        id: format!("custom-{width}x{height}"),
        label: format!("Custom {width} × {height}"),
        device_class: DeviceClass::Custom,
        width: width.clamp(240, 7680),
        height: height.clamp(240, 4320),
        default_zoom: normalize_zoom(zoom),
    }
}

pub fn normalize_zoom(zoom: f32) -> f32 {
    if zoom.is_finite() {
        zoom.clamp(0.1, 4.0)
    } else {
        1.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_cover_desktop_tablet_and_mobile() {
        let presets = builtin_viewport_presets();
        assert!(presets.iter().any(|preset| preset.device_class == DeviceClass::Desktop));
        assert!(presets.iter().any(|preset| preset.device_class == DeviceClass::Tablet));
        assert!(presets.iter().any(|preset| preset.device_class == DeviceClass::Mobile));
    }

    #[test]
    fn custom_viewport_clamps_unsafe_dimensions_and_zoom() {
        let preset = custom_viewport(10, 100_000, 20.0);
        assert_eq!(preset.width, 240);
        assert_eq!(preset.height, 4320);
        assert_eq!(preset.default_zoom, 4.0);
    }
}
