use std::fmt;
use std::str::FromStr;

use crate::emCore::emPanelTree::PanelId;

/// Axis orientation for layout algorithms.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Orientation {
    Horizontal,
    Vertical,
    /// Switches between horizontal and vertical based on the container's
    /// tallness (height / width). If tallness <= threshold, use horizontal
    /// (wide panel → children side by side); if tallness > threshold, use
    /// vertical (tall panel → children stacked). Matches C++
    /// `horizontal = (h/w <= OrientationThreshold)`.
    Adaptive {
        tallness_threshold: f64,
    },
}

impl Orientation {
    /// Resolve to a concrete horizontal or vertical based on container rect.
    pub fn resolve(self, w: f64, h: f64) -> ResolvedOrientation {
        match self {
            Self::Horizontal => ResolvedOrientation::Horizontal,
            Self::Vertical => ResolvedOrientation::Vertical,
            Self::Adaptive { tallness_threshold } => {
                let tallness = if w > 0.0 { h / w } else { f64::INFINITY };
                if tallness <= tallness_threshold {
                    ResolvedOrientation::Horizontal
                } else {
                    ResolvedOrientation::Vertical
                }
            }
        }
    }
}

/// A resolved (non-adaptive) orientation.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ResolvedOrientation {
    Horizontal,
    Vertical,
}

/// Cross-axis alignment for children within a layout.
///
/// Used by emRasterLayout for block-level alignment. emLinearLayout uses
/// per-axis `AlignmentH`/`AlignmentV` instead.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum Alignment {
    Start,
    #[default]
    Center,
    End,
    Stretch,
}

impl fmt::Display for Alignment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Alignment::Start => write!(f, "Start"),
            Alignment::Center => write!(f, "Center"),
            Alignment::End => write!(f, "End"),
            Alignment::Stretch => write!(f, "Stretch"),
        }
    }
}

impl FromStr for Alignment {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Start" => Ok(Alignment::Start),
            "Center" => Ok(Alignment::Center),
            "End" => Ok(Alignment::End),
            "Stretch" => Ok(Alignment::Stretch),
            _ => Err(format!("unknown Alignment: {s:?}")),
        }
    }
}

/// Horizontal alignment (matching C++ EM_ALIGN_LEFT / EM_ALIGN_CENTER / EM_ALIGN_RIGHT).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum AlignmentH {
    Left,
    #[default]
    Center,
    Right,
}

impl fmt::Display for AlignmentH {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AlignmentH::Left => write!(f, "Left"),
            AlignmentH::Center => write!(f, "Center"),
            AlignmentH::Right => write!(f, "Right"),
        }
    }
}

impl FromStr for AlignmentH {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Left" => Ok(AlignmentH::Left),
            "Center" => Ok(AlignmentH::Center),
            "Right" => Ok(AlignmentH::Right),
            _ => Err(format!("unknown AlignmentH: {s:?}")),
        }
    }
}

/// Vertical alignment (matching C++ EM_ALIGN_TOP / EM_ALIGN_CENTER / EM_ALIGN_BOTTOM).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum AlignmentV {
    Top,
    #[default]
    Center,
    Bottom,
}

impl fmt::Display for AlignmentV {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AlignmentV::Top => write!(f, "Top"),
            AlignmentV::Center => write!(f, "Center"),
            AlignmentV::Bottom => write!(f, "Bottom"),
        }
    }
}

impl FromStr for AlignmentV {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Top" => Ok(AlignmentV::Top),
            "Center" => Ok(AlignmentV::Center),
            "Bottom" => Ok(AlignmentV::Bottom),
            _ => Err(format!("unknown AlignmentV: {s:?}")),
        }
    }
}

/// Spacing configuration for layouts.
///
/// Matches C++ spacing model with separate horizontal/vertical inner spacing.
/// In horizontal layout, `inner_h` goes between children and `margin_top`/
/// `margin_bottom` above/below. In vertical layout, `inner_v` goes between
/// children and `margin_left`/`margin_right` beside.
#[derive(Clone, Debug)]
pub struct Spacing {
    /// Space between children when laid out horizontally.
    pub inner_h: f64,
    /// Space between children when laid out vertically.
    pub inner_v: f64,
    pub margin_left: f64,
    pub margin_right: f64,
    pub margin_top: f64,
    pub margin_bottom: f64,
}

impl Default for Spacing {
    fn default() -> Self {
        Self {
            inner_h: 0.0,
            inner_v: 0.0,
            margin_left: 0.0,
            margin_right: 0.0,
            margin_top: 0.0,
            margin_bottom: 0.0,
        }
    }
}

impl Spacing {
    pub fn uniform(margin: f64, inner: f64) -> Self {
        Self {
            inner_h: inner,
            inner_v: inner,
            margin_left: margin,
            margin_right: margin,
            margin_top: margin,
            margin_bottom: margin,
        }
    }

    /// Get inner spacing for a resolved orientation.
    pub fn inner_for(&self, orientation: ResolvedOrientation) -> f64 {
        match orientation {
            ResolvedOrientation::Horizontal => self.inner_h,
            ResolvedOrientation::Vertical => self.inner_v,
        }
    }

    pub(crate) fn clamped(&self) -> Self {
        Self {
            inner_h: self.inner_h.max(0.0),
            inner_v: self.inner_v.max(0.0),
            margin_left: self.margin_left.max(0.0),
            margin_right: self.margin_right.max(0.0),
            margin_top: self.margin_top.max(0.0),
            margin_bottom: self.margin_bottom.max(0.0),
        }
    }
}

/// Per-child constraint used by emLinearLayout and emPackLayout.
#[derive(Clone, Debug)]
pub struct ChildConstraint {
    /// Relative weight for distributing space on the main axis.
    pub weight: f64,
    /// Minimum size on the main axis.
    pub min_main: f64,
    /// Maximum size on the main axis (f64::INFINITY = unconstrained).
    pub max_main: f64,
    /// Preferred tallness (height / width) for layout scoring.
    pub preferred_tallness: f64,
    /// Minimum tallness (height / width) constraint.
    pub min_tallness: f64,
    /// Maximum tallness (height / width) constraint (f64::INFINITY = unconstrained).
    pub max_tallness: f64,
}

impl Default for ChildConstraint {
    fn default() -> Self {
        Self {
            weight: 1.0,
            min_main: 0.0,
            max_main: f64::INFINITY,
            preferred_tallness: 0.2,
            min_tallness: 1e-4,
            max_tallness: 1e4,
        }
    }
}

/// Helper: get constraint for a child, falling back to default.
pub(crate) fn get_constraint<'a>(
    constraints: &'a std::collections::HashMap<PanelId, ChildConstraint>,
    child: PanelId,
    default: &'a ChildConstraint,
) -> &'a ChildConstraint {
    constraints.get(&child).unwrap_or(default)
}

/// Position the aux panel and return its ID (if any) so the layout can skip it.
///
/// Replicates `emBorder::LayoutChildren()` base-call behavior: finds the aux
/// panel by name, positions it using `border.get_aux_rect()`, and returns its
/// PanelId so the layout algorithm can exclude it from normal layout.
pub(crate) fn position_aux_panel(
    ctx: &mut crate::emCore::emPanelCtx::PanelCtx,
    border: &crate::emCore::emBorder::emBorder,
) -> Option<PanelId> {
    let aux_name = border.GetAuxPanelName();
    if aux_name.is_empty() {
        return None;
    }

    let aux_id = ctx.find_child_by_name(aux_name)?;
    let r = ctx.layout_rect();
    let aux_rect = border.GetAuxRect(r.w, r.h)?;
    ctx.layout_child(aux_id, aux_rect.x, aux_rect.y, aux_rect.w, aux_rect.h);
    Some(aux_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orientation_resolve_wide() {
        let o = Orientation::Adaptive {
            tallness_threshold: 1.0,
        };
        assert_eq!(o.resolve(2.0, 1.0), ResolvedOrientation::Horizontal);
    }

    #[test]
    fn orientation_resolve_tall() {
        let o = Orientation::Adaptive {
            tallness_threshold: 1.0,
        };
        assert_eq!(o.resolve(1.0, 2.0), ResolvedOrientation::Vertical);
    }

    #[test]
    fn orientation_resolve_square() {
        let o = Orientation::Adaptive {
            tallness_threshold: 1.0,
        };
        // tallness = 1.0 <= threshold 1.0 → Horizontal (boundary)
        assert_eq!(o.resolve(1.0, 1.0), ResolvedOrientation::Horizontal);
    }

    #[test]
    fn alignment_display_roundtrip() {
        for variant in [
            Alignment::Start,
            Alignment::Center,
            Alignment::End,
            Alignment::Stretch,
        ] {
            let s = variant.to_string();
            let parsed: Alignment = s.parse().unwrap();
            assert_eq!(parsed, variant);
        }

        for variant in [AlignmentH::Left, AlignmentH::Center, AlignmentH::Right] {
            let s = variant.to_string();
            let parsed: AlignmentH = s.parse().unwrap();
            assert_eq!(parsed, variant);
        }

        for variant in [AlignmentV::Top, AlignmentV::Center, AlignmentV::Bottom] {
            let s = variant.to_string();
            let parsed: AlignmentV = s.parse().unwrap();
            assert_eq!(parsed, variant);
        }
    }

    #[test]
    fn spacing_uniform() {
        let s = Spacing::uniform(5.0, 3.0);
        assert_eq!(s.margin_left, 5.0);
        assert_eq!(s.margin_right, 5.0);
        assert_eq!(s.margin_top, 5.0);
        assert_eq!(s.margin_bottom, 5.0);
        assert_eq!(s.inner_h, 3.0);
        assert_eq!(s.inner_v, 3.0);
    }

    #[test]
    fn child_constraint_defaults() {
        let c = ChildConstraint::default();
        assert_eq!(c.weight, 1.0);
        assert_eq!(c.min_main, 0.0);
        assert!(c.max_main.is_infinite());
        assert_eq!(c.preferred_tallness, 0.2);
        assert_eq!(c.min_tallness, 1e-4);
        assert_eq!(c.max_tallness, 1e4);
    }
}
