//! Numeric builtins that the compiler links into the modules it emits.
//!
//! A builtin takes numbers, or bytes at an address in linear memory, and
//! returns numbers. It knows nothing of GC values, Python or exceptions. The
//! emitted code raises the exception when a builtin reports a failure.
//!
//! Every function here has to be free of panics. The test of the `.wasm`
//! rejects an `unreachable`, which is where a panic would go.

#![cfg_attr(target_arch = "wasm32", no_std)]

#[cfg(target_arch = "wasm32")]
#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    core::arch::wasm32::unreachable()
}

mod div;
mod math;
mod pow;
mod round;

pub use div::{floor_div, fmod, modulo};
pub use pow::pow;
pub use round::round;

/// The longest `repr` of a float, `-2.2250738585072014e-308`.
pub const REPR_MAX: usize = 24;

/// Writes `repr(f)` as CPython writes it to `out` and returns its length.
///
/// # Safety
///
/// `out` has to point to [`REPR_MAX`] writable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn f64_repr(f: f64, out: *mut [u8; REPR_MAX]) -> usize {
    repr(f, unsafe { &mut *out })
}

/// Writes `repr(f)` as CPython writes it to `out` and returns its length.
///
/// CPython takes the shortest digits that read back as `f`, the same digits
/// that ryu gives. It writes them in fixed notation when the exponent is in
/// `-4..16`, and in scientific notation otherwise, with a sign and at least
/// two digits in the exponent.
pub fn repr(f: f64, out: &mut [u8; REPR_MAX]) -> usize {
    let mut w = Writer { out, len: 0 };
    if f.is_nan() {
        w.str(b"nan");
    } else if f.is_infinite() {
        w.str(if f < 0.0 { b"-inf" } else { b"inf" });
    } else {
        let mut buffer = ryu::Buffer::new();
        let d = Decimal::parse(buffer.format_finite(f).as_bytes());
        d.write(&mut w);
    }
    w.len
}

/// A finite float as `0.d1d2...dn * 10^point`, with no zero at either end of
/// the digits. Zero has no digits.
struct Decimal {
    negative: bool,
    digits: [u8; 17],
    len: usize,
    point: i32,
}

impl Decimal {
    /// Reads what `ryu::Buffer::format_finite` writes, such as `-1.5e-7`,
    /// `0.001` or `100.0`.
    fn parse(s: &[u8]) -> Decimal {
        let mut d = Decimal {
            negative: false,
            digits: [0; 17],
            len: 0,
            point: 0,
        };
        let mut before_point = true;
        // Zeros after a significant digit, kept back until a nonzero digit
        // shows that they are not at the end.
        let mut zeros = 0;
        let mut exp_sign = 1;
        let mut exp = 0;
        let mut in_exp = false;
        for &c in s {
            match c {
                b'-' if in_exp => exp_sign = -1,
                b'-' => d.negative = true,
                b'.' => before_point = false,
                b'e' => in_exp = true,
                b'0'..=b'9' if in_exp => exp = exp * 10 + i32::from(c - b'0'),
                _ => {
                    if before_point {
                        d.point += 1;
                    }
                    if c == b'0' {
                        if d.len == 0 {
                            d.point -= 1;
                        } else {
                            zeros += 1;
                        }
                    } else {
                        for _ in 0..zeros {
                            d.push(b'0');
                        }
                        zeros = 0;
                        d.push(c);
                    }
                }
            }
        }
        d.point += exp_sign * exp;
        d
    }

    fn push(&mut self, c: u8) {
        if let Some(slot) = self.digits.get_mut(self.len) {
            *slot = c;
            self.len += 1;
        }
    }

    fn write(&self, w: &mut Writer) {
        if self.negative {
            w.byte(b'-');
        }
        let digits = self.digits.iter().take(self.len).copied();
        if self.len == 0 {
            w.str(b"0.0");
            return;
        }
        let e = self.point - 1;
        if (-4..16).contains(&e) {
            if self.point <= 0 {
                w.str(b"0.");
                w.zeros(-self.point);
                digits.for_each(|c| w.byte(c));
            } else {
                let mut left = self.point;
                for c in digits {
                    if left == 0 {
                        w.byte(b'.');
                    }
                    w.byte(c);
                    left -= 1;
                }
                if left >= 0 {
                    w.zeros(left);
                    w.str(b".0");
                }
            }
        } else {
            for (i, c) in digits.enumerate() {
                if i == 1 {
                    w.byte(b'.');
                }
                w.byte(c);
            }
            w.str(if e < 0 { b"e-" } else { b"e+" });
            let e = e.unsigned_abs();
            if e >= 100 {
                w.digit(e / 100);
            }
            w.digit(e / 10 % 10);
            w.digit(e % 10);
        }
    }
}

struct Writer<'a> {
    out: &'a mut [u8; REPR_MAX],
    len: usize,
}

impl Writer<'_> {
    fn byte(&mut self, c: u8) {
        if let Some(slot) = self.out.get_mut(self.len) {
            *slot = c;
            self.len += 1;
        }
    }

    fn str(&mut self, s: &[u8]) {
        for &c in s {
            self.byte(c);
        }
    }

    fn zeros(&mut self, n: i32) {
        for _ in 0..n {
            self.byte(b'0');
        }
    }

    fn digit(&mut self, d: u32) {
        // `d` is below 10, so the cast keeps it.
        self.byte(b'0' + d as u8);
    }
}
