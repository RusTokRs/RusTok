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
        if let Some(crop) = self.crop {
            if crop.width == 0 || crop.height == 0 {
                return Err(ImageProcessingError::InvalidRecipe(
                    "crop dimensions must be greater than zero".to_string(),
                ));
            }
        }
        if let Some(resize) = self.resize {
            if resize.width == 0 || resize.height == 0 {
                return Err(ImageProcessingError::InvalidRecipe(
                    "resize dimensions must be greater than zero".to_string(),
                ));
            }
            let requested_pixels = u64::from(resize.width) * u64::from(resize.height);
            if requested_pixels > limits.max_output_pixels {
                return Err(ImageProcessingError::LimitExceeded(
                    "requested output dimensions exceed Media limits".to_string(),
                ));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImageProcessingLimits {
    pub max_input_bytes: u64,
    pub max_input_pixels: u64,
    pub max_output_pixels: u64,
    pub max_frames: u32,
    pub max_concurrency: usize,
    pub operation_timeout: Duration,
}

impl Default for ImageProcessingLimits {
    fn default() -> Self {
        Self {
            max_input_bytes: 32 * 1024 * 1024,
            max_input_pixels: 64 * 1024 * 1024,
            max_output_pixels: 64 * 1024 * 1024,
            max_frames: 1,
            max_concurrency: 4,
            operation_timeout: Duration::from_secs(30),
        }
    }
}

#[derive(Debug, Error)]
pub enum ImageProcessingError {
    #[error("invalid image recipe: {0}")]
    InvalidRecipe(String),
    #[error("image processing limit exceeded: {0}")]
    LimitExceeded(String),
    #[error("image decode failed: {0}")]
    Decode(String),
    #[error("image encode failed: {0}")]
    Encoder(String),
    #[error("image processing timed out")]
    Timeout,
    #[error("image processing worker failed")]
    Worker,
}

impl From<image::ImageError> for ImageProcessingError {
    fn from(value: image::ImageError) -> Self {
        Self::Decode(value.to_string())
    }
}

#[derive(Clone)]
pub struct ImageProcessor {
    limits: ImageProcessingLimits,
    semaphore: Arc<Semaphore>,
}

impl ImageProcessor {
    pub fn new(limits: ImageProcessingLimits) -> Self {
        Self {
            limits,
            semaphore: Arc::new(Semaphore::new(limits.max_concurrency.max(1))),
        }
    }

    pub async fn process(
        &self,
        input: Vec<u8>,
        recipe: ImageRecipe,
    ) -> Result<Vec<u8>, ImageProcessingError> {
        if input.len() as u64 > self.limits.max_input_bytes {
            return Err(ImageProcessingError::LimitExceeded(
                "input payload exceeds Media limits".to_string(),
            ));
        }
        recipe.validate(self.limits)?;
        let permit = self
            .semaphore
            .acquire()
            .await
            .map_err(|_| ImageProcessingError::Worker)?;
        let limits = self.limits;
        let result = tokio::time::timeout(
            limits.operation_timeout,
            tokio::task::spawn_blocking(move || process_sync(&input, recipe, limits)),
        )
        .await;
        drop(permit);
        match result {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err(ImageProcessingError::Worker),
            Err(_) => Err(ImageProcessingError::Timeout),
        }
    }
}

fn process_sync(
    input: &[u8],
    recipe: ImageRecipe,
    limits: ImageProcessingLimits,
) -> Result<Vec<u8>, ImageProcessingError> {
    let format = image::guess_format(input)
        .map_err(|error| ImageProcessingError::Decode(error.to_string()))?;
    let reader = ImageReader::with_format(Cursor::new(input), format);
    let decoder = reader
        .into_decoder()
        .map_err(|error| ImageProcessingError::Decode(error.to_string()))?;
    let (width, height) = decoder.dimensions();
    let input_pixels = u64::from(width) * u64::from(height);
    if input_pixels > limits.max_input_pixels {
        return Err(ImageProcessingError::LimitExceeded(
            "input dimensions exceed Media limits".to_string(),
        ));
    }
    if decoder.is_apng().unwrap_or(false) {
        let frames = decoder
            .apng()
            .map_err(|error| ImageProcessingError::Decode(error.to_string()))?
            .into_frames()
            .take((limits.max_frames + 1) as usize)
            .collect_frames()
            .map_err(|error| ImageProcessingError::Decode(error.to_string()))?;
        if frames.len() > limits.max_frames as usize {
            return Err(ImageProcessingError::LimitExceeded(
                "animated image frame count exceeds Media limits".to_string(),
            ));
        }
    }
    let decoded = image::load_from_memory_with_format(input, format)
        .map_err(|error| ImageProcessingError::Decode(error.to_string()))?;
    let transformed = transform(decoded, recipe, limits)?;
    encode(&transformed, recipe)
}

fn transform(
    mut image: DynamicImage,
    recipe: ImageRecipe,
    limits: ImageProcessingLimits,
) -> Result<DynamicImage, ImageProcessingError> {
    if let Some(crop) = recipe.crop {
        if crop.x.saturating_add(crop.width) > image.width()
            || crop.y.saturating_add(crop.height) > image.height()
        {
            return Err(ImageProcessingError::InvalidRecipe(
                "crop rectangle exceeds image dimensions".to_string(),
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
        let destination_pixels = u64::from(resize.width) * u64::from(resize.height);
        if destination_pixels > limits.max_output_pixels {
            return Err(ImageProcessingError::LimitExceeded(
                "resize result exceeds Media limits".to_string(),
            ));
        }
        image = resize_with_fir(image, resize)?;
    }
    Ok(image)
}

fn resize_with_fir(
    image: DynamicImage,
    resize: ImageResize,
) -> Result<DynamicImage, ImageProcessingError> {
    let rgba = image.to_rgba8();
    if resize.cover {
        let source_width = rgba.width();
        let source_height = rgba.height();
        let source_ratio = source_width as f64 / source_height as f64;
        let target_ratio = resize.width as f64 / resize.height as f64;
        let (intermediate_width, intermediate_height) = if source_ratio > target_ratio {
            (
                ((resize.height as f64 * source_ratio).ceil() as u32).max(resize.width),
                resize.height,
            )
        } else {
            (
                resize.width,
                ((resize.width as f64 / source_ratio).ceil() as u32).max(resize.height),
            )
        };
        let mut source = Image::from_vec_u8(
            source_width,
            source_height,
            rgba.into_raw(),
            fast_image_resize::PixelType::U8x4,
        )
        .map_err(|error| ImageProcessingError::Decode(error.to_string()))?;
        let mut intermediate = Image::new(
            intermediate_width,
            intermediate_height,
            fast_image_resize::PixelType::U8x4,
        );
        let mut resizer = Resizer::new();
        resizer
            .resize(&source, &mut intermediate, None)
            .map_err(|error| ImageProcessingError::Encoder(error.to_string()))?;
        let horizontal_offset = (intermediate_width - resize.width) / 2;
        let vertical_offset = (intermediate_height - resize.height) / 2;
        source = Image::from_vec_u8(
            intermediate_width,
            intermediate_height,
            intermediate.buffer().to_vec(),
            fast_image_resize::PixelType::U8x4,
        )
        .map_err(|error| ImageProcessingError::Decode(error.to_string()))?;
        let cropped = source
            .view(
                horizontal_offset,
                vertical_offset,
                resize.width,
                resize.height,
            )
            .map_err(|error| ImageProcessingError::InvalidRecipe(error.to_string()))?;
        let buffer = cropped.image_view().buffer().to_vec();
        RgbaImage::from_raw(resize.width, resize.height, buffer)
            .map(DynamicImage::ImageRgba8)
            .ok_or_else(|| {
                ImageProcessingError::Encoder("failed to construct resized image".to_string())
            })
    } else {
        let mut source = Image::from_vec_u8(
            rgba.width(),
            rgba.height(),
            rgba.into_raw(),
            fast_image_resize::PixelType::U8x4,
        )
        .map_err(|error| ImageProcessingError::Decode(error.to_string()))?;
        let mut destination = Image::new(
            resize.width,
            resize.height,
            fast_image_resize::PixelType::U8x4,
        );
        let mut resizer = Resizer::new();
        resizer
            .resize(&source, &mut destination, None)
            .map_err(|error| ImageProcessingError::Encoder(error.to_string()))?;
        source = destination;
        RgbaImage::from_raw(resize.width, resize.height, source.buffer().to_vec())
            .map(DynamicImage::ImageRgba8)
            .ok_or_else(|| {
                ImageProcessingError::Encoder("failed to construct resized image".to_string())
            })
    }
}

fn encode(image: &DynamicImage, recipe: ImageRecipe) -> Result<Vec<u8>, ImageProcessingError> {
    let rgba = image.to_rgba8();
    let width = rgba.width();
    let height = rgba.height();
    match recipe.output {
        ImageOutputFormat::Jpeg => {
            let flattened = flatten_alpha(&rgba, recipe.background);
            let mut encoded = Vec::new();
            let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(
                &mut encoded,
                recipe.quality,
            );
            encoder.write_image(
                flattened.as_raw(),
                width,
                height,
                ExtendedColorType::Rgb8,
            )?;
            Ok(encoded)
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
            let encoded = webp::Encoder::from_rgba(rgba.as_raw(), width, height)
                .encode_simple(false, f32::from(recipe.quality))
                .map_err(|error| ImageProcessingError::Encoder(format!("{error:?}")))?;
            Ok(encoded.to_vec())
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

fn flatten_alpha(image: &RgbaImage, background: ImageBackground) -> image::RgbImage {
    let mut flattened = image::RgbImage::new(image.width(), image.height());
    for (x, y, pixel) in image.enumerate_pixels() {
        let alpha = f32::from(pixel[3]) / 255.0;
        let blend = |foreground: u8, matte: u8| {
            ((f32::from(foreground) * alpha) + (f32::from(matte) * (1.0 - alpha)))
                .round()
                .clamp(0.0, 255.0) as u8
        };
        flattened.put_pixel(
            x,
            y,
            image::Rgb([
                blend(pixel[0], background.red),
                blend(pixel[1], background.green),
                blend(pixel[2], background.blue),
            ]),
        );
    }
    flattened
}
