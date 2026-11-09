//! The functions of `math` that CPython takes from the libm of the system,
//! from the `libm` crate. They can differ from the libm of glibc in the last
//! bit.

use crate::div::fmod;

macro_rules! unary {
    ($($export:ident = $func:ident;)*) => {
        $(
            #[unsafe(no_mangle)]
            pub extern "C" fn $export(x: f64) -> f64 {
                libm::$func(x)
            }
        )*
    };
}

unary! {
    f64_expm1 = expm1;
    f64_log2 = log2;
    f64_log10 = log10;
    f64_log1p = log1p;
    f64_sin = sin;
    f64_cos = cos;
    f64_tan = tan;
    f64_asin = asin;
    f64_acos = acos;
    f64_atan = atan;
    f64_sinh = sinh;
    f64_cosh = cosh;
    f64_asinh = asinh;
    f64_acosh = acosh;
    f64_atanh = atanh;
    f64_cbrt = cbrt;
    f64_erf = erf;
    f64_erfc = erfc;
}

#[unsafe(no_mangle)]
pub extern "C" fn f64_nextafter(x: f64, y: f64) -> f64 {
    libm::nextafter(x, y)
}

/// `math.ulp(x)`, as `math_ulp` of CPython computes it.
#[unsafe(no_mangle)]
pub extern "C" fn f64_ulp(x: f64) -> f64 {
    if x.is_nan() {
        return x;
    }
    let x = libm::fabs(x);
    if x.is_infinite() {
        return x;
    }
    let next = libm::nextafter(x, f64::INFINITY);
    if next.is_infinite() {
        // The greatest float has no next one, so its ulp is the gap below.
        return x - libm::nextafter(x, -f64::INFINITY);
    }
    next - x
}

#[unsafe(no_mangle)]
pub extern "C" fn f64_ldexp(x: f64, i: i32) -> f64 {
    libm::ldexp(x, i)
}

/// `math.remainder(x, y)`, as `m_remainder` of CPython computes it. The
/// result is exact, and a tie goes to the even multiple of `y`. A `y` of 0
/// and an infinite `x` give nan.
#[unsafe(no_mangle)]
pub extern "C" fn f64_remainder(x: f64, y: f64) -> f64 {
    if x.is_finite() && y.is_finite() {
        if y == 0.0 {
            return f64::NAN;
        }
        let (absx, absy) = (libm::fabs(x), libm::fabs(y));
        let m = fmod(absx, absy);
        let c = absy - m;
        let r = if m < c {
            m
        } else if m > c {
            -c
        } else {
            // A tie. `absx - m` is an exact multiple of `absy`, and the
            // `fmod` tells whether half of it is an odd multiple.
            m - 2.0 * fmod(0.5 * (absx - m), absy)
        };
        return libm::copysign(1.0, x) * r;
    }
    if x.is_nan() {
        return x;
    }
    if y.is_nan() {
        return y;
    }
    if x.is_infinite() {
        return f64::NAN;
    }
    x
}

#[unsafe(no_mangle)]
pub extern "C" fn f64_fma(x: f64, y: f64, z: f64) -> f64 {
    libm::fma(x, y, z)
}

#[unsafe(no_mangle)]
pub extern "C" fn f64_atan2(y: f64, x: f64) -> f64 {
    libm::atan2(y, x)
}
