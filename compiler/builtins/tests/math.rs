use std::io::Write;
use std::process::{Command, Stdio};

use builtins::{REPR_MAX, exp, exp2, log, repr, tanh};

/// Compares `math.exp` with CPython on special values, integers, short
/// decimals, values near 0 and random values in and out of range.
#[test]
fn exp_matches_cpython() {
    let mut random = SplitMix64(1);
    let mut values = specials();
    for _ in 0..200_000 {
        values.push((random.next() % 1501) as f64 - 750.0);
        values.push((random.next() % 2_000_000) as f64 / 1000.0 - 1000.0);
        values.push(((random.next() % 2_000_000) as f64 - 1e6) * 1e-12);
        values.push(random.uniform(-746.0, 710.0));
        values.push(f64::from_bits(random.next()));
    }
    compare("exp", exp, true, (0.0, 0), &values);
}

/// Compares `math.exp2` with CPython on special values, integers, short
/// decimals, values near 0 and random values in and out of range.
#[test]
fn exp2_matches_cpython() {
    let mut random = SplitMix64(3);
    let mut values = specials();
    for _ in 0..200_000 {
        values.push((random.next() % 2201) as f64 - 1100.0);
        values.push((random.next() % 2_200_000) as f64 / 1000.0 - 1100.0);
        values.push(((random.next() % 2_000_000) as f64 - 1e6) * 1e-12);
        values.push(random.uniform(-1080.0, 1030.0));
        values.push(f64::from_bits(random.next()));
    }
    compare("exp2", exp2, true, (0.0, 0), &values);
}

/// Compares `math.log` with CPython on special values, integers, short
/// decimals, values near 1 and random positive values.
#[test]
fn log_matches_cpython() {
    let mut random = SplitMix64(2);
    let mut values = specials();
    for _ in 0..200_000 {
        values.push((random.next() % 1_000_000) as f64);
        values.push((random.next() % 1_000_000) as f64 / 1000.0);
        values.push(1.0 + ((random.next() % 2_000_000) as f64 - 1e6) * 1e-7);
        values.push(random.uniform(0.9, 1.1));
        values.push(f64::from_bits(random.next() >> 1));
    }
    compare("log", log, false, (0.0, 0), &values);
}

/// Compares `math.tanh` with CPython on special values, short decimals,
/// values near 0 and 1 and random values in and out of the range where
/// `tanh` is not 1. Below 1 the port calls the `expm1` of the `libm` crate,
/// which differs from the `expm1` of glibc in the last bit, and `-t / (t + 2)`
/// makes that difference up to 3 ulps.
#[test]
fn tanh_matches_cpython() {
    let mut random = SplitMix64(4);
    let mut values = specials();
    for _ in 0..200_000 {
        values.push((random.next() % 50_000) as f64 / 1000.0 - 25.0);
        values.push(((random.next() % 2_000_000) as f64 - 1e6) * 1e-12);
        values.push(random.uniform(-1.1, 1.1));
        values.push(random.uniform(-23.0, 23.0));
        values.push(f64::from_bits(random.next()));
    }
    compare("tanh", tanh, false, (1.0, 3), &values);
}

/// Compares `f` with `math.<name>` of CPython at each of `values`. A
/// result that CPython gives as an error has to be the one that the
/// emitted code turns into that error, by the rule of `math_1` in CPython.
/// With `ulps` as `(bound, n)`, a result at an `x` with `|x| < bound` can
/// differ from CPython by up to `n` ulps.
fn compare(name: &str, f: fn(f64) -> f64, can_overflow: bool, ulps: (f64, u64), values: &[f64]) {
    let input: String = values
        .iter()
        .map(|x| format!("{:x}\n", x.to_bits()))
        .collect();
    let mut python = Command::new(std::env::var("PYTHON").unwrap_or_else(|_| "python3".into()))
        .args([
            "-c",
            &format!(
                "import math, sys, struct\n\
                 for line in sys.stdin:\n    \
                     x = struct.unpack('<d', struct.pack('<Q', int(line, 16)))[0]\n    \
                     try:\n        \
                         print(repr(math.{name}(x)))\n    \
                     except Exception as e:\n        \
                         print(type(e).__name__)"
            ),
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("python3 should run");
    let mut stdin = python.stdin.take().expect("stdin is piped");
    let writer = std::thread::spawn(move || stdin.write_all(input.as_bytes()));
    let output = python.wait_with_output().expect("python3 should finish");
    writer
        .join()
        .expect("the writer should not panic")
        .expect("python3 should read");
    let expected = String::from_utf8(output.stdout).expect("repr is ASCII");

    let mut lines = expected.lines();
    let mut mismatches = Vec::new();
    for &x in values {
        let r = f(x);
        let got = if r.is_nan() && !x.is_nan() {
            "ValueError".into()
        } else if r.is_infinite() && x.is_finite() {
            if can_overflow {
                "OverflowError"
            } else {
                "ValueError"
            }
            .into()
        } else {
            show(r)
        };
        let want = lines.next().expect("one line per value");
        if got != want && !(x.abs() < ulps.0 && ulps_apart(&got, want) <= ulps.1) {
            mismatches.push(format!("{name}({x:?}): {got} != {want}"));
        }
    }
    assert_eq!(lines.next(), None);
    assert!(
        mismatches.is_empty(),
        "{} mismatches:\n{}",
        mismatches.len(),
        mismatches
            .iter()
            .take(20)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// The distance in ulps between the floats with reprs `a` and `b`, or
/// `u64::MAX` if one of them is not a float.
fn ulps_apart(a: &str, b: &str) -> u64 {
    match (a.parse::<f64>(), b.parse::<f64>()) {
        (Ok(a), Ok(b)) if a.is_sign_negative() == b.is_sign_negative() => {
            a.to_bits().abs_diff(b.to_bits())
        }
        _ => u64::MAX,
    }
}

fn specials() -> Vec<f64> {
    let specials = [
        0.0,
        f64::INFINITY,
        f64::NAN,
        5e-324,
        1e-310,
        f64::MIN_POSITIVE,
        f64::MAX,
        f64::EPSILON,
        1e-300,
        0.1,
        0.5,
        1.0,
        1.0 + f64::EPSILON,
        1.0 - f64::EPSILON / 2.0,
        2.0,
        3.0,
        10.0,
        100.0,
        512.0,
        700.0,
        709.782712893384,
        709.7827128933841,
        708.3964185322641,
        745.1332191019411,
        745.1332191019412,
        1024.0,
        1e300,
    ];
    specials.iter().flat_map(|&x| [x, -x]).collect()
}

fn show(f: f64) -> String {
    let mut out = [0; REPR_MAX];
    let len = repr(f, &mut out);
    String::from_utf8_lossy(out.get(..len).expect("len fits")).into_owned()
}

struct SplitMix64(u64);

impl SplitMix64 {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e3779b97f4a7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        z ^ (z >> 31)
    }

    /// A float in `[low, high)`.
    fn uniform(&mut self, low: f64, high: f64) -> f64 {
        low + (self.next() >> 11) as f64 / (1u64 << 53) as f64 * (high - low)
    }
}
