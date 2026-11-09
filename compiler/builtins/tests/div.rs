use std::io::Write;
use std::process::{Command, Stdio};

use builtins::{REPR_MAX, floor_div, fmod, modulo, repr};

/// Compares `//` and `%` with CPython on pairs of special values, of small
/// numbers, of short decimals and of random bits, with every combination of
/// signs.
#[test]
fn floor_div_and_modulo_match_cpython() {
    let pairs = pairs();
    let input: String = pairs
        .iter()
        .map(|(a, b)| format!("{:x} {:x}\n", a.to_bits(), b.to_bits()))
        .collect();
    let mut python = Command::new(std::env::var("PYTHON").unwrap_or_else(|_| "python3".into()))
        .args([
            "-c",
            "import sys, struct\n\
             f = lambda h: struct.unpack('<d', struct.pack('<Q', int(h, 16)))[0]\n\
             for line in sys.stdin:\n    \
                 a, b = map(f, line.split())\n    \
                 print(repr(a // b), repr(a % b))",
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
    for &(a, b) in &pairs {
        let got = format!("{} {}", show(floor_div(a, b)), show(modulo(a, b)));
        let want = lines.next().expect("one line per pair");
        if got != want {
            mismatches.push(format!("{a:?} {b:?}: {got} != {want}"));
        }
    }
    assert_eq!(lines.next(), None);
    assert!(
        mismatches.is_empty(),
        "{}",
        mismatches
            .iter()
            .take(20)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// Compares the port of `fmod` with the `%` of Rust, which calls the `fmod`
/// of the platform, also on a divisor of 0.
#[test]
fn fmod_matches_the_platform() {
    let mut pairs = pairs();
    pairs.extend([(1.0, 0.0), (-1.0, -0.0), (f64::NAN, 0.0), (0.0, 0.0)]);
    for (a, b) in pairs {
        let (got, want) = (fmod(a, b), a % b);
        assert!(
            got.to_bits() == want.to_bits() || (got.is_nan() && want.is_nan()),
            "fmod({a:?}, {b:?}) = {got:?}, not {want:?}"
        );
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
        f64::MIN_POSITIVE,
        f64::MAX,
        f64::EPSILON,
        1e-300,
        1e300,
        0.1,
        0.5,
        1.0,
        1.5,
        2.0,
        3.0,
        7.5,
        10.0,
        1e16,
        9007199254740993.0,
        1073741823.0,
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
        .flat_map(|&a| values.iter().map(move |&b| (a, b)))
        .collect();
    for _ in 0..100_000 {
        pairs.push((f64::from_bits(random.next()), f64::from_bits(random.next())));
    }
    pairs.retain(|&(_, b)| b != 0.0);
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
