//! `round(x, n)` of a float, as `double_round` in `Objects/floatobject.c`
//! of CPython does it. CPython rounds the exact decimal value of `x` to `n`
//! digits after the point, with a tie going to the even digit, and reads
//! the digits back as the nearest float.
//!
//! The formatting and the `parse` of `core` leave a table in the `.wasm`,
//! and the bignum crates leave paths to a panic, so [`exact`] gives the
//! digits here and `fast-float2` reads them back.

/// Above this many digits, every float is already round. CPython calls it
/// `NDIGITS_MAX`.
const DIGITS_MAX: i32 = 323;
/// Below this many digits, every float rounds to 0. CPython calls it
/// `NDIGITS_MIN`.
const DIGITS_MIN: i32 = -308;
/// The most digits of the exact value of a float, which `m * 5^1074` of
/// the least normal float has. A float of at least 1 has at most 309.
const DIGITS: usize = 767;

#[unsafe(no_mangle)]
pub extern "C" fn f64_round(x: f64, n: i32) -> f64 {
    round(x, n)
}

/// `round(x, n)` as CPython gives it, or an infinity when the rounded value
/// is too large for a float, where CPython raises `OverflowError`.
pub fn round(x: f64, n: i32) -> f64 {
    if !x.is_finite() || x == 0.0 || n > DIGITS_MAX {
        return x;
    }
    if n < DIGITS_MIN {
        return 0.0 * x;
    }
    let (digits, fraction) = exact(x);
    let digits = digits.bytes();
    // How many of the digits stay. The value is `digits * 10^-fraction`,
    // and the last digit that stays is the unit of `10^-n`.
    let keep = digits.len().cast_signed() - fraction.cast_signed() + n as isize;
    let Ok(keep) = usize::try_from(keep) else {
        // The value is below a tenth of `10^-n`, so it rounds to 0.
        return 0.0 * x;
    };
    let Some(kept) = digits.get(..keep) else {
        return x;
    };
    let mut text = Buffer::<{ DIGITS + 8 }>::new();
    let up = rounds_up(digits, keep);
    // A spot in front for a carry out of the first digit, or for a 0 when
    // no digit stays.
    text.push(b'0');
    for &c in kept {
        text.push(c);
    }
    if up {
        carry(text.bytes_mut());
    }
    text.push(b'e');
    if n > 0 {
        text.push(b'-');
    }
    text.decimal(n.unsigned_abs());
    let rounded: f64 = fast_float2::parse(text.bytes()).unwrap_or(x);
    rounded.copysign(x)
}

/// The exact value of `|x|`, as its decimal digits, with no zero in front,
/// and how many of them come after the point.
fn exact(x: f64) -> (Buffer<DIGITS>, usize) {
    let bits = x.to_bits();
    let field = ((bits >> 52) & 0x7ff) as i32;
    let fraction = bits & ((1 << 52) - 1);
    // `|x|` is `m * 2^e`.
    let (m, e) = if field == 0 {
        (fraction, -1074)
    } else {
        (fraction | (1 << 52), field - 1075)
    };
    let mut big = Big::new(m);
    if e < 0 {
        // `m * 2^-k` is `m * 5^k / 10^k`. `5^13` is the greatest power of
        // 5 that fits in a `u32`.
        let k = e.unsigned_abs();
        for _ in 0..k / 13 {
            big.mul(1_220_703_125);
        }
        big.mul(5_u32.pow(k % 13));
    } else {
        // `2^31` fits in a `u32`.
        let k = e.unsigned_abs();
        for _ in 0..k / 31 {
            big.mul(1 << 31);
        }
        big.mul(1 << (k % 31));
    }
    // Groups of 9 digits, the last one first.
    let mut groups = [0; DIGITS / 9 + 1];
    let mut count = 0;
    while !big.is_zero() {
        let group = big.div(1_000_000_000);
        if let Some(slot) = groups.get_mut(count) {
            *slot = group;
            count += 1;
        }
    }
    let mut digits = Buffer::new();
    let mut groups = groups.iter().take(count).rev();
    if let Some(&first) = groups.next() {
        digits.decimal(first);
    }
    for &group in groups {
        let mut power = 100_000_000;
        while power > 0 {
            digits.push(b'0' + (group / power % 10) as u8);
            power /= 10;
        }
    }
    (digits, usize::try_from(-e).unwrap_or(0))
}

/// A natural number big enough for `2^53 * 5^1074`, which has 2548 bits,
/// with the least significant limb first.
struct Big {
    limbs: [u32; 80],
    len: usize,
}

impl Big {
    fn new(m: u64) -> Self {
        let mut big = Big {
            limbs: [0; 80],
            len: 0,
        };
        // The casts take the low and the high half.
        for half in [m as u32, (m >> 32) as u32] {
            if let Some(limb) = big.limbs.get_mut(big.len) {
                *limb = half;
                big.len += 1;
            }
        }
        big.trim();
        big
    }

    fn is_zero(&self) -> bool {
        self.len == 0
    }

    fn mul(&mut self, k: u32) {
        let mut carry = 0;
        for limb in self.limbs.iter_mut().take(self.len) {
            let v = u64::from(*limb) * u64::from(k) + carry;
            *limb = v as u32;
            carry = v >> 32;
        }
        if carry != 0
            && let Some(limb) = self.limbs.get_mut(self.len)
        {
            *limb = carry as u32;
            self.len += 1;
        }
    }

    /// Divides by `d` and returns the remainder.
    fn div(&mut self, d: u32) -> u32 {
        let mut rem = 0;
        for limb in self.limbs.iter_mut().take(self.len).rev() {
            let v = (rem << 32) | u64::from(*limb);
            *limb = (v / u64::from(d)) as u32;
            rem = v % u64::from(d);
        }
        self.trim();
        rem as u32
    }

    /// Drops the limbs of 0 at the top.
    fn trim(&mut self) {
        while self.len > 0 && self.limbs.get(self.len - 1) == Some(&0) {
            self.len -= 1;
        }
    }
}

/// Returns `true` if the digits after the first `keep` are more than half
/// of a unit of the last kept digit, or exactly half and the last kept
/// digit is odd, `false` otherwise.
fn rounds_up(digits: &[u8], keep: usize) -> bool {
    let Some((&first, rest)) = digits.get(keep..).and_then(<[u8]>::split_first) else {
        return false;
    };
    match first {
        b'6'..=b'9' => true,
        b'5' if rest.iter().any(|&c| c != b'0') => true,
        b'5' => keep
            .checked_sub(1)
            .and_then(|last| digits.get(last))
            .is_some_and(|&c| (c - b'0') % 2 == 1),
        _ => false,
    }
}

/// Adds 1 to the last of `digits`, which has a 0 in front for the carry.
fn carry(digits: &mut [u8]) {
    for c in digits.iter_mut().rev() {
        if *c == b'9' {
            *c = b'0';
        } else {
            *c += 1;
            return;
        }
    }
}

struct Buffer<const N: usize> {
    bytes: [u8; N],
    len: usize,
}

impl<const N: usize> Buffer<N> {
    fn new() -> Self {
        Buffer {
            bytes: [0; N],
            len: 0,
        }
    }

    fn push(&mut self, c: u8) {
        if let Some(slot) = self.bytes.get_mut(self.len) {
            *slot = c;
            self.len += 1;
        }
    }

    fn bytes(&self) -> &[u8] {
        self.bytes.get(..self.len).unwrap_or_default()
    }

    fn bytes_mut(&mut self) -> &mut [u8] {
        self.bytes.get_mut(..self.len).unwrap_or_default()
    }

    /// Writes `v` in decimal, with no zero in front.
    fn decimal(&mut self, v: u32) {
        let mut power = 1;
        while power <= v / 10 {
            power *= 10;
        }
        while power > 0 {
            self.push(b'0' + (v / power % 10) as u8);
            power /= 10;
        }
    }
}
