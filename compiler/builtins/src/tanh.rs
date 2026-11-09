//! `math.tanh`, ported from `src/s_tanh.c` of the msun of FreeBSD, which
//! comes from the fdlibm of Sun. It gives the digits of glibc more often
//! than the `tanh` of the `libm` crate, which comes from musl.
//!
//! Copyright (C) 1993 by Sun Microsystems, Inc. All rights reserved.
//!
//! Developed at SunPro, a Sun Microsystems, Inc. business.
//! Permission to use, copy, modify, and distribute this
//! software is freely granted, provided that this notice
//! is preserved.

use libm::{expm1, fabs};

#[unsafe(no_mangle)]
pub extern "C" fn f64_tanh(x: f64) -> f64 {
    tanh(x)
}

/// The hyperbolic tangent of `x`.
pub fn tanh(x: f64) -> f64 {
    let jx = (x.to_bits() >> 32) as u32 as i32;
    let ix = jx & 0x7fffffff;
    if ix >= 0x7ff00000 {
        // 1/inf + 1 is 1 and a nan stays a nan.
        return if jx >= 0 {
            1.0 / x + 1.0
        } else {
            1.0 / x - 1.0
        };
    }
    let z = if ix < 0x40360000 {
        // msun returns x below 2^-28, and glibc only below 2^-55. The port
        // follows glibc, which gives other digits between the two.
        if ix < 0x3c800000 {
            return x;
        }
        if ix >= 0x3ff00000 {
            let t = expm1(2.0 * fabs(x));
            1.0 - 2.0 / (t + 2.0)
        } else {
            let t = expm1(-2.0 * fabs(x));
            -t / (t + 2.0)
        }
    } else {
        // |x| >= 22
        1.0
    };
    if jx >= 0 { z } else { -z }
}
