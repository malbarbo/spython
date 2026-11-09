//! `math.hypot`, as `math_hypot` and `vector_norm` in `Modules/mathmodule.c`
//! of CPython compute it. The squares and their sum keep twice the bits of
//! a float, so the result is almost always correctly rounded.

use libm::{fabs, fma, ldexp, sqrt};

#[unsafe(no_mangle)]
pub extern "C" fn f64_hypot(x: f64, y: f64) -> f64 {
    hypot(&mut [x, y])
}

/// `math.hypot` of `coordinates`, which it overwrites.
pub fn hypot(coordinates: &mut [f64]) -> f64 {
    let mut max = 0.0;
    let mut found_nan = false;
    for x in coordinates.iter_mut() {
        *x = fabs(*x);
        found_nan |= x.is_nan();
        if *x > max {
            max = *x;
        }
    }
    norm(coordinates, max, found_nan)
}

/// The norm of `vec`, whose values are not negative and at most `max`.
/// An infinity wins over a nan.
fn norm(vec: &mut [f64], max: f64, found_nan: bool) -> f64 {
    if max.is_infinite() {
        return max;
    }
    if found_nan {
        return f64::NAN;
    }
    if max == 0.0 || vec.len() <= 1 {
        return max;
    }
    let mut max_e = exponent(max);
    // The norm of `vec` is `unit` times the norm of the values in `vec`.
    let mut unit = 1.0;
    if max_e < -1023 {
        // `2^-max_e` would overflow, so the subnormals become normal first.
        for x in vec.iter_mut() {
            *x /= f64::MIN_POSITIVE;
        }
        unit = f64::MIN_POSITIVE;
        max_e = exponent(max / f64::MIN_POSITIVE);
    }
    // The scale puts `max` in `0.5..1`, and no bit is lost.
    let scale = ldexp(1.0, -max_e);
    // The sum starts at 1, above every square, so `fast_sum` holds. The 1
    // comes out at the end.
    let mut csum = 1.0;
    let mut frac1 = 0.0;
    let mut frac2 = 0.0;
    for &x in vec.iter() {
        let x = x * scale;
        let (hi, lo) = mul(x, x);
        let (sum, error) = fast_sum(csum, hi);
        csum = sum;
        frac1 += lo;
        frac2 += error;
    }
    let mut h = sqrt(csum - 1.0 + (frac1 + frac2));
    // One step of Newton corrects the square root of the rounded sum.
    let (hi, lo) = mul(-h, h);
    let (sum, error) = fast_sum(csum, hi);
    csum = sum;
    frac1 += lo;
    frac2 += error;
    let x = csum - 1.0 + (frac1 + frac2);
    h += x / (2.0 * h);
    unit * (h / scale)
}

/// The exponent `e` of a positive finite `x` in `0.5..1` times `2^e`, as
/// `frexp` gives it. `libm::frexp` calls itself for a subnormal.
fn exponent(x: f64) -> i32 {
    let bits = x.to_bits();
    // The sign bit is 0, so the cast keeps the field of the exponent.
    let field = (bits >> 52) as i32;
    if field == 0 {
        // `x` is `bits * 2^-1074`.
        64 - bits.leading_zeros() as i32 - 1074
    } else {
        field - 1022
    }
}

/// `a + b` and its rounding error, for `|a| >= |b|`.
fn fast_sum(a: f64, b: f64) -> (f64, f64) {
    let x = a + b;
    (x, (a - x) + b)
}

/// `x * y` and its rounding error.
fn mul(x: f64, y: f64) -> (f64, f64) {
    let z = x * y;
    (z, fma(x, y, -z))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hypot_follows_python() {
        assert_eq!(f64_hypot(3.0, 4.0), 5.0);
        assert_eq!(f64_hypot(-5.0, 12.0), 13.0);
        assert_eq!(f64_hypot(0.0, -0.0), 0.0);
        assert_eq!(f64_hypot(1e308, 1e308), 1.4142135623730951e308);
        assert_eq!(f64_hypot(5e-324, 5e-324), 5e-324);
        assert_eq!(f64_hypot(f64::NAN, f64::NEG_INFINITY), f64::INFINITY);
        assert!(f64_hypot(f64::NAN, 1.0).is_nan());
        assert_eq!(hypot(&mut [3.0, 4.0, 12.0]), 13.0);
        assert_eq!(hypot(&mut [-2.5]), 2.5);
        assert_eq!(hypot(&mut []), 0.0);
        assert_eq!(hypot(&mut [3e-310, 4e-310]), 5e-310);
    }

    #[test]
    fn exponent_follows_frexp() {
        for x in [1.0, 0.75, 3.0, 1e308, f64::MIN_POSITIVE, 5e-324, 3e-310] {
            assert_eq!(exponent(x), libm::frexp(x).1);
        }
    }
}
