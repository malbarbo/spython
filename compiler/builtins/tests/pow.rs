use std::io::Write;
use std::process::{Command, Stdio};

use builtins::{REPR_MAX, pow, repr};

/// Compares `**` with CPython on pairs of special values, of small
/// integers, of short decimals, of numbers near 1 and of random bits, with
/// every combination of signs. Where CPython raises, or gives a `complex`,
/// the result has to be the one that the emitted code turns into an error.
#[test]
fn pow_matches_cpython() {
    let pairs = pairs();
    let input: String = pairs
        .iter()
        .map(|(x, y)| format!("{:x} {:x}\n", x.to_bits(), y.to_bits()))
        .collect();
    let mut python = Command::new(std::env::var("PYTHON").unwrap_or_else(|_| "python3".into()))
        .args([
            "-c",
            "import math, sys, struct\n\
             f = lambda h: struct.unpack('<d', struct.pack('<Q', int(h, 16)))[0]\n\
             for line in sys.stdin:\n    \
                 x, y = map(f, line.split())\n    \
                 if -math.inf < x < 0 and math.isfinite(y) and y != math.floor(y):\n        \
                     print('complex')\n        \
                     continue\n    \
                 try:\n        \
                     print(repr(x ** y))\n    \
                 except Exception as e:\n        \
                     print(type(e).__name__)",
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
    for &(x, y) in &pairs {
        let got = outcome(x, y);
        let want = lines.next().expect("one line per pair");
        if got != want {
            mismatches.push(format!("{x:?} ** {y:?}: {got} != {want}"));
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

/// What the emitted code gives for `x ** y`. With finite operands, an
/// infinite result is an error, and so is nan from operands that are not
/// nan.
fn outcome(x: f64, y: f64) -> String {
    let r = pow(x, y);
    if x.is_finite() && y.is_finite() && r.is_infinite() {
        if x == 0.0 {
            "ZeroDivisionError".into()
        } else {
            "OverflowError".into()
        }
    } else if r.is_nan() && !x.is_nan() && !y.is_nan() {
        "complex".into()
    } else {
        show(r)
    }
}

fn show(f: f64) -> String {
    let mut out = [0; REPR_MAX];
    let len = repr(f, &mut out);
    String::from_utf8_lossy(out.get(..len).expect("len fits")).into_owned()
}

fn pairs() -> Vec<(f64, f64)> {
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
        1e300,
        0.1,
        0.5,
        1.0,
        1.0 + f64::EPSILON,
        1.0 - f64::EPSILON / 2.0,
        1.5,
        2.0,
        3.0,
        7.5,
        10.0,
        1023.0,
        1024.0,
        1074.0,
        1075.0,
        1e16,
        9007199254740993.0,
        1e19,
        1e300,
    ];
    let mut values: Vec<f64> = specials.iter().flat_map(|&x| [x, -x]).collect();
    let mut random = SplitMix64(1);
    for _ in 0..40 {
        let mantissa = random.next() % 100_000;
        let exp = (random.next() % 13) as i32 - 6;
        let x = format!("{mantissa}e{exp}").parse::<f64>().expect("a float");
        values.extend([x, -x]);
    }
    let mut pairs: Vec<(f64, f64)> = values
        .iter()
        .flat_map(|&x| values.iter().map(move |&y| (x, y)))
        .collect();
    for _ in 0..40_000 {
        // Small integers, as in `3.0 ** 5`.
        let x = (random.next() % 2001) as f64 - 1000.0;
        let y = (random.next() % 121) as f64 - 60.0;
        pairs.push((x, y));
        // Short decimals.
        let x = (random.next() % 1_000_000) as f64 / 1000.0;
        let y = (random.next() % 200_000) as f64 / 1000.0 - 100.0;
        pairs.push((x, y));
        // Near 1, with large exponents.
        let x = 1.0 + ((random.next() % 2_000_000) as f64 - 1e6) * 1e-12;
        let y = (random.next() % 2_000_000_000_000) as f64 - 1e12;
        pairs.push((x, y));
        // Random bits of a positive base, and an exponent that keeps the
        // result in range most of the time.
        let x = f64::from_bits(random.next() >> 1);
        let y = (random.next() % 4_000_000) as f64 / 1e6 - 2.0;
        pairs.push((x, y));
        pairs.push((f64::from_bits(random.next()), f64::from_bits(random.next())));
    }
    pairs
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
}
