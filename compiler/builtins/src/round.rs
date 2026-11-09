//! `round(x, n)` of a float, as `double_round` in `Objects/floatobject.c`
//! of CPython does it. CPython rounds the exact decimal value of `x` to `n`
//! digits after the point, with a tie going to the even digit, and reads
//! the digits back as the nearest float.
//!
//! The formatting and the `parse` of `core` leave a table in the `.wasm`,
//! so [`exact`] gives the digits here and [`nearest`] reads them back.

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
    // The rounded value is `k * 10^-n`.
    let mut k = Big::new(0);
    for &c in kept {
        k.mul(10);
        k.add(u32::from(c - b'0'));
    }
    if rounds_up(digits, keep) {
        k.add(1);
    }
    let rounded = if n > 0 {
        let mut den = Big::new(1);
        den.mul_pow5(n.unsigned_abs());
        nearest(k, den, -n)
    } else {
        k.mul_pow5(n.unsigned_abs());
        nearest(k, Big::new(1), -n)
    };
    rounded.copysign(x)
}

/// The float nearest to `num / den * 2^exp`, with a tie going to the even
/// one, and an infinity when the value is too large for a float.
fn nearest(mut num: Big, mut den: Big, exp: i32) -> f64 {
    if num.is_zero() {
        return 0.0;
    }
    // With `d` the difference of the bit lengths, `num / den` is at least
    // `2^(d - 1)` and less than `2^(d + 1)`. The shift makes the quotient
    // `q` 55 or 56 bits long, two or three more than the 53 of a float,
    // and `sticky` tells whether something is left below them.
    let shift = 55 - (num.bits() - den.bits());
    if shift > 0 {
        num.shl(shift.unsigned_abs());
    } else {
        den.shl(shift.unsigned_abs());
    }
    let exp = exp - shift;
    den.shl(55);
    let mut q = 0_u64;
    for i in (0..56).rev() {
        if !num.less_than(&den) {
            num.sub(&den);
            q |= 1 << i;
        }
        den.shr1();
    }
    let sticky = !num.is_zero();
    // The value is `q * 2^exp`, plus less than `2^exp` when `sticky`. Of
    // the bits of `q`, `drop` go, so the float keeps 53 bits, or fewer
    // when it is subnormal and its unit is `2^-1074`.
    let bits = 64 - q.leading_zeros().cast_signed();
    let drop = (bits - 53).max(-1074 - exp);
    let Ok(drop @ ..64) = u32::try_from(drop) else {
        return 0.0;
    };
    let mut m = q >> drop;
    let rest = q & ((1 << drop) - 1);
    let half = 1 << (drop - 1);
    if rest > half || (rest == half && (sticky || m & 1 == 1)) {
        m += 1;
    }
    let mut unit = exp + drop.cast_signed();
    if m == 1 << 53 {
        m >>= 1;
        unit += 1;
    }
    if m < 1 << 52 {
        // A subnormal, whose unit is `2^-1074`.
        return f64::from_bits(m);
    }
    let field = unit + 52 + 1023;
    if field >= 0x7ff {
        return f64::INFINITY;
    }
    f64::from_bits((u64::from(field.unsigned_abs()) << 52) | (m - (1 << 52)))
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
    // `m * 2^-k` is `m * 5^k / 10^k`.
    if e < 0 {
        big.mul_pow5(e.unsigned_abs());
    } else {
        big.shl(e.unsigned_abs());
    }
    let mut digits = Buffer::new();
    big.decimal(&mut digits);
    (digits, usize::try_from(-e).unwrap_or(0))
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

/// A natural number big enough for `2^53 * 5^1074`, which has 2548 bits.
struct Big {
    limbs: [u32; LIMBS],
    len: usize,
}

const LIMBS: usize = 84;

impl Big {
    fn new(m: u64) -> Self {
        let mut limbs = [0; LIMBS];
        let [low, high, ..] = &mut limbs;
        *low = m as u32;
        *high = (m >> 32) as u32;
        let mut big = Big { limbs, len: 2 };
        big.trim();
        big
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

    fn mul_pow5(&mut self, mut n: u32) {
        // `5^13` is the greatest power of 5 that fits in a `u32`.
        while n >= 13 {
            self.mul(1_220_703_125);
            n -= 13;
        }
        self.mul(5_u32.pow(n));
    }

    fn add(&mut self, k: u32) {
        let mut carry = u64::from(k);
        for limb in self.limbs.iter_mut() {
            if carry == 0 {
                break;
            }
            let v = u64::from(*limb) + carry;
            *limb = v as u32;
            carry = v >> 32;
        }
        self.trim();
    }

    fn is_zero(&self) -> bool {
        self.len == 0
    }

    fn bits(&self) -> i32 {
        let top = self
            .limbs
            .get(self.len.wrapping_sub(1))
            .copied()
            .unwrap_or(0);
        (32 * self.len as i32) - top.leading_zeros().cast_signed()
    }

    fn shl(&mut self, n: u32) {
        let (limbs, bits) = ((n / 32) as usize, n % 32);
        let mut shifted = [0; LIMBS];
        let mut carry = 0;
        for (i, &limb) in self.limbs.iter().enumerate() {
            if let Some(slot) = shifted.get_mut(i + limbs) {
                *slot = (limb << bits) | carry;
            }
            carry = if bits == 0 { 0 } else { limb >> (32 - bits) };
        }
        self.limbs = shifted;
        self.len = LIMBS;
        self.trim();
    }

    fn shr1(&mut self) {
        let mut carry = 0;
        for limb in self.limbs.iter_mut().take(self.len).rev() {
            let next = *limb << 31;
            *limb = (*limb >> 1) | carry;
            carry = next;
        }
        self.trim();
    }

    /// Returns `true` if `self` is less than `other`, `false` otherwise.
    fn less_than(&self, other: &Big) -> bool {
        if self.len != other.len {
            return self.len < other.len;
        }
        for (a, b) in self.limbs.iter().zip(&other.limbs).take(self.len).rev() {
            if a != b {
                return a < b;
            }
        }
        false
    }

    /// Subtracts `other`, which is at most `self`.
    fn sub(&mut self, other: &Big) {
        let mut borrow = 0;
        for (a, &b) in self.limbs.iter_mut().zip(&other.limbs).take(self.len) {
            let v = i64::from(*a) - i64::from(b) - borrow;
            borrow = i64::from(v < 0);
            *a = v as u32;
        }
        self.trim();
    }

    /// Counts the limbs up to the last one that is not 0.
    fn trim(&mut self) {
        self.len = LIMBS;
        while self.len > 0 && self.limbs.get(self.len - 1) == Some(&0) {
            self.len -= 1;
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

    /// Writes the decimal digits, with no zero in front, and leaves the
    /// number at 0.
    fn decimal<const N: usize>(&mut self, out: &mut Buffer<N>) {
        let mut groups = [0; DIGITS / 9 + 1];
        let mut count = 0;
        while self.len > 0 {
            let group = self.div(1_000_000_000);
            if let Some(slot) = groups.get_mut(count) {
                *slot = group;
                count += 1;
            }
        }
        // The last group of 9 digits came out first.
        let mut groups = groups.iter().take(count).rev();
        if let Some(&first) = groups.next() {
            out.decimal(first);
        }
        for &group in groups {
            let mut power = 100_000_000;
            while power > 0 {
                out.push(b'0' + (group / power % 10) as u8);
                power /= 10;
            }
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
