//! `//` and `%` on floats, ported from `_float_div_mod` and `float_rem` in
//! `Objects/floatobject.c` of CPython. The emitted code raises
//! `ZeroDivisionError` before it calls them, so the divisor is never 0.
//!
//! The `%` of Rust on wasm32 calls the `fmod` of `compiler_builtins`, which
//! brings a path to a panic, so [`fmod`] here is a port of the one in musl.

#[unsafe(no_mangle)]
pub extern "C" fn f64_floor_div(a: f64, b: f64) -> f64 {
    floor_div(a, b)
}

#[unsafe(no_mangle)]
pub extern "C" fn f64_mod(a: f64, b: f64) -> f64 {
    modulo(a, b)
}

#[unsafe(no_mangle)]
pub extern "C" fn f64_fmod(x: f64, y: f64) -> f64 {
    fmod(x, y)
}

/// The `fmod` of C, ported from `src/math/fmod.c` of musl. It is exact,
/// and gives the remainder of `x` by `y` with the sign of `x`.
pub fn fmod(x: f64, y: f64) -> f64 {
    const BITS: u64 = 52;
    let mut ux = x.to_bits();
    let mut uy = y.to_bits();
    let mut ex = ((ux >> BITS) & 0x7ff) as i32;
    let mut ey = ((uy >> BITS) & 0x7ff) as i32;
    let sx = ux >> 63;

    // A divisor of 0 or nan, or an infinite or nan dividend, has no
    // remainder.
    if uy << 1 == 0 || y.is_nan() || ex == 0x7ff {
        return f64::NAN;
    }
    if ux << 1 <= uy << 1 {
        if ux << 1 == uy << 1 {
            return 0.0 * x;
        }
        return x;
    }

    // Normalize the mantissas, with the bit at 52 set.
    if ex == 0 {
        let mut i = ux << 12;
        while i >> 63 == 0 {
            ex -= 1;
            i <<= 1;
        }
        ux = ux.wrapping_shl((1 - ex) as u32);
    } else {
        ux &= u64::MAX >> 12;
        ux |= 1 << BITS;
    }
    if ey == 0 {
        let mut i = uy << 12;
        while i >> 63 == 0 {
            ey -= 1;
            i <<= 1;
        }
        uy = uy.wrapping_shl((1 - ey) as u32);
    } else {
        uy &= u64::MAX >> 12;
        uy |= 1 << BITS;
    }

    // Subtract the shifted divisor, one exponent at a time.
    while ex > ey {
        let i = ux.wrapping_sub(uy);
        if i >> 63 == 0 {
            if i == 0 {
                return 0.0 * x;
            }
            ux = i;
        }
        ux <<= 1;
        ex -= 1;
    }
    let i = ux.wrapping_sub(uy);
    if i >> 63 == 0 {
        if i == 0 {
            return 0.0 * x;
        }
        ux = i;
    }
    while ux >> BITS == 0 {
        ux <<= 1;
        ex -= 1;
    }

    // Scale the result back.
    if ex > 0 {
        ux -= 1 << BITS;
        ux |= (ex as u64) << BITS;
    } else {
        ux = ux.wrapping_shr((1 - ex) as u32);
    }
    f64::from_bits(ux | sx << 63)
}

/// The `floor` of the C library, which `core` lacks. A float of 2^52 or
/// more has no fraction, and a smaller one fits in an `i64`.
fn floor(x: f64) -> f64 {
    if x.is_nan() || x.abs() >= 4503599627370496.0 || x == 0.0 {
        return x;
    }
    let t = x as i64 as f64;
    if t > x { t - 1.0 } else { t }
}

/// `a // b`, the quotient of `a - a % b` by `b`, snapped to the nearest
/// integral value.
pub fn floor_div(a: f64, b: f64) -> f64 {
    let m = fmod(a, b);
    let mut div = (a - m) / b;
    // The remainder takes the sign of the divisor.
    if m != 0.0 && (b < 0.0) != (m < 0.0) {
        div -= 1.0;
    }
    if div != 0.0 {
        let floored = floor(div);
        if div - floored > 0.5 {
            floored + 1.0
        } else {
            floored
        }
    } else {
        0.0f64.copysign(a / b)
    }
}

/// `a % b`, which has the sign of `b`, also when it is 0.
pub fn modulo(a: f64, b: f64) -> f64 {
    let m = fmod(a, b);
    if m == 0.0 {
        0.0f64.copysign(b)
    } else if (b < 0.0) != (m < 0.0) {
        m + b
    } else {
        m
    }
}
