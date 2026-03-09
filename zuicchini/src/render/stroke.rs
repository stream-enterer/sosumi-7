use crate::foundation::Color;

/// Line join style.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum LineJoin {
    Miter,
    Round,
    Bevel,
}

/// Line cap style.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum LineCap {
    Butt,
    Round,
    Square,
}

/// Dash pattern type matching C++ `emStroke::DashTypeEnum`.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum DashType {
    /// Solid line (no dashes).
    Solid,
    /// Dashes only.
    Dashed,
    /// Dots only.
    Dotted,
    /// Alternating dashes and dots.
    DashDotted,
}

/// Stroke end type matching Eagle Mode's 17 `emStrokeEnd` variants.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum StrokeEndType {
    Butt,
    Cap,
    Arrow,
    ContourArrow,
    LineArrow,
    Triangle,
    ContourTriangle,
    Square,
    ContourSquare,
    HalfSquare,
    Circle,
    ContourCircle,
    HalfCircle,
    Diamond,
    ContourDiamond,
    HalfDiamond,
    Stroke,
}

/// Stroke end decoration with configurable color and size factors.
#[derive(Copy, Clone, Debug)]
pub struct StrokeEnd {
    /// The type of end decoration.
    pub end_type: StrokeEndType,
    /// Fill color for Contour* variants.
    pub inner_color: Color,
    /// Multiplier on decoration width (default 1.0).
    pub width_factor: f64,
    /// Multiplier on decoration length (default 1.0).
    pub length_factor: f64,
}

impl StrokeEnd {
    /// Create a butt (no decoration) stroke end.
    pub fn butt() -> Self {
        Self {
            end_type: StrokeEndType::Butt,
            inner_color: Color::TRANSPARENT,
            width_factor: 1.0,
            length_factor: 1.0,
        }
    }

    /// Create a stroke end with the given type and default factors.
    pub fn new(end_type: StrokeEndType) -> Self {
        Self {
            end_type,
            inner_color: Color::TRANSPARENT,
            width_factor: 1.0,
            length_factor: 1.0,
        }
    }

    /// Set the inner color (for Contour* variants).
    pub fn with_inner_color(mut self, color: Color) -> Self {
        self.inner_color = color;
        self
    }

    /// Set the width factor.
    pub fn with_width_factor(mut self, factor: f64) -> Self {
        self.width_factor = factor;
        self
    }

    /// Set the length factor.
    pub fn with_length_factor(mut self, factor: f64) -> Self {
        self.length_factor = factor;
        self
    }

    /// Whether this end type draws a decoration (everything except Butt and Cap).
    /// Matches C++ `emStrokeEnd::IsDecorated()` which returns `Type >= ARROW`.
    pub fn is_decorated(&self) -> bool {
        !matches!(self.end_type, StrokeEndType::Butt | StrokeEndType::Cap)
    }
}

/// Stroke properties for outlined shapes.
#[derive(Clone, Debug)]
pub struct Stroke {
    /// Stroke color.
    pub color: Color,
    /// Stroke width in pixels.
    pub width: f64,
    /// Line join style.
    pub join: LineJoin,
    /// Line cap style.
    pub cap: LineCap,
    /// Start end style.
    pub start_end: StrokeEnd,
    /// Finish end style.
    pub finish_end: StrokeEnd,
    /// Dash pattern: alternating on/off lengths. Empty = solid line.
    /// This is the legacy API; prefer `dash_type` + factors for C++ parity.
    pub dash_pattern: Vec<f64>,
    /// Dash offset (legacy pattern API).
    pub dash_offset: f64,
    /// Dash type (C++ parity API). Overrides `dash_pattern` when not `Solid`.
    pub dash_type: DashType,
    /// Dash length factor (C++ `DashLengthFactor`). Default 1.0.
    pub dash_length_factor: f64,
    /// Gap length factor (C++ `GapLengthFactor`). Default 1.0.
    pub gap_length_factor: f64,
}

impl Default for Stroke {
    fn default() -> Self {
        Self {
            color: Color::BLACK,
            width: 1.0,
            join: LineJoin::Miter,
            cap: LineCap::Butt,
            start_end: StrokeEnd::butt(),
            finish_end: StrokeEnd::butt(),
            dash_pattern: Vec::new(),
            dash_offset: 0.0,
            dash_type: DashType::Solid,
            dash_length_factor: 1.0,
            gap_length_factor: 1.0,
        }
    }
}

impl Stroke {
    /// Create a simple solid stroke with the given color and width.
    pub fn new(color: Color, width: f64) -> Self {
        Self {
            color,
            width,
            ..Default::default()
        }
    }

    /// Whether this stroke uses any dash pattern (via either API).
    pub fn is_dashed(&self) -> bool {
        self.dash_type != DashType::Solid || !self.dash_pattern.is_empty()
    }
}
