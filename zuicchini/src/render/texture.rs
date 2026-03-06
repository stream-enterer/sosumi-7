use crate::foundation::{Color, Image};

/// How to extend an image beyond its bounds.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum ImageExtension {
    /// Clamp to edge pixels.
    Clamp,
    /// Repeat (tile).
    Repeat,
    /// Zero/transparent beyond bounds.
    Zero,
}

/// Quality hint for image rendering.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum ImageQuality {
    /// Nearest-neighbor sampling.
    Nearest,
    /// Bilinear interpolation.
    Bilinear,
    /// Box filter for downscaling.
    AreaSampled,
    /// Catmull-Rom bicubic (4x4 kernel).
    Bicubic,
    /// Windowed sinc (4-tap).
    Lanczos,
    /// Edge-sensitive adaptive (Hermite/bicubic blend).
    Adaptive,
}

/// A texture describes how a shape is filled.
#[derive(Clone, Debug)]
pub enum Texture {
    /// Solid color fill.
    SolidColor(Color),
    /// Image fill with extension and quality options.
    Image {
        image: Image,
        extension: ImageExtension,
        quality: ImageQuality,
    },
    /// Linear gradient between two colors.
    LinearGradient {
        color_a: Color,
        color_b: Color,
        /// Start point (x, y) in local coordinates.
        start: (f64, f64),
        /// End point (x, y) in local coordinates.
        end: (f64, f64),
    },
    /// Radial gradient between two colors.
    RadialGradient {
        color_inner: Color,
        color_outer: Color,
        /// Center (x, y) in local coordinates.
        center: (f64, f64),
        /// Radius.
        radius: f64,
    },
    /// Image tinted with a color (multiplied).
    ImageColored {
        image: Image,
        color: Color,
        extension: ImageExtension,
        quality: ImageQuality,
    },
}

impl Texture {
    /// Create a solid color texture.
    pub fn color(c: Color) -> Self {
        Texture::SolidColor(c)
    }
}
