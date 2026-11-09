//! `math.gamma` and `math.lgamma`, as `m_tgamma` and `m_lgamma` in
//! `Modules/mathmodule.c` of CPython compute them, with the Lanczos
//! approximation. CPython does not take them from the libm of the system,
//! which gives poor results on some platforms.
//!
//! At a pole, where C gives an infinity and CPython raises `ValueError`,
//! these functions give nan. The emitted code raises `ValueError` for a nan
//! from a number that is not nan, as `math_1` of CPython does.
//!
//! The notice of CPython is in `NOTICE`.

// The constants are as CPython writes them.
#![allow(clippy::excessive_precision)]

use crate::div::fmod;
use crate::log::log;
use crate::pow::{exp, pow};
use core::f64::consts::PI;
use libm::{copysign, cos, fabs, floor, round, sin};

const LOG_PI: f64 = 1.144729885849400174143427351353058711647;

const LANCZOS_G: f64 = 6.024680040776729583740234375;
const LANCZOS_G_MINUS_HALF: f64 = 5.524680040776729583740234375;
const LANCZOS_NUM: [f64; 13] = [
    23531376880.410759688572007674451636754734846804940,
    42919803642.649098768957899047001988850926355848959,
    35711959237.355668049440185451547166705960488635843,
    17921034426.037209699919755754458931112671403265390,
    6039542586.3520280050642916443072979210699388420708,
    1439720407.3117216736632230727949123939715485786772,
    248874557.86205415651146038641322942321632125127801,
    31426415.585400194380614231628318205362874684987640,
    2876370.6289353724412254090516208496135991145378768,
    186056.26539522349504029498971604569928220784236328,
    8071.6720023658162106380029022722506138218516325024,
    210.82427775157934587250973392071336271166969580291,
    2.5066282746310002701649081771338373386264310793408,
];
/// The coefficients of `x * (x + 1) * ... * (x + 11)`.
const LANCZOS_DEN: [f64; 13] = [
    0.0,
    39916800.0,
    120543840.0,
    150917976.0,
    105258076.0,
    45995730.0,
    13339535.0,
    2637558.0,
    357423.0,
    32670.0,
    1925.0,
    66.0,
    1.0,
];

/// `gamma(n)` of the integers from 1 to 23, which a float holds exactly.
const GAMMA_INTEGRAL: [f64; 23] = [
    1.0,
    1.0,
    2.0,
    6.0,
    24.0,
    120.0,
    720.0,
    5040.0,
    40320.0,
    362880.0,
    3628800.0,
    39916800.0,
    479001600.0,
    6227020800.0,
    87178291200.0,
    1307674368000.0,
    20922789888000.0,
    355687428096000.0,
    6402373705728000.0,
    121645100408832000.0,
    2432902008176640000.0,
    51090942171709440000.0,
    1124000727777607680000.0,
];

#[unsafe(no_mangle)]
pub extern "C" fn f64_gamma(x: f64) -> f64 {
    gamma(x)
}

#[unsafe(no_mangle)]
pub extern "C" fn f64_lgamma(x: f64) -> f64 {
    lgamma(x)
}

/// `sin(pi * x)` of a finite `x`, exact at the integers and accurate near
/// them.
fn sin_pi(x: f64) -> f64 {
    let y = fmod(fabs(x), 2.0);
    // `y` is in `0..2`, so the cast gives 0 to 4.
    let r = match round(2.0 * y) as i32 {
        0 => sin(PI * y),
        1 => cos(PI * (y - 0.5)),
        // `-sin(pi * (y - 1))` would give -0.0 at 1.
        2 => sin(PI * (1.0 - y)),
        3 => -cos(PI * (y - 1.5)),
        _ => sin(PI * (y - 2.0)),
    };
    copysign(1.0, x) * r
}

/// The Lanczos sum of a positive `x`, as a rational function.
fn lanczos_sum(x: f64) -> f64 {
    let (mut num, mut den) = (0.0, 0.0);
    let coefficients = LANCZOS_NUM.iter().zip(&LANCZOS_DEN);
    if x < 5.0 {
        for (n, d) in coefficients.rev() {
            num = num * x + n;
            den = den * x + d;
        }
    } else {
        // In powers of `1 / x`, which keeps a large `x` from overflowing.
        for (n, d) in coefficients {
            num = num / x + n;
            den = den / x + d;
        }
    }
    num / den
}

/// `math.gamma(x)`, or nan at a pole.
pub fn gamma(x: f64) -> f64 {
    if !x.is_finite() {
        return if x > 0.0 { x } else { f64::NAN };
    }
    if x == 0.0 {
        return f64::NAN;
    }
    if x == floor(x) {
        if x < 0.0 {
            return f64::NAN;
        }
        if x <= 23.0 {
            // `x` is an integer from 1 to 23.
            return GAMMA_INTEGRAL.get(x as usize - 1).copied().unwrap_or(x);
        }
    }
    let absx = fabs(x);
    if absx < 1e-20 {
        return 1.0 / x;
    }
    // The result overflows above 200, and goes to 0 below -200.
    if absx > 200.0 {
        return if x < 0.0 {
            0.0 / sin_pi(x)
        } else {
            f64::INFINITY
        };
    }
    let y = absx + LANCZOS_G_MINUS_HALF;
    // `z` is the error of `y` times `g / y`. The result times `1 + z`
    // corrects the error that `pow` and `exp` make larger.
    let z = if absx > LANCZOS_G_MINUS_HALF {
        (y - absx) - LANCZOS_G_MINUS_HALF
    } else {
        (y - LANCZOS_G_MINUS_HALF) - absx
    };
    let z = z * LANCZOS_G / y;
    let mut r;
    if x < 0.0 {
        r = -PI / sin_pi(absx) / absx * exp(y) / lanczos_sum(absx);
        r -= z * r;
        if absx < 140.0 {
            r /= pow(y, absx - 0.5);
        } else {
            let sqrtpow = pow(y, absx / 2.0 - 0.25);
            r /= sqrtpow;
            r /= sqrtpow;
        }
    } else {
        r = lanczos_sum(absx) / exp(y);
        r += z * r;
        if absx < 140.0 {
            r *= pow(y, absx - 0.5);
        } else {
            let sqrtpow = pow(y, absx / 2.0 - 0.25);
            r *= sqrtpow;
            r *= sqrtpow;
        }
    }
    r
}

/// `math.lgamma(x)`, the log of `|gamma(x)|`, or nan at a pole.
pub fn lgamma(x: f64) -> f64 {
    if !x.is_finite() {
        return fabs(x);
    }
    if x == floor(x) && x <= 2.0 {
        // `lgamma(1)` and `lgamma(2)` are 0.
        return if x <= 0.0 { f64::NAN } else { 0.0 };
    }
    let absx = fabs(x);
    if absx < 1e-20 {
        return -log(absx);
    }
    let mut r = log(lanczos_sum(absx)) - LANCZOS_G;
    r += (absx - 0.5) * (log(absx + LANCZOS_G - 0.5) - 1.0);
    if x < 0.0 {
        // The reflection formula.
        r = LOG_PI - log(fabs(sin_pi(absx))) - log(absx) - r;
    }
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gamma_of_integers_is_exact() {
        assert_eq!(gamma(1.0), 1.0);
        assert_eq!(gamma(5.0), 24.0);
        assert_eq!(gamma(23.0), 1124000727777607680000.0);
    }

    #[test]
    fn gamma_of_special_values() {
        assert!(gamma(0.0).is_nan());
        assert!(gamma(-0.0).is_nan());
        assert!(gamma(-3.0).is_nan());
        assert!(gamma(f64::NEG_INFINITY).is_nan());
        assert_eq!(gamma(f64::INFINITY), f64::INFINITY);
        assert_eq!(gamma(201.0), f64::INFINITY);
        assert_eq!(gamma(1e-310), f64::INFINITY);
        assert_eq!(gamma(-200.5), 0.0);
    }

    #[test]
    fn lgamma_of_special_values() {
        assert_eq!(lgamma(1.0), 0.0);
        assert_eq!(lgamma(2.0), 0.0);
        assert!(lgamma(0.0).is_nan());
        assert!(lgamma(-2.0).is_nan());
        assert_eq!(lgamma(f64::NEG_INFINITY), f64::INFINITY);
        assert_eq!(lgamma(1e306), f64::INFINITY);
    }
}
