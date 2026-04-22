use std::ops::Mul;

/// 2D affine transformation matrix.
///
/// Represents a 3x3 matrix with constant elements in the last column:
/// ```text
///    | a00  a01  0.0 |
///    | a10  a11  0.0 |
///    | a20  a21  1.0 |
/// ```
///
/// Transforming source coordinates (sx, sy) to target coordinates (tx, ty):
/// ```text
///    tx = a00*sx + a10*sy + a20
///    ty = a01*sx + a11*sy + a21
/// ```
///
/// Port of C++ `emATMatrix` and its family of constructor classes.
#[derive(Debug, Clone, Copy)]
pub struct AffineMatrix {
    /// Storage: `a[row][col]` where row is 0..3 and col is 0..2.
    a: [[f64; 2]; 3],
}

impl PartialEq for AffineMatrix {
    fn eq(&self, other: &Self) -> bool {
        self.a[0][0] == other.a[0][0]
            && self.a[0][1] == other.a[0][1]
            && self.a[1][0] == other.a[1][0]
            && self.a[1][1] == other.a[1][1]
            && self.a[2][0] == other.a[2][0]
            && self.a[2][1] == other.a[2][1]
    }
}

impl AffineMatrix {
    /// Construct from raw elements.
    pub fn new(a00: f64, a01: f64, a10: f64, a11: f64, a20: f64, a21: f64) -> Self {
        Self {
            a: [[a00, a01], [a10, a11], [a20, a21]],
        }
    }

    /// Identity matrix (source coordinates equal target coordinates).
    pub fn identity() -> Self {
        Self::new(1.0, 0.0, 0.0, 1.0, 0.0, 0.0)
    }

    /// Translation matrix: tx = sx + dx, ty = sy + dy.
    pub fn translate(dx: f64, dy: f64) -> Self {
        Self::new(1.0, 0.0, 0.0, 1.0, dx, dy)
    }

    /// Compose translation after `m`: equivalent to multiply(m, translate(dx, dy)).
    pub fn translate_after(dx: f64, dy: f64, m: &AffineMatrix) -> Self {
        Self::new(
            m.a[0][0],
            m.a[0][1],
            m.a[1][0],
            m.a[1][1],
            m.a[2][0] + dx,
            m.a[2][1] + dy,
        )
    }

    /// Scale matrix: tx = sx * sx_factor, ty = sy * sy_factor.
    pub fn scale(fac_x: f64, fac_y: f64) -> Self {
        Self::new(fac_x, 0.0, 0.0, fac_y, 0.0, 0.0)
    }

    /// Compose scale after `m`: equivalent to multiply(m, scale(fac_x, fac_y)).
    pub fn scale_after(fac_x: f64, fac_y: f64, m: &AffineMatrix) -> Self {
        Self::new(
            m.a[0][0] * fac_x,
            m.a[0][1] * fac_y,
            m.a[1][0] * fac_x,
            m.a[1][1] * fac_y,
            m.a[2][0] * fac_x,
            m.a[2][1] * fac_y,
        )
    }

    /// Scale around a fixed point.
    pub fn scale_around(fac_x: f64, fac_y: f64, fix_x: f64, fix_y: f64) -> Self {
        Self::new(
            fac_x,
            0.0,
            0.0,
            fac_y,
            fix_x - fix_x * fac_x,
            fix_y - fix_y * fac_y,
        )
    }

    /// Scale around a fixed point after `m`.
    pub fn scale_around_after(
        fac_x: f64,
        fac_y: f64,
        fix_x: f64,
        fix_y: f64,
        m: &AffineMatrix,
    ) -> Self {
        Self::new(
            m.a[0][0] * fac_x,
            m.a[0][1] * fac_y,
            m.a[1][0] * fac_x,
            m.a[1][1] * fac_y,
            (m.a[2][0] - fix_x) * fac_x + fix_x,
            (m.a[2][1] - fix_y) * fac_y + fix_y,
        )
    }

    /// Rotation matrix. Angle is in degrees.
    pub fn rotate(angle: f64) -> Self {
        let rad = angle * std::f64::consts::PI / 180.0;
        let c = rad.cos();
        let s = rad.sin();
        Self::new(c, s, -s, c, 0.0, 0.0)
    }

    /// Compose rotation after `m`.
    pub fn rotate_after(angle: f64, m: &AffineMatrix) -> Self {
        let rad = angle * std::f64::consts::PI / 180.0;
        let c = rad.cos();
        let s = rad.sin();
        Self::new(
            m.a[0][0] * c - m.a[0][1] * s,
            m.a[0][0] * s + m.a[0][1] * c,
            m.a[1][0] * c - m.a[1][1] * s,
            m.a[1][0] * s + m.a[1][1] * c,
            m.a[2][0] * c - m.a[2][1] * s,
            m.a[2][0] * s + m.a[2][1] * c,
        )
    }

    /// Rotation around a fixed point. Angle is in degrees.
    pub fn rotate_around(angle: f64, fix_x: f64, fix_y: f64) -> Self {
        let rad = angle * std::f64::consts::PI / 180.0;
        let c = rad.cos();
        let s = rad.sin();
        Self::new(
            c,
            s,
            -s,
            c,
            fix_x - fix_x * c + fix_y * s,
            fix_y - fix_x * s - fix_y * c,
        )
    }

    /// Rotation around a fixed point after `m`.
    pub fn rotate_around_after(angle: f64, fix_x: f64, fix_y: f64, m: &AffineMatrix) -> Self {
        let rad = angle * std::f64::consts::PI / 180.0;
        let c = rad.cos();
        let s = rad.sin();
        Self::new(
            m.a[0][0] * c - m.a[0][1] * s,
            m.a[0][0] * s + m.a[0][1] * c,
            m.a[1][0] * c - m.a[1][1] * s,
            m.a[1][0] * s + m.a[1][1] * c,
            (m.a[2][0] - fix_x) * c - (m.a[2][1] - fix_y) * s + fix_x,
            (m.a[2][0] - fix_x) * s + (m.a[2][1] - fix_y) * c + fix_y,
        )
    }

    /// Shear matrix: tx = sx + sy*sh_x, ty = sy + sx*sh_y.
    pub fn shear(sh_x: f64, sh_y: f64) -> Self {
        Self::new(1.0, sh_y, sh_x, 1.0, 0.0, 0.0)
    }

    /// Compose shear after `m`.
    pub fn shear_after(sh_x: f64, sh_y: f64, m: &AffineMatrix) -> Self {
        Self::new(
            m.a[0][1] * sh_x + m.a[0][0],
            m.a[0][0] * sh_y + m.a[0][1],
            m.a[1][1] * sh_x + m.a[1][0],
            m.a[1][0] * sh_y + m.a[1][1],
            m.a[2][1] * sh_x + m.a[2][0],
            m.a[2][0] * sh_y + m.a[2][1],
        )
    }

    /// Shear around a fixed point.
    pub fn shear_around(sh_x: f64, sh_y: f64, fix_x: f64, fix_y: f64) -> Self {
        Self::new(1.0, sh_y, sh_x, 1.0, -fix_y * sh_x, -fix_x * sh_y)
    }

    /// Shear around a fixed point after `m`.
    pub fn shear_around_after(
        sh_x: f64,
        sh_y: f64,
        fix_x: f64,
        fix_y: f64,
        m: &AffineMatrix,
    ) -> Self {
        Self::new(
            m.a[0][1] * sh_x + m.a[0][0],
            m.a[0][0] * sh_y + m.a[0][1],
            m.a[1][1] * sh_x + m.a[1][0],
            m.a[1][0] * sh_y + m.a[1][1],
            (m.a[2][1] - fix_y) * sh_x + m.a[2][0],
            (m.a[2][0] - fix_x) * sh_y + m.a[2][1],
        )
    }

    /// Compute the inverse matrix. Returns `None` if the matrix is singular.
    pub fn inverse(&self) -> Option<Self> {
        let det = self.a[0][0] * self.a[1][1] - self.a[0][1] * self.a[1][0];
        if det == 0.0 {
            return None;
        }
        let p = 1.0 / det;
        let n = -p;
        Some(Self::new(
            self.a[1][1] * p,
            self.a[0][1] * n,
            self.a[1][0] * n,
            self.a[0][0] * p,
            (self.a[1][0] * self.a[2][1] - self.a[1][1] * self.a[2][0]) * p,
            (self.a[0][0] * self.a[2][1] - self.a[0][1] * self.a[2][0]) * n,
        ))
    }

    /// Multiply two matrices. Transforming with the result is equivalent to
    /// transforming first with `self` and then with `other`.
    pub fn multiply(&self, other: &AffineMatrix) -> Self {
        Self::new(
            self.a[0][0] * other.a[0][0] + self.a[0][1] * other.a[1][0],
            self.a[0][0] * other.a[0][1] + self.a[0][1] * other.a[1][1],
            self.a[1][0] * other.a[0][0] + self.a[1][1] * other.a[1][0],
            self.a[1][0] * other.a[0][1] + self.a[1][1] * other.a[1][1],
            self.a[2][0] * other.a[0][0] + self.a[2][1] * other.a[1][0] + other.a[2][0],
            self.a[2][0] * other.a[0][1] + self.a[2][1] * other.a[1][1] + other.a[2][1],
        )
    }

    /// Multiply three matrices: multiply(multiply(m1, m2), m3).
    pub fn multiply3(m1: &AffineMatrix, m2: &AffineMatrix, m3: &AffineMatrix) -> Self {
        let a00 = m1.a[0][0] * m2.a[0][0] + m1.a[0][1] * m2.a[1][0];
        let a01 = m1.a[0][0] * m2.a[0][1] + m1.a[0][1] * m2.a[1][1];
        let a10 = m1.a[1][0] * m2.a[0][0] + m1.a[1][1] * m2.a[1][0];
        let a11 = m1.a[1][0] * m2.a[0][1] + m1.a[1][1] * m2.a[1][1];
        let a20 = m1.a[2][0] * m2.a[0][0] + m1.a[2][1] * m2.a[1][0] + m2.a[2][0];
        let a21 = m1.a[2][0] * m2.a[0][1] + m1.a[2][1] * m2.a[1][1] + m2.a[2][1];
        Self::new(
            a00 * m3.a[0][0] + a01 * m3.a[1][0],
            a00 * m3.a[0][1] + a01 * m3.a[1][1],
            a10 * m3.a[0][0] + a11 * m3.a[1][0],
            a10 * m3.a[0][1] + a11 * m3.a[1][1],
            a20 * m3.a[0][0] + a21 * m3.a[1][0] + m3.a[2][0],
            a20 * m3.a[0][1] + a21 * m3.a[1][1] + m3.a[2][1],
        )
    }

    /// Multiply four matrices: multiply(multiply3(m1, m2, m3), m4).
    pub fn multiply4(
        m1: &AffineMatrix,
        m2: &AffineMatrix,
        m3: &AffineMatrix,
        m4: &AffineMatrix,
    ) -> Self {
        let a00 = m1.a[0][0] * m2.a[0][0] + m1.a[0][1] * m2.a[1][0];
        let a01 = m1.a[0][0] * m2.a[0][1] + m1.a[0][1] * m2.a[1][1];
        let b00 = a00 * m3.a[0][0] + a01 * m3.a[1][0];
        let b01 = a00 * m3.a[0][1] + a01 * m3.a[1][1];

        let a10 = m1.a[1][0] * m2.a[0][0] + m1.a[1][1] * m2.a[1][0];
        let a11 = m1.a[1][0] * m2.a[0][1] + m1.a[1][1] * m2.a[1][1];
        let b10 = a10 * m3.a[0][0] + a11 * m3.a[1][0];
        let b11 = a10 * m3.a[0][1] + a11 * m3.a[1][1];

        let a20 = m1.a[2][0] * m2.a[0][0] + m1.a[2][1] * m2.a[1][0] + m2.a[2][0];
        let a21 = m1.a[2][0] * m2.a[0][1] + m1.a[2][1] * m2.a[1][1] + m2.a[2][1];
        let b20 = a20 * m3.a[0][0] + a21 * m3.a[1][0] + m3.a[2][0];
        let b21 = a20 * m3.a[0][1] + a21 * m3.a[1][1] + m3.a[2][1];

        Self::new(
            b00 * m4.a[0][0] + b01 * m4.a[1][0],
            b00 * m4.a[0][1] + b01 * m4.a[1][1],
            b10 * m4.a[0][0] + b11 * m4.a[1][0],
            b10 * m4.a[0][1] + b11 * m4.a[1][1],
            b20 * m4.a[0][0] + b21 * m4.a[1][0] + m4.a[2][0],
            b20 * m4.a[0][1] + b21 * m4.a[1][1] + m4.a[2][1],
        )
    }

    // DIVERGED: (language-forced) TransX, TransY — also available as individual methods below;
    // tuple-returning method added for idiomatic Rust use.
    /// Transform source coordinates to target coordinates.
    pub fn transform_point(&self, sx: f64, sy: f64) -> (f64, f64) {
        (
            self.a[0][0] * sx + self.a[1][0] * sy + self.a[2][0],
            self.a[0][1] * sx + self.a[1][1] * sy + self.a[2][1],
        )
    }

    /// Transform source X coordinate. C++ `emATMatrix::TransX`.
    pub fn TransX(&self, sx: f64, sy: f64) -> f64 {
        self.a[0][0] * sx + self.a[1][0] * sy + self.a[2][0]
    }

    /// Transform source Y coordinate. C++ `emATMatrix::TransY`.
    pub fn TransY(&self, sx: f64, sy: f64) -> f64 {
        self.a[0][1] * sx + self.a[1][1] * sy + self.a[2][1]
    }

    // DIVERGED: (language-forced) InverseTransX, InverseTransY — also available as individual methods below;
    // tuple-returning method added for idiomatic Rust use.
    /// Transform target coordinates back to source coordinates.
    ///
    /// For inverse-transforming more than about 4 points, use `inverse()`
    /// and `transform_point` instead.
    pub fn inverse_transform_point(&self, tx: f64, ty: f64) -> Option<(f64, f64)> {
        let det = self.a[0][0] * self.a[1][1] - self.a[0][1] * self.a[1][0];
        if det == 0.0 {
            return None;
        }
        Some((
            (self.a[1][1] * tx - self.a[1][0] * ty + self.a[1][0] * self.a[2][1]
                - self.a[1][1] * self.a[2][0])
                / det,
            (self.a[0][0] * ty - self.a[0][1] * tx + self.a[0][1] * self.a[2][0]
                - self.a[0][0] * self.a[2][1])
                / det,
        ))
    }

    /// Inverse-transform target X coordinate. C++ `emATMatrix::InverseTransX`.
    pub fn InverseTransX(&self, tx: f64, ty: f64) -> Option<f64> {
        self.inverse_transform_point(tx, ty).map(|(sx, _)| sx)
    }

    /// Inverse-transform target Y coordinate. C++ `emATMatrix::InverseTransY`.
    pub fn InverseTransY(&self, tx: f64, ty: f64) -> Option<f64> {
        self.inverse_transform_point(tx, ty).map(|(_, sy)| sy)
    }

    /// Get an element. Row index i is 0..3, column index j is 0..2.
    pub fn Get(&self, i: usize, j: usize) -> f64 {
        self.a[i][j]
    }

    /// Set an element. Row index i is 0..3, column index j is 0..2.
    pub fn Set(&mut self, i: usize, j: usize, val: f64) {
        self.a[i][j] = val;
    }

    /// Multiply in place: `*self = self.multiply(other)`.
    pub fn multiply_assign(&mut self, other: &AffineMatrix) {
        let a00 = self.a[0][0];
        self.a[0][0] = a00 * other.a[0][0] + self.a[0][1] * other.a[1][0];
        self.a[0][1] = a00 * other.a[0][1] + self.a[0][1] * other.a[1][1];
        let a10 = self.a[1][0];
        self.a[1][0] = a10 * other.a[0][0] + self.a[1][1] * other.a[1][0];
        self.a[1][1] = a10 * other.a[0][1] + self.a[1][1] * other.a[1][1];
        let a20 = self.a[2][0];
        self.a[2][0] = a20 * other.a[0][0] + self.a[2][1] * other.a[1][0] + other.a[2][0];
        self.a[2][1] = a20 * other.a[0][1] + self.a[2][1] * other.a[1][1] + other.a[2][1];
    }
}

impl Mul for AffineMatrix {
    type Output = AffineMatrix;

    fn mul(self, rhs: AffineMatrix) -> AffineMatrix {
        self.multiply(&rhs)
    }
}

impl Mul for &AffineMatrix {
    type Output = AffineMatrix;

    fn mul(self, rhs: &AffineMatrix) -> AffineMatrix {
        self.multiply(rhs)
    }
}

impl Default for AffineMatrix {
    fn default() -> Self {
        Self::identity()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f64 = 1e-10;

    fn approx_eq(a: f64, b: f64) -> bool {
        (a - b).abs() < EPSILON
    }

    fn matrices_approx_eq(a: &AffineMatrix, b: &AffineMatrix) -> bool {
        for i in 0..3 {
            for j in 0..2 {
                if !approx_eq(a.Get(i, j), b.Get(i, j)) {
                    return false;
                }
            }
        }
        true
    }

    #[test]
    fn identity_preserves_point() {
        let m = AffineMatrix::identity();
        let (tx, ty) = m.transform_point(3.0, 7.0);
        assert!(approx_eq(tx, 3.0));
        assert!(approx_eq(ty, 7.0));
    }

    #[test]
    fn translate_moves_point() {
        let m = AffineMatrix::translate(5.0, -2.0);
        let (tx, ty) = m.transform_point(1.0, 1.0);
        assert!(approx_eq(tx, 6.0));
        assert!(approx_eq(ty, -1.0));
    }

    #[test]
    fn scale_multiplies() {
        let m = AffineMatrix::scale(2.0, 3.0);
        let (tx, ty) = m.transform_point(4.0, 5.0);
        assert!(approx_eq(tx, 8.0));
        assert!(approx_eq(ty, 15.0));
    }

    #[test]
    fn rotate_90_degrees() {
        let m = AffineMatrix::rotate(90.0);
        let (tx, ty) = m.transform_point(1.0, 0.0);
        assert!(approx_eq(tx, 0.0));
        assert!(approx_eq(ty, 1.0));
    }

    #[test]
    fn shear_applies() {
        let m = AffineMatrix::shear(0.5, 0.0);
        let (tx, ty) = m.transform_point(2.0, 4.0);
        // tx = 2 + 4*0.5 = 4
        assert!(approx_eq(tx, 4.0));
        assert!(approx_eq(ty, 4.0));
    }

    #[test]
    fn inverse_round_trip() {
        let m = AffineMatrix::new(2.0, 1.0, 0.5, 3.0, 4.0, -1.0);
        let inv = m.inverse().expect("non-singular");
        let result = m.multiply(&inv);
        assert!(matrices_approx_eq(&result, &AffineMatrix::identity()));
    }

    #[test]
    fn singular_returns_none() {
        let m = AffineMatrix::new(1.0, 2.0, 2.0, 4.0, 0.0, 0.0);
        assert!(m.inverse().is_none());
    }

    #[test]
    fn multiply_matches_sequential_transform() {
        let m1 = AffineMatrix::rotate(45.0);
        let m2 = AffineMatrix::translate(10.0, 0.0);
        let combined = m1.multiply(&m2);

        let (ix, iy) = m1.transform_point(3.0, 7.0);
        let (expected_x, expected_y) = m2.transform_point(ix, iy);
        let (actual_x, actual_y) = combined.transform_point(3.0, 7.0);

        assert!(approx_eq(actual_x, expected_x));
        assert!(approx_eq(actual_y, expected_y));
    }

    #[test]
    fn mul_operator_matches_multiply() {
        let m1 = AffineMatrix::scale(2.0, 3.0);
        let m2 = AffineMatrix::translate(1.0, 1.0);
        let via_method = m1.multiply(&m2);
        let via_op = m1 * m2;
        assert!(matrices_approx_eq(&via_method, &via_op));
    }

    #[test]
    fn inverse_transform_point_round_trip() {
        let m = AffineMatrix::new(2.0, 1.0, 0.5, 3.0, 4.0, -1.0);
        let (tx, ty) = m.transform_point(5.0, 6.0);
        let (sx, sy) = m.inverse_transform_point(tx, ty).expect("non-singular");
        assert!(approx_eq(sx, 5.0));
        assert!(approx_eq(sy, 6.0));
    }

    #[test]
    fn translate_after_matches_multiply() {
        let m = AffineMatrix::rotate(30.0);
        let via_compose = AffineMatrix::translate_after(5.0, 7.0, &m);
        let via_multiply = m.multiply(&AffineMatrix::translate(5.0, 7.0));
        assert!(matrices_approx_eq(&via_compose, &via_multiply));
    }

    #[test]
    fn scale_after_matches_multiply() {
        let m = AffineMatrix::rotate(30.0);
        let via_compose = AffineMatrix::scale_after(2.0, 3.0, &m);
        let via_multiply = m.multiply(&AffineMatrix::scale(2.0, 3.0));
        assert!(matrices_approx_eq(&via_compose, &via_multiply));
    }

    #[test]
    fn rotate_after_matches_multiply() {
        let m = AffineMatrix::scale(2.0, 3.0);
        let via_compose = AffineMatrix::rotate_after(45.0, &m);
        let via_multiply = m.multiply(&AffineMatrix::rotate(45.0));
        assert!(matrices_approx_eq(&via_compose, &via_multiply));
    }

    #[test]
    fn shear_after_matches_multiply() {
        let m = AffineMatrix::translate(1.0, 2.0);
        let via_compose = AffineMatrix::shear_after(0.5, 0.3, &m);
        let via_multiply = m.multiply(&AffineMatrix::shear(0.5, 0.3));
        assert!(matrices_approx_eq(&via_compose, &via_multiply));
    }

    #[test]
    fn scale_around_matches_translate_scale_translate() {
        let via_compose = AffineMatrix::scale_around(2.0, 3.0, 5.0, 7.0);
        let t1 = AffineMatrix::translate(-5.0, -7.0);
        let s = AffineMatrix::scale(2.0, 3.0);
        let t2 = AffineMatrix::translate(5.0, 7.0);
        let via_multiply = AffineMatrix::multiply3(&t1, &s, &t2);
        assert!(matrices_approx_eq(&via_compose, &via_multiply));
    }

    #[test]
    fn rotate_around_matches_translate_rotate_translate() {
        let via_compose = AffineMatrix::rotate_around(45.0, 3.0, 4.0);
        let t1 = AffineMatrix::translate(-3.0, -4.0);
        let r = AffineMatrix::rotate(45.0);
        let t2 = AffineMatrix::translate(3.0, 4.0);
        let via_multiply = AffineMatrix::multiply3(&t1, &r, &t2);
        assert!(matrices_approx_eq(&via_compose, &via_multiply));
    }

    #[test]
    fn multiply3_matches_sequential() {
        let m1 = AffineMatrix::rotate(30.0);
        let m2 = AffineMatrix::scale(2.0, 3.0);
        let m3 = AffineMatrix::translate(1.0, 2.0);
        let via_3 = AffineMatrix::multiply3(&m1, &m2, &m3);
        let via_seq = m1.multiply(&m2).multiply(&m3);
        assert!(matrices_approx_eq(&via_3, &via_seq));
    }

    #[test]
    fn multiply4_matches_sequential() {
        let m1 = AffineMatrix::rotate(30.0);
        let m2 = AffineMatrix::scale(2.0, 3.0);
        let m3 = AffineMatrix::translate(1.0, 2.0);
        let m4 = AffineMatrix::shear(0.1, 0.2);
        let via_4 = AffineMatrix::multiply4(&m1, &m2, &m3, &m4);
        let via_seq = m1.multiply(&m2).multiply(&m3).multiply(&m4);
        assert!(matrices_approx_eq(&via_4, &via_seq));
    }

    #[test]
    fn cpp_chain_example() {
        // The C++ example: emTranslateATM(5,7, emScaleATM(2,3, emRotateATM(90)))
        // means rotate first, then scale, then translate.
        let via_chain = AffineMatrix::translate_after(
            5.0,
            7.0,
            &AffineMatrix::scale_after(2.0, 3.0, &AffineMatrix::rotate(90.0)),
        );
        // Same as: rotate(90) * scale(2,3) * translate(5,7)
        let via_mul = AffineMatrix::rotate(90.0)
            .multiply(&AffineMatrix::scale(2.0, 3.0))
            .multiply(&AffineMatrix::translate(5.0, 7.0));
        assert!(matrices_approx_eq(&via_chain, &via_mul));
    }

    #[test]
    fn multiply_assign_matches_multiply() {
        let m1 = AffineMatrix::rotate(30.0);
        let m2 = AffineMatrix::scale(2.0, 3.0);
        let expected = m1.multiply(&m2);
        let mut actual = m1;
        actual.multiply_assign(&m2);
        assert!(matrices_approx_eq(&actual, &expected));
    }

    #[test]
    fn test_individual_trans_methods() {
        // Identity + translation (10, 20)
        let m = AffineMatrix::translate(10.0, 20.0);
        let (tx, ty) = m.transform_point(5.0, 7.0);
        assert_eq!(m.TransX(5.0, 7.0), tx);
        assert_eq!(m.TransY(5.0, 7.0), ty);
    }

    #[test]
    fn test_individual_inverse_trans_methods() {
        let m = AffineMatrix::translate(10.0, 20.0);
        let (ix, iy) = m.inverse_transform_point(15.0, 27.0).unwrap();
        assert_eq!(m.InverseTransX(15.0, 27.0).unwrap(), ix);
        assert_eq!(m.InverseTransY(15.0, 27.0).unwrap(), iy);
    }
}
