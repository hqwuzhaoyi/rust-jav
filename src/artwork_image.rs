use std::{io::Cursor, sync::Mutex};

use image::{ImageFormat, ImageReader};
use once_cell::sync::Lazy;

pub(crate) const MAX_ARTWORK_BYTES: u64 = 32 * 1024 * 1024;
const MAX_ARTWORK_DIMENSION: u32 = 16_384;
const MAX_ARTWORK_PIXELS: u64 = 16_000_000;
const MAX_ARTWORK_OUTPUT_BYTES: u64 = 48 * 1024 * 1024;
const MAX_ARTWORK_DECODE_ALLOC: u64 = 64 * 1024 * 1024;
const MAX_WEBP_INTERNAL_BYTES: usize = 64 * 1024 * 1024;
static ARTWORK_DECODE_LIMIT: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ValidationErrorKind {
    Unrecognized,
    Animated,
    TruncatedOrCorrupt,
    TooLarge,
    Unreadable,
}

#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub(crate) struct ValidationError {
    pub kind: ValidationErrorKind,
    message: String,
}

impl ValidationError {
    fn new(kind: ValidationErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ValidatedImage {
    pub bytes: Vec<u8>,
    pub content_type: &'static str,
}

pub(crate) fn validate(bytes: Vec<u8>) -> Result<ValidatedImage, ValidationError> {
    if bytes.len() as u64 > MAX_ARTWORK_BYTES {
        return Err(ValidationError::new(
            ValidationErrorKind::TooLarge,
            format!("artwork exceeds the {MAX_ARTWORK_BYTES} byte safety limit"),
        ));
    }
    let (format, content_type) = sniff_format(&bytes).ok_or_else(|| {
        ValidationError::new(
            ValidationErrorKind::Unrecognized,
            "artwork is not recognized JPEG, PNG, or WebP content",
        )
    })?;
    validate_decode(&bytes, format)?;
    Ok(ValidatedImage {
        bytes,
        content_type,
    })
}

pub(crate) fn sniff_content_type(bytes: &[u8]) -> Option<&'static str> {
    sniff_format(bytes).map(|(_, content_type)| content_type)
}

fn sniff_format(bytes: &[u8]) -> Option<(ImageFormat, &'static str)> {
    match image::guess_format(bytes).ok()? {
        ImageFormat::Jpeg => Some((ImageFormat::Jpeg, "image/jpeg")),
        ImageFormat::Png => Some((ImageFormat::Png, "image/png")),
        ImageFormat::WebP => Some((ImageFormat::WebP, "image/webp")),
        _ => None,
    }
}

fn validate_pixel_budget(width: u32, height: u32) -> Result<(), ValidationError> {
    if width == 0 || height == 0 {
        return Err(ValidationError::new(
            ValidationErrorKind::TruncatedOrCorrupt,
            "image dimensions must be non-zero",
        ));
    }
    if width > MAX_ARTWORK_DIMENSION || height > MAX_ARTWORK_DIMENSION {
        return Err(ValidationError::new(
            ValidationErrorKind::TooLarge,
            format!(
                "image dimensions {width}x{height} exceed the {MAX_ARTWORK_DIMENSION}px axis limit"
            ),
        ));
    }
    let pixels = u64::from(width)
        .checked_mul(u64::from(height))
        .ok_or_else(|| {
            ValidationError::new(
                ValidationErrorKind::TooLarge,
                "image pixel count overflowed",
            )
        })?;
    if pixels > MAX_ARTWORK_PIXELS {
        return Err(ValidationError::new(
            ValidationErrorKind::TooLarge,
            format!(
                "image contains {pixels} pixels, exceeding the {MAX_ARTWORK_PIXELS} pixel limit"
            ),
        ));
    }
    let output_bytes = pixels.checked_mul(4).ok_or_else(|| {
        ValidationError::new(
            ValidationErrorKind::TooLarge,
            "decoded image output size overflowed",
        )
    })?;
    if output_bytes > MAX_ARTWORK_OUTPUT_BYTES {
        return Err(ValidationError::new(
            ValidationErrorKind::TooLarge,
            format!(
                "decoded output (RGBA) requires {output_bytes} bytes, exceeding the {MAX_ARTWORK_OUTPUT_BYTES} byte limit"
            ),
        ));
    }
    Ok(())
}

fn validate_decode(bytes: &[u8], format: ImageFormat) -> Result<(), ValidationError> {
    let _permit = ARTWORK_DECODE_LIMIT.lock().map_err(|_| {
        ValidationError::new(
            ValidationErrorKind::Unreadable,
            "artwork decoder concurrency gate was poisoned",
        )
    })?;

    if format == ImageFormat::WebP {
        let mut decoder = image_webp::WebPDecoder::new(Cursor::new(bytes)).map_err(|error| {
            ValidationError::new(
                ValidationErrorKind::TruncatedOrCorrupt,
                format!("WebP header is invalid: {error}"),
            )
        })?;
        if decoder.is_animated() {
            return Err(ValidationError::new(
                ValidationErrorKind::Animated,
                "animated WebP artwork is not supported; replace it with a static image",
            ));
        }
        let (width, height) = decoder.dimensions();
        validate_pixel_budget(width, height)?;
        let output_size = decoder.output_buffer_size().ok_or_else(|| {
            ValidationError::new(
                ValidationErrorKind::TooLarge,
                "WebP decoded output size overflowed",
            )
        })?;
        if output_size as u64 > MAX_ARTWORK_OUTPUT_BYTES {
            return Err(ValidationError::new(
                ValidationErrorKind::TooLarge,
                format!(
                    "WebP decoded output requires {output_size} bytes, exceeding the {MAX_ARTWORK_OUTPUT_BYTES} byte limit"
                ),
            ));
        }
        decoder.set_memory_limit(MAX_WEBP_INTERNAL_BYTES);
        let mut output = Vec::new();
        output.try_reserve_exact(output_size).map_err(|error| {
            ValidationError::new(
                ValidationErrorKind::TooLarge,
                format!("WebP decoded output allocation failed: {error}"),
            )
        })?;
        output.resize(output_size, 0);
        decoder.read_image(&mut output).map_err(|error| {
            ValidationError::new(
                ValidationErrorKind::TruncatedOrCorrupt,
                format!("WebP decode failed: {error}"),
            )
        })?;
        return Ok(());
    }

    let dimensions = ImageReader::with_format(Cursor::new(bytes), format)
        .into_dimensions()
        .map_err(|error| {
            ValidationError::new(
                ValidationErrorKind::TruncatedOrCorrupt,
                format!("image dimensions cannot be decoded: {error}"),
            )
        })?;
    validate_pixel_budget(dimensions.0, dimensions.1)?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_ARTWORK_DIMENSION);
    limits.max_image_height = Some(MAX_ARTWORK_DIMENSION);
    limits.max_alloc = Some(MAX_ARTWORK_DECODE_ALLOC);
    let mut reader = ImageReader::with_format(Cursor::new(bytes), format);
    reader.limits(limits);
    reader.decode().map_err(|error| {
        ValidationError::new(
            ValidationErrorKind::TruncatedOrCorrupt,
            format!("image decode failed: {error}"),
        )
    })?;
    Ok(())
}
