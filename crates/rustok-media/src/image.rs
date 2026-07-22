use std::{io::Cursor, sync::Arc, time::Duration};

use fast_image_resize::{IntoImageView, Resizer, images::Image};
use image::{
    AnimationDecoder, DynamicImage, ExtendedColorType, GenericImageView, ImageDecoder,
    ImageEncoder, ImageFormat, ImageReader, RgbaImage,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use tokio::sync::Semaphore;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageOutputFormat {
    Jpeg,
    Png,
    Webp,
    Avif,
}

impl ImageOutputFormat {
    pub const fn extension(self) -> &'static str {
        match self {
            Self::Jpeg => "jpg",
            Self::Png => "png",
            Self::Webp => "webp",
            Self::Avif => "avif",
        }
    }

    pub const fn mime_type(self) -> &'static str {
        match self {
            Self::Jpeg => "image/jpeg",
            Self::Png => "image/png",
            Self::Webp => "image/webp",
            Self::Avif => "image/avif",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageBackground {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

impl Default for ImageBackground {
    fn default() -> Self {
        Self {
            red: 255,
            green: 255,
            blue: 255,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CropRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageResize {
    pub width: u32,
    pub height: u32,
    #[serde(default)]
    pub cover: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum QuarterTurn {
    #[default]
    None,
    Clockwise90,
    Clockwise180,
    Clockwise270,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageRecipe {
    #[serde(default)]
    pub crop: Option<CropRect>,
    #[serde(default)]
    pub resize: Option<ImageResize>,
    #[serde(default)]
    pub rotate: QuarterTurn,
    #[serde(default)]
    pub flip_horizontal: bool,
    #[serde(default)]
    pub flip_vertical: bool,
    pub output: ImageOutputFormat,
    pub quality: u8,
    /// Matte used when a format without alpha support is requested.
    #[serde(default)]
    pub background: ImageBackground,
    #[serde(default = "default_strip_metadata")]
    pub strip_metadata: bool,
}

fn default_strip_metadata() -> bool {
    true
}

impl ImageRecipe {
    pub fn validate(&self, limits: ImageProcessingLimits) -> Result<(), ImageProcessingError> {
        if !(1..=100).contains(&self.quality) {
            return Err(ImageProcessingError::InvalidRecipe(
                "quality must be between 1 and 100".to_string(),
            ));
        }
        if !self.strip_metadata {
            return Err(ImageProcessingError::InvalidRecipe(
                "metadata preservation is not enabled by Media policy".to_string(),
            ));
        }
        if let Some(crop) = self.crop
            && (crop.width == 0 || crop.height == 0)
        {
            return Err(ImageProcessingError::InvalidRecipe(
                "crop dimensions must be non-zero".to_string(),
            ));
        }
        if let Some(resize) = self.resize {
            validate_dimensions(resize.width, resize.height, limits)?;
        }
        Ok(())
    }

    /// SHA-256 over the canonical field-order JSON representation.
    pub fn digest(&self) -> Result<String, ImageProcessingError> {
        let bytes = serde_json::to_vec(self)
            .map_err(|error| ImageProcessingError::RecipeEncoding(error.to_string()))?;
        Ok(hex::encode(Sha256::digest(bytes)))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImageProcessingLimits {
    pub max_input_bytes: usize,
    pub max_output_bytes: usize,
    pub max_width: u32,
    pub max_height: u32,
    pub max_pixels: u64,
    pub max_decoded_bytes: u64,
    pub max_frames: usize,
}

impl Default for ImageProcessingLimits {
    fn default() -> Self {
        Self {
            max_input_bytes: crate::dto::DEFAULT_MAX_SIZE as usize,
            max_output_bytes: 32 * 1024 * 1024,
            max_width: 16_384,
            max_height: 16_384,
            max_pixels: 100_000_000,
            max_decoded_bytes: 256 * 1024 * 1024,
            max_frames: 1,
        }
    }
}

pub fn inspect_image(
    input: &[u8],
    limits: ImageProcessingLimits,
) -> Result<(u32, u32), ImageProcessingError> {
    if input.is_empty() || input.len() > limits.max_input_bytes {
        return Err(ImageProcessingError::InputBytesLimit {
            actual: input.len(),
            max: limits.max_input_bytes,
        });
    }
    let format = image::guess_format(input)?;
    let mut reader = ImageReader::new(Cursor::new(input));
    reader.set_format(format);
    let decoder = reader.into_decoder()?;
    let (width, height) = decoder.dimensions();
    validate_dimensions(width, height, limits)?;
    validate_decoded_bytes(decoder.total_bytes(), limits)?;
    Ok((width, height))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageOutput {
    pub bytes: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub mime_type: &'static str,
    pub extension: &'static str,
    pub recipe_digest: String,
}

pub fn process_image(
    input: &[u8],
    recipe: &ImageRecipe,
    limits: ImageProcessingLimits,
) -> Result<ImageOutput, ImageProcessingError> {
    recipe.validate(limits)?;
    if input.is_empty() || input.len() > limits.max_input_bytes {
        return Err(ImageProcessingError::InputBytesLimit {
            actual: input.len(),
            max: limits.max_input_bytes,
        });
    }

    if limits.max_frames != 1 {
        return Err(ImageProcessingError::InvalidLimits(
            "Media rendition processing supports exactly one frame".to_string(),
        ));
    }

    let format = image::guess_format(input)?;
    ensure_single_frame(input, format, limits)?;
    let mut reader = ImageReader::new(Cursor::new(input));
    reader.set_format(format);
    let mut decoder = reader.into_decoder()?;
    let (source_width, source_height) = decoder.dimensions();
    validate_dimensions(source_width, source_height, limits)?;
    validate_decoded_bytes(decoder.total_bytes(), limits)?;
    let orientation = decoder.orientation()?;
    let mut image = DynamicImage::from_decoder(decoder)?;
    image.apply_orientation(orientation);

    if let Some(crop) = recipe.crop {
        let right = crop.x.checked_add(crop.width);
        let bottom = crop.y.checked_add(crop.height);
        if right.is_none_or(|right| right > image.width())
            || bottom.is_none_or(|bottom| bottom > image.height())
        {
            return Err(ImageProcessingError::InvalidRecipe(
                "crop rectangle is outside the decoded image".to_string(),
            ));
        }
        image = image.crop_imm(crop.x, crop.y, crop.width, crop.height);
    }

    image = match recipe.rotate {
        QuarterTurn::None => image,
        QuarterTurn::Clockwise90 => image.rotate90(),
        QuarterTurn::Clockwise180 => image.rotate180(),
        QuarterTurn::Clockwise270 => image.rotate270(),
    };
    if recipe.flip_horizontal {
        image = image.fliph();
    }
    if recipe.flip_vertical {
        image = image.flipv();
    }
    if let Some(resize) = recipe.resize {
        image = resize_rgba(image, resize)?;
    }

    let (width, height) = image.dimensions();
    validate_dimensions(width, height, limits)?;
    let bytes = encode_image(&image, recipe)?;
    if bytes.len() > limits.max_output_bytes {
        return Err(ImageProcessingError::OutputBytesLimit {
            actual: bytes.len(),
            max: limits.max_output_bytes,
        });
    }
    Ok(ImageOutput {
        bytes,
        width,
        height,
        mime_type: recipe.output.mime_type(),
        extension: recipe.output.extension(),
        recipe_digest: recipe.digest()?,
    })
}

fn ensure_single_frame(
    input: &[u8],
    format: ImageFormat,
    limits: ImageProcessingLimits,
) -> Result<(), ImageProcessingError> {
    let frames = match format {
        ImageFormat::Gif => {
            let decoder = image::codecs::gif::GifDecoder::new(Cursor::new(input))?;
            let (width, height) = decoder.dimensions();
            validate_dimensions(width, height, limits)?;
            let frame_bytes = u64::from(width)
                .checked_mul(u64::from(height))
                .and_then(|pixels| pixels.checked_mul(4))
                .ok_or(ImageProcessingError::DecodedBytesLimit {
                    actual: u64::MAX,
                    max: limits.max_decoded_bytes,
                })?;
            validate_decoded_bytes(frame_bytes, limits)?;
            let mut frames = decoder.into_frames();
            let first = usize::from(frames.next().transpose()?.is_some());
            let second = usize::from(frames.next().transpose()?.is_some());
            first + second
        }
        ImageFormat::WebP => {
            let decoder = image::codecs::webp::WebPDecoder::new(Cursor::new(input))?;
            usize::from(decoder.has_animation()) + 1
        }
        _ => 1,
    };
    if frames > limits.max_frames {
        return Err(ImageProcessingError::FrameLimit {
            actual_at_least: frames,
            max: limits.max_frames,
        });
    }
    Ok(())
}

fn encode_image(
    image: &DynamicImage,
    recipe: &ImageRecipe,
) -> Result<Vec<u8>, ImageProcessingError> {
    let (width, height) = image.dimensions();
    let rgba = image.to_rgba8();
    match recipe.output {
        ImageOutputFormat::Jpeg => {
            let rgb = flatten_alpha(&rgba, recipe.background);
            mozjpeg_rs::Encoder::progressive_balanced()
                .quality(recipe.quality)
                .encode_rgb(&rgb, width, height)
                .map_err(|error| ImageProcessingError::Encoder(error.to_string()))
        }
        ImageOutputFormat::Png => {
            let mut encoded = Vec::new();
            image::codecs::png::PngEncoder::new(&mut encoded).write_image(
                rgba.as_raw(),
                width,
                height,
                ExtendedColorType::Rgba8,
            )?;
            let preset = match recipe.quality {
                1..=49 => 1,
                50..=84 => 2,
                _ => 3,
            };
            let mut options = oxipng::Options::from_preset(preset);
            options.strip = oxipng::StripChunks::All;
            oxipng::optimize_from_memory(&encoded, &options)
                .map_err(|error| ImageProcessingError::Encoder(error.to_string()))
        }
        ImageOutputFormat::Webp => {
            let config = zenwebp::LossyConfig::new()
                .with_quality(f32::from(recipe.quality))
                .with_method(4)
                .with_alpha_quality(100)
                .with_sharp_yuv(true);
            zenwebp::EncodeRequest::lossy(
                &config,
                rgba.as_raw(),
                zenwebp::PixelLayout::Rgba8,
                width,
                height,
            )
            .encode()
            .map_err(|error| ImageProcessingError::Encoder(error.to_string()))
        }
        ImageOutputFormat::Avif => {
            let mut encoded = Vec::new();
            image::codecs::avif::AvifEncoder::new_with_speed_quality(
                &mut encoded,
                6,
                recipe.quality,
            )
            .with_num_threads(Some(1))
            .write_image(rgba.as_raw(), width, height, ExtendedColorType::Rgba8)?;
            Ok(encoded)
        }
    }
}

fn flatten_alpha(image: &RgbaImage, background: ImageBackground) -> Vec<u8> {
    let mut rgb = Vec::with_capacity(image.width() as usize * image.height() as usize * 3);
    for pixel in image.pixels() {
        let alpha = u16::from(pixel[3]);
        for (foreground, background) in
            pixel.0[..3]
                .iter()
                .zip([background.red, background.green, background.blue])
        {
            let blended =
                (u16::from(*foreground) * alpha + u16::from(background) * (255 - alpha) + 127)
                    / 255;
            rgb.push(blended as u8);
        }
    }
    rgb
}

#[derive(Debug, Clone)]
pub struct ImageWorker {
    permits: Arc<Semaphore>,
    timeout: Duration,
    limits: ImageProcessingLimits,
}

impl ImageWorker {
    pub fn production() -> Self {
        let parallelism = std::thread::available_parallelism()
            .map(usize::from)
            .unwrap_or(2);
        Self::new(
            parallelism.div_ceil(2).clamp(1, 2),
            Duration::from_secs(60),
            ImageProcessingLimits::default(),
        )
        .expect("production image worker limits are valid")
    }

    pub fn new(
        max_concurrency: usize,
        timeout: Duration,
        limits: ImageProcessingLimits,
    ) -> Result<Self, ImageProcessingError> {
        if max_concurrency == 0 {
            return Err(ImageProcessingError::InvalidLimits(
                "image worker concurrency must be greater than zero".to_string(),
            ));
        }
        if timeout.is_zero() {
            return Err(ImageProcessingError::InvalidLimits(
                "image worker timeout must be greater than zero".to_string(),
            ));
        }
        Ok(Self {
            permits: Arc::new(Semaphore::new(max_concurrency)),
            timeout,
            limits,
        })
    }

    pub async fn process(
        &self,
        input: Vec<u8>,
        recipe: ImageRecipe,
    ) -> Result<ImageOutput, ImageProcessingError> {
        let deadline = tokio::time::Instant::now() + self.timeout;
        let permit = tokio::time::timeout_at(deadline, self.permits.clone().acquire_owned())
            .await
            .map_err(|_| ImageProcessingError::Timeout)?
            .map_err(|_| ImageProcessingError::WorkerClosed)?;
        let limits = self.limits;
        let task = tokio::task::spawn_blocking(move || {
            let _permit = permit;
            process_image(&input, &recipe, limits)
        });
        tokio::time::timeout_at(deadline, task)
            .await
            .map_err(|_| ImageProcessingError::Timeout)?
            .map_err(|error| ImageProcessingError::WorkerJoin(error.to_string()))?
    }
}

fn resize_rgba(
    image: DynamicImage,
    resize: ImageResize,
) -> Result<DynamicImage, ImageProcessingError> {
    let source = DynamicImage::ImageRgba8(image.to_rgba8());
    let (target_width, target_height) = if resize.cover {
        (resize.width, resize.height)
    } else {
        contained_dimensions(source.width(), source.height(), resize.width, resize.height)
    };
    let pixel_type = source
        .pixel_type()
        .ok_or(ImageProcessingError::UnsupportedPixelType)?;
    let mut destination = Image::new(target_width, target_height, pixel_type);
    let mut resizer = Resizer::new();
    let options = resize
        .cover
        .then(|| fast_image_resize::ResizeOptions::new().fit_into_destination(Some((0.5, 0.5))));
    resizer.resize(&source, &mut destination, options.as_ref())?;
    let buffer = RgbaImage::from_raw(target_width, target_height, destination.into_vec())
        .ok_or(ImageProcessingError::UnsupportedPixelType)?;
    Ok(DynamicImage::ImageRgba8(buffer))
}

fn contained_dimensions(
    source_width: u32,
    source_height: u32,
    max_width: u32,
    max_height: u32,
) -> (u32, u32) {
    let width_limited = u64::from(max_width) * u64::from(source_height)
        <= u64::from(max_height) * u64::from(source_width);
    if width_limited {
        let height = (u64::from(source_height) * u64::from(max_width) / u64::from(source_width))
            .max(1) as u32;
        (max_width, height)
    } else {
        let width = (u64::from(source_width) * u64::from(max_height) / u64::from(source_height))
            .max(1) as u32;
        (width, max_height)
    }
}

fn validate_dimensions(
    width: u32,
    height: u32,
    limits: ImageProcessingLimits,
) -> Result<(), ImageProcessingError> {
    let pixels = u64::from(width) * u64::from(height);
    if width == 0
        || height == 0
        || width > limits.max_width
        || height > limits.max_height
        || pixels > limits.max_pixels
    {
        return Err(ImageProcessingError::DimensionLimit {
            width,
            height,
            max_width: limits.max_width,
            max_height: limits.max_height,
            max_pixels: limits.max_pixels,
        });
    }
    Ok(())
}

fn validate_decoded_bytes(
    decoded_bytes: u64,
    limits: ImageProcessingLimits,
) -> Result<(), ImageProcessingError> {
    if decoded_bytes > limits.max_decoded_bytes {
        return Err(ImageProcessingError::DecodedBytesLimit {
            actual: decoded_bytes,
            max: limits.max_decoded_bytes,
        });
    }
    Ok(())
}

#[derive(Debug, Error)]
pub enum ImageProcessingError {
    #[error("invalid image recipe: {0}")]
    InvalidRecipe(String),
    #[error("image input is {actual} bytes; maximum is {max}")]
    InputBytesLimit { actual: usize, max: usize },
    #[error("image output is {actual} bytes; maximum is {max}")]
    OutputBytesLimit { actual: usize, max: usize },
    #[error("decoded image requires {actual} bytes; maximum is {max}")]
    DecodedBytesLimit { actual: u64, max: u64 },
    #[error("image has at least {actual_at_least} frames; maximum is {max}")]
    FrameLimit { actual_at_least: usize, max: usize },
    #[error(
        "image dimensions {width}x{height} exceed limits {max_width}x{max_height} or {max_pixels} pixels"
    )]
    DimensionLimit {
        width: u32,
        height: u32,
        max_width: u32,
        max_height: u32,
        max_pixels: u64,
    },
    #[error("unsupported decoded pixel type")]
    UnsupportedPixelType,
    #[error("image codec error: {0}")]
    Codec(#[from] image::ImageError),
    #[error("image resize error: {0}")]
    Resize(#[from] fast_image_resize::ResizeError),
    #[error("image encoder error: {0}")]
    Encoder(String),
    #[error("recipe encoding error: {0}")]
    RecipeEncoding(String),
    #[error("invalid image worker limits: {0}")]
    InvalidLimits(String),
    #[error("image worker timed out")]
    Timeout,
    #[error("image worker is closed")]
    WorkerClosed,
    #[error("image worker task failed: {0}")]
    WorkerJoin(String),
}

#[cfg(test)]
mod tests {
    use image::{ImageBuffer, Rgba};

    use super::*;

    fn png(width: u32, height: u32) -> Vec<u8> {
        let source = DynamicImage::ImageRgba8(ImageBuffer::from_pixel(
            width,
            height,
            Rgba([10, 20, 30, 255]),
        ));
        let mut bytes = Cursor::new(Vec::new());
        source.write_to(&mut bytes, ImageFormat::Png).unwrap();
        bytes.into_inner()
    }

    fn recipe() -> ImageRecipe {
        ImageRecipe {
            crop: None,
            resize: Some(ImageResize {
                width: 200,
                height: 200,
                cover: false,
            }),
            rotate: QuarterTurn::None,
            flip_horizontal: false,
            flip_vertical: false,
            output: ImageOutputFormat::Png,
            quality: 85,
            background: ImageBackground::default(),
            strip_metadata: true,
        }
    }

    #[test]
    fn recipe_digest_is_deterministic() {
        let recipe = recipe();
        assert_eq!(recipe.digest().unwrap(), recipe.digest().unwrap());
    }

    #[test]
    fn resize_preserves_aspect_ratio_without_updating_original() {
        let input = png(400, 200);
        let output = process_image(&input, &recipe(), ImageProcessingLimits::default()).unwrap();
        assert_eq!((output.width, output.height), (200, 100));
        assert_eq!(
            image::load_from_memory(&input).unwrap().dimensions(),
            (400, 200)
        );
    }

    #[test]
    fn rejects_resource_limit_before_processing() {
        let limits = ImageProcessingLimits {
            max_input_bytes: 8,
            ..ImageProcessingLimits::default()
        };
        assert!(matches!(
            process_image(&png(2, 2), &recipe(), limits),
            Err(ImageProcessingError::InputBytesLimit { .. })
        ));
    }

    #[test]
    fn all_output_formats_are_deterministic_and_decodable() {
        let input = png(16, 8);
        for format in [
            ImageOutputFormat::Jpeg,
            ImageOutputFormat::Png,
            ImageOutputFormat::Webp,
            ImageOutputFormat::Avif,
        ] {
            let mut recipe = recipe();
            recipe.resize = None;
            recipe.output = format;
            let first = process_image(&input, &recipe, ImageProcessingLimits::default()).unwrap();
            let second = process_image(&input, &recipe, ImageProcessingLimits::default()).unwrap();
            assert_eq!(first.bytes, second.bytes, "{format:?} output changed");
            let expected_digest = match format {
                ImageOutputFormat::Jpeg => {
                    "cfa3d0c44aab3eab0951bb310dc6ce926359c26635297806572a077c39684e97"
                }
                ImageOutputFormat::Png => {
                    "993a5e2ad1e67b33630e77fcf053435d60fa5b41c8a5383c597a19839950db6f"
                }
                ImageOutputFormat::Webp => {
                    "d6712b1ba836e8edc137d27ac3fd6cd66dbed0faf802f7eaa23e5df13d24b6c5"
                }
                ImageOutputFormat::Avif => {
                    "0135335872a973e4a3e5fd855082e364ba9b2cd069bc0a868e3c63b8eed02b77"
                }
            };
            assert_eq!(hex::encode(Sha256::digest(&first.bytes)), expected_digest);
            if format == ImageOutputFormat::Avif {
                assert_eq!(&first.bytes[4..12], b"ftypavif");
                continue;
            }
            assert_eq!(
                image::load_from_memory_with_format(
                    &first.bytes,
                    match format {
                        ImageOutputFormat::Jpeg => ImageFormat::Jpeg,
                        ImageOutputFormat::Png => ImageFormat::Png,
                        ImageOutputFormat::Webp => ImageFormat::WebP,
                        ImageOutputFormat::Avif => unreachable!(),
                    }
                )
                .unwrap()
                .dimensions(),
                (16, 8)
            );
        }
    }

    #[test]
    fn output_limit_is_enforced_after_encoding() {
        let limits = ImageProcessingLimits {
            max_output_bytes: 8,
            ..ImageProcessingLimits::default()
        };
        assert!(matches!(
            process_image(&png(16, 16), &recipe(), limits),
            Err(ImageProcessingError::OutputBytesLimit { .. })
        ));
    }

    #[test]
    fn decoded_memory_limit_is_enforced_before_allocation() {
        let limits = ImageProcessingLimits {
            max_decoded_bytes: 1,
            ..ImageProcessingLimits::default()
        };
        assert!(matches!(
            process_image(&png(16, 16), &recipe(), limits),
            Err(ImageProcessingError::DecodedBytesLimit { .. })
        ));
    }

    #[test]
    fn animated_input_is_rejected_instead_of_silently_dropping_frames() {
        let frame = || image::Frame::new(ImageBuffer::from_pixel(2, 2, Rgba([10, 20, 30, 255])));
        let mut input = Vec::new();
        image::codecs::gif::GifEncoder::new(&mut input)
            .encode_frames([frame(), frame()])
            .unwrap();
        assert!(matches!(
            process_image(&input, &recipe(), ImageProcessingLimits::default()),
            Err(ImageProcessingError::FrameLimit { .. })
        ));
    }

    #[test]
    fn exif_orientation_is_normalized_before_recipe_transforms() {
        let jpeg = mozjpeg_rs::Encoder::progressive_balanced()
            .quality(100)
            .encode_rgb(&[255, 0, 0, 0, 255, 0], 2, 1)
            .unwrap();
        let exif = [
            0xff, 0xe1, 0x00, 0x22, b'E', b'x', b'i', b'f', 0x00, 0x00, b'I', b'I', 0x2a, 0x00,
            0x08, 0x00, 0x00, 0x00, 0x01, 0x00, 0x12, 0x01, 0x03, 0x00, 0x01, 0x00, 0x00, 0x00,
            0x06, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];
        let mut oriented = Vec::with_capacity(jpeg.len() + exif.len());
        oriented.extend_from_slice(&jpeg[..2]);
        oriented.extend_from_slice(&exif);
        oriented.extend_from_slice(&jpeg[2..]);
        let mut output_recipe = recipe();
        output_recipe.resize = None;
        let output =
            process_image(&oriented, &output_recipe, ImageProcessingLimits::default()).unwrap();
        assert_eq!((output.width, output.height), (1, 2));
    }

    #[test]
    fn jpeg_flattens_alpha_against_explicit_background() {
        let source = DynamicImage::ImageRgba8(ImageBuffer::from_pixel(1, 1, Rgba([0, 0, 0, 0])));
        let mut input = Cursor::new(Vec::new());
        source.write_to(&mut input, ImageFormat::Png).unwrap();
        let mut recipe = recipe();
        recipe.resize = None;
        recipe.output = ImageOutputFormat::Jpeg;
        recipe.quality = 100;
        recipe.background = ImageBackground {
            red: 240,
            green: 10,
            blue: 20,
        };
        let output = process_image(
            &input.into_inner(),
            &recipe,
            ImageProcessingLimits::default(),
        )
        .unwrap();
        let decoded = image::load_from_memory(&output.bytes).unwrap().to_rgb8();
        let pixel = decoded.get_pixel(0, 0);
        assert!(pixel[0] > 200);
        assert!(pixel[1] < 50);
        assert!(pixel[2] < 60);
    }

    #[test]
    fn worker_rejects_unbounded_configuration() {
        assert!(matches!(
            ImageWorker::new(0, Duration::from_secs(1), ImageProcessingLimits::default()),
            Err(ImageProcessingError::InvalidLimits(_))
        ));
    }

    #[tokio::test]
    async fn worker_processes_on_the_blocking_pool() {
        let worker =
            ImageWorker::new(2, Duration::from_secs(10), ImageProcessingLimits::default()).unwrap();
        let output = worker.process(png(12, 6), recipe()).await.unwrap();
        assert_eq!((output.width, output.height), (200, 100));
    }

    #[tokio::test]
    async fn worker_enforces_concurrency_before_starting_cpu_work() {
        let worker =
            ImageWorker::new(1, Duration::from_secs(10), ImageProcessingLimits::default()).unwrap();
        let held = worker.permits.clone().acquire_owned().await.unwrap();
        let queued_worker = worker.clone();
        let queued = tokio::spawn(async move { queued_worker.process(png(12, 6), recipe()).await });
        tokio::task::yield_now().await;
        assert!(!queued.is_finished());
        drop(held);
        assert!(queued.await.unwrap().is_ok());
    }

    #[tokio::test]
    async fn worker_timeout_includes_queue_wait() {
        let worker = ImageWorker::new(
            1,
            Duration::from_millis(20),
            ImageProcessingLimits::default(),
        )
        .unwrap();
        let held = worker.permits.clone().acquire_owned().await.unwrap();
        let result = worker.process(png(12, 6), recipe()).await;
        drop(held);
        assert!(matches!(result, Err(ImageProcessingError::Timeout)));
    }
}
