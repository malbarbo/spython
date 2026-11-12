//! The `hash` of CPython 3.13 for the values that need 64 bits.
//!
//! The notice of CPython is in `NOTICE`.

/// The number of bits of the modulus of the hash of a number.
const BITS: u32 = 61;
/// The hash of a number is its value modulo this prime, `2^61 - 1`.
const MODULUS: u64 = (1 << BITS) - 1;
/// The hash of the positive infinity.
const INF: i64 = 314159;

#[unsafe(no_mangle)]
pub extern "C" fn f64_hash(x: f64) -> i64 {
    float(x)
}

/// The hash of `x`, as `_Py_HashDouble` of `Python/pyhash.c` gives it. A
/// float with an integer value has the hash of the `int`. CPython takes the
/// hash of a NaN from the address of the object, and a NaN here has 0.
pub fn float(x: f64) -> i64 {
    if x.is_infinite() {
        return if x > 0.0 { INF } else { -INF };
    }
    if x.is_nan() {
        return 0;
    }
    let (mut m, mut e) = frexp(x);
    let negative = m < 0.0;
    if negative {
        m = -m;
    }
    // 28 bits at a time, as CPython takes them.
    let mut h: u64 = 0;
    while m != 0.0 {
        h = ((h << 28) & MODULUS) | h >> (BITS - 28);
        m *= 268435456.0;
        e -= 28;
        // `m` is below 2^28, so the cast keeps its integer part.
        let y = m as u64;
        m -= y as f64;
        h += y;
        if h >= MODULUS {
            h -= MODULUS;
        }
    }
    let e = e.rem_euclid(BITS as i32) as u32;
    h = ((h << e) & MODULUS) | h >> (BITS - e);
    let h = if negative { h.wrapping_neg() } else { h } as i64;
    if h == -1 { -2 } else { h }
}

/// `x` as `m * 2^e`, with `m` in `[0.5, 1)` or 0, for a finite `x`. The
/// `frexp` of the `libm` crate calls itself for a subnormal, and a builtin
/// does not recurse.
fn frexp(x: f64) -> (f64, i32) {
    if x == 0.0 {
        return (x, 0);
    }
    // A subnormal times 2^64 is normal.
    let (x, shift) = if x.abs() < f64::MIN_POSITIVE {
        (x * f64::from_bits(0x43f0_0000_0000_0000), 64)
    } else {
        (x, 0)
    };
    let bits = x.to_bits();
    let exp = ((bits >> 52) & 0x7ff) as i32;
    let m = f64::from_bits((bits & !(0x7ff << 52)) | (0x3fe << 52));
    (m, exp - 0x3fe - shift)
}
