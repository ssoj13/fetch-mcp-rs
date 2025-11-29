
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use schemars::JsonSchema;

/// Image information
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ImageInfo {
    /// Image format (PNG, JPEG, GIF, WebP, etc.)
    pub format: String,

    /// Image width in pixels
    pub width: u32,

    /// Image height in pixels
    pub height: u32,

    /// Color type (RGB, RGBA, Grayscale, etc.)
    pub color_type: String,

    /// File size in bytes
    pub size_bytes: usize,

    /// Aspect ratio (width / height)
    pub aspect_ratio: f32,

    /// Megapixels
    pub megapixels: f32,

    /// Size category (thumbnail, small, medium, large, very_large, ultra_high_res)
    pub size_category: String,

    /// Orientation (landscape, portrait, square)
    pub orientation: String,
}

/// Extract image information from bytes
#[cfg(feature = "images")]
pub fn extract_image_info(image_bytes: &[u8]) -> Result<ImageInfo> {
    // Detect format first (fast)
    let format = detect_image_format(image_bytes)?;

    // Get dimensions (fast, no full decode)
    let (width, height) = get_image_dimensions(image_bytes)?;

    // Calculate derived properties
    let size_bytes = image_bytes.len();
    let aspect_ratio = width as f32 / height as f32;
    let megapixels = (width * height) as f32 / 1_000_000.0;
    let size_category = categorize_image_size(width, height).to_string();
    let orientation = get_image_orientation(width, height).to_string();

    // Load full image only for color type
    let img = image::load_from_memory(image_bytes).context("Failed to load image")?;
    let color_type = format!("{:?}", img.color());

    Ok(ImageInfo {
        format,
        width,
        height,
        color_type,
        size_bytes,
        aspect_ratio,
        megapixels,
        size_category,
        orientation,
    })
}

/// Detect image format from bytes (faster than full loading)
#[cfg(feature = "images")]
pub fn detect_image_format(image_bytes: &[u8]) -> Result<String> {
    let format = image::guess_format(image_bytes).context("Failed to detect image format")?;
    Ok(format!("{:?}", format))
}

/// Get image dimensions without full decoding (fast)
#[cfg(feature = "images")]
pub fn get_image_dimensions(image_bytes: &[u8]) -> Result<(u32, u32)> {
    let reader = image::ImageReader::new(std::io::Cursor::new(image_bytes))
        .with_guessed_format()
        .context("Failed to create image reader")?;

    let dimensions = reader
        .into_dimensions()
        .context("Failed to read image dimensions")?;

    Ok(dimensions)
}

/// Calculate image file size category
pub fn categorize_image_size(width: u32, height: u32) -> &'static str {
    let pixels = width * height;

    match pixels {
        0..=100_000 => "thumbnail",           // < 0.1 MP
        100_001..=500_000 => "small",         // 0.1 - 0.5 MP
        500_001..=2_000_000 => "medium",      // 0.5 - 2 MP
        2_000_001..=8_000_000 => "large",     // 2 - 8 MP
        8_000_001..=20_000_000 => "very_large", // 8 - 20 MP
        _ => "ultra_high_res",                 // > 20 MP
    }
}

/// Check if image is landscape or portrait
pub fn get_image_orientation(width: u32, height: u32) -> &'static str {
    if width > height {
        "landscape"
    } else if height > width {
        "portrait"
    } else {
        "square"
    }
}

/// Fallback implementation when images feature is disabled
#[cfg(not(feature = "images"))]
pub fn extract_image_info(_image_bytes: &[u8]) -> Result<ImageInfo> {
    anyhow::bail!("Image support is not enabled. Rebuild with --features images")
}

#[cfg(not(feature = "images"))]
pub fn detect_image_format(_image_bytes: &[u8]) -> Result<String> {
    anyhow::bail!("Image support is not enabled. Rebuild with --features images")
}

#[cfg(not(feature = "images"))]
pub fn get_image_dimensions(_image_bytes: &[u8]) -> Result<(u32, u32)> {
    anyhow::bail!("Image support is not enabled. Rebuild with --features images")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_categorize_image_size() {
        assert_eq!(categorize_image_size(100, 100), "thumbnail");
        assert_eq!(categorize_image_size(800, 600), "small");
        assert_eq!(categorize_image_size(1920, 1080), "medium");
        assert_eq!(categorize_image_size(4000, 3000), "large");
    }

    #[test]
    fn test_image_orientation() {
        assert_eq!(get_image_orientation(1920, 1080), "landscape");
        assert_eq!(get_image_orientation(1080, 1920), "portrait");
        assert_eq!(get_image_orientation(1000, 1000), "square");
    }

    #[cfg(feature = "images")]
    #[test]
    fn test_extract_image_info() {
        // Create a simple 10x10 red PNG image
        use image::{ImageBuffer, Rgb};

        let img: ImageBuffer<Rgb<u8>, Vec<u8>> = ImageBuffer::from_fn(10, 10, |_, _| {
            Rgb([255, 0, 0])
        });

        let mut bytes: Vec<u8> = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Png)
            .unwrap();

        let info = extract_image_info(&bytes).unwrap();
        assert_eq!(info.width, 10);
        assert_eq!(info.height, 10);
        assert_eq!(info.format, "Png");
    }
}
