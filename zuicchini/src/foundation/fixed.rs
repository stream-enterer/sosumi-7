/// Fixed-point number with 12 fractional bits (4096 sub-pixel grid).
///
/// Used for sub-pixel anti-aliased rasterization. The 12-bit fractional
/// part gives 1/4096 precision, sufficient for high-quality AA coverage.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Fixed12(i32);

const FRAC_BITS: i32 = 12;
const SCALE: i32 = 1 << FRAC_BITS; // 4096
const FRAC_MASK: i32 = SCALE - 1; // 0xFFF

impl Fixed12 {
    pub const ZERO: Fixed12 = Fixed12(0);
    pub const ONE: Fixed12 = Fixed12(SCALE);

    #[inline]
    pub fn from_raw(raw: i32) -> Self {
        Self(raw)
    }

    #[inline]
    pub fn raw(self) -> i32 {
        self.0
    }

    #[inline]
    pub fn from_f64(v: f64) -> Self {
        Self((v * SCALE as f64) as i32)
    }

    #[inline]
    pub fn from_i32(v: i32) -> Self {
        Self(v << FRAC_BITS)
    }

    #[inline]
    pub fn to_f64(self) -> f64 {
        self.0 as f64 / SCALE as f64
    }

    /// Integer part (truncates toward negative infinity).
    #[inline]
    pub fn to_i32(self) -> i32 {
        self.0 >> FRAC_BITS
    }

    /// Fractional part (lower 12 bits), always in [0, 4095].
    #[inline]
    pub fn frac(self) -> i32 {
        self.0 & FRAC_MASK
    }

    #[inline]
    pub fn floor(self) -> Self {
        Self(self.0 & !FRAC_MASK)
    }

    #[inline]
    pub fn ceil(self) -> Self {
        Self((self.0 + FRAC_MASK) & !FRAC_MASK)
    }

    #[inline]
    pub fn round(self) -> Self {
        Self((self.0 + (SCALE >> 1)) & !FRAC_MASK)
    }
}

impl std::ops::Add for Fixed12 {
    type Output = Self;
    #[inline]
    fn add(self, rhs: Self) -> Self {
        Self(self.0 + rhs.0)
    }
}

impl std::ops::AddAssign for Fixed12 {
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        self.0 += rhs.0;
    }
}

impl std::ops::Sub for Fixed12 {
    type Output = Self;
    #[inline]
    fn sub(self, rhs: Self) -> Self {
        Self(self.0 - rhs.0)
    }
}

impl std::ops::SubAssign for Fixed12 {
    #[inline]
    fn sub_assign(&mut self, rhs: Self) {
        self.0 -= rhs.0;
    }
}

impl std::ops::Mul for Fixed12 {
    type Output = Self;
    #[inline]
    fn mul(self, rhs: Self) -> Self {
        // Use i64 intermediate to prevent overflow for values > 1024.
        Self(((self.0 as i64 * rhs.0 as i64) >> FRAC_BITS) as i32)
    }
}

impl std::ops::Neg for Fixed12 {
    type Output = Self;
    #[inline]
    fn neg(self) -> Self {
        Self(-self.0)
    }
}

impl std::fmt::Display for Fixed12 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:.4}", self.to_f64())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_f64() {
        let v = Fixed12::from_f64(3.75);
        assert!((v.to_f64() - 3.75).abs() < 1e-6);
    }

    #[test]
    fn round_trip_i32() {
        let v = Fixed12::from_i32(42);
        assert_eq!(v.to_i32(), 42);
        assert_eq!(v.frac(), 0);
    }

    #[test]
    fn frac_bits() {
        let v = Fixed12::from_f64(1.5);
        assert_eq!(v.to_i32(), 1);
        assert_eq!(v.frac(), 2048); // 0.5 * 4096
    }

    #[test]
    fn arithmetic() {
        let a = Fixed12::from_f64(2.5);
        let b = Fixed12::from_f64(1.25);
        assert!((a + b).to_f64() - 3.75 < 1e-6);
        assert!((a - b).to_f64() - 1.25 < 1e-6);
        assert!(((a * b).to_f64() - 3.125).abs() < 1e-3);
    }

    #[test]
    fn negation() {
        let v = Fixed12::from_f64(3.5);
        assert!(((-v).to_f64() + 3.5).abs() < 1e-6);
    }

    #[test]
    fn floor_ceil_round() {
        let v = Fixed12::from_f64(2.7);
        assert_eq!(v.floor().to_i32(), 2);
        assert_eq!(v.ceil().to_i32(), 3);
        assert_eq!(v.round().to_i32(), 3);

        let v2 = Fixed12::from_f64(2.3);
        assert_eq!(v2.floor().to_i32(), 2);
        assert_eq!(v2.ceil().to_i32(), 3);
        assert_eq!(v2.round().to_i32(), 2);
    }

    #[test]
    fn negative_values() {
        let v = Fixed12::from_f64(-1.75);
        assert_eq!(v.to_i32(), -2); // Right-shift floors toward negative infinity
        assert!((v.to_f64() + 1.75).abs() < 1e-6);
    }

    #[test]
    fn overflow_mul_uses_i64() {
        // Values > 1024 would overflow i32 without i64 intermediate.
        let a = Fixed12::from_f64(2000.0);
        let b = Fixed12::from_f64(3.0);
        assert!(((a * b).to_f64() - 6000.0).abs() < 1.0);
    }

    #[test]
    fn constants() {
        assert_eq!(Fixed12::ZERO.to_i32(), 0);
        assert_eq!(Fixed12::ZERO.frac(), 0);
        assert_eq!(Fixed12::ONE.to_i32(), 1);
        assert_eq!(Fixed12::ONE.frac(), 0);
    }
}
