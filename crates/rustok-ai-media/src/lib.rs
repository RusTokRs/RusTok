#![cfg_attr(not(feature = "server"), allow(dead_code))]

pub const IMAGE_ASSET_TASK_SLUG: &str = "image_asset";
pub const IMAGE_ASSET_TOOL_NAME: &str = "direct.media.generate_image";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MediaAiVerticalDescriptor {
    pub task_slug: &'static str,
    pub tool_name: &'static str,
    pub sensitive: bool,
}

pub const MEDIA_AI_VERTICALS: &[MediaAiVerticalDescriptor] = &[MediaAiVerticalDescriptor {
    task_slug: IMAGE_ASSET_TASK_SLUG,
    tool_name: IMAGE_ASSET_TOOL_NAME,
    sensitive: false,
}];

pub fn media_ai_verticals() -> &'static [MediaAiVerticalDescriptor] {
    MEDIA_AI_VERTICALS
}

pub fn register_media_ai_verticals() -> &'static [MediaAiVerticalDescriptor] {
    media_ai_verticals()
}

pub fn register_media_ai_vertical_handlers(
    mut register: impl FnMut(&'static MediaAiVerticalDescriptor),
) {
    for vertical in media_ai_verticals() {
        register(vertical);
    }
}

pub fn normalize_image_size(size: Option<String>) -> Result<Option<String>, String> {
    let Some(size) = size
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
    else {
        return Ok(None);
    };
    let (width, height) = size
        .split_once('x')
        .ok_or_else(|| "image size must be formatted as WIDTHxHEIGHT".to_string())?;
    let width = width
        .trim()
        .parse::<u32>()
        .map_err(|_| "image width must be numeric".to_string())?;
    let height = height
        .trim()
        .parse::<u32>()
        .map_err(|_| "image height must be numeric".to_string())?;
    if width == 0 || height == 0 || width > 4096 || height > 4096 {
        return Err("image size must stay within 1..=4096 for both dimensions".to_string());
    }
    Ok(Some(format!("{width}x{height}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_image_size() {
        assert_eq!(normalize_image_size(None).unwrap(), None);
        assert_eq!(
            normalize_image_size(Some("1024x1024".to_string())).unwrap(),
            Some("1024x1024".to_string())
        );
        assert!(normalize_image_size(Some("0x1024".to_string())).is_err());
        assert!(normalize_image_size(Some("5000x1024".to_string())).is_err());
    }
}
