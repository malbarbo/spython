use std::io::Write;
use std::process::{Command, Stdio};

use builtins::{REPR_MAX, repr, round};

/// Compares `round(x, n)` with CPython on special values, short decimals
/// with their ties, integers, subnormals, values near the largest float
/// and random bits, each with numbers of digits around where it rounds.
/// Where CPython raises `OverflowError`, the result has to be the infinity
/// that the emitted code turns into the error.
#[test]
fn round_matches_cpython() {
    let cases = cases();
    let input: String = cases
        .iter()
        .map(|(x, n)| format!("{:x} {n}\n", x.to_bits()))
        .collect();
    let mut python = Command::new(std::env::var("PYTHON").unwrap_or_else(|_| "python3".into()))
        .args([
            "-c",
            "import sys, struct\n\
             f = lambda h: struct.unpack('<d', struct.pack('<Q', int(h, 16)))[0]\n\
             for line in sys.stdin:\n    \
                 x, n = line.split()\n    \
                 try:\n        \
                     print(repr(round(f(x), int(n))))\n    \
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
    for &(x, n) in &cases {
        let got = outcome(x, n);
        let want = lines.next().expect("one line per case");
        if got != want {
            mismatches.push(format!("round({x:?}, {n}): {got} != {want}"));
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

/// What the emitted code gives for `round(x, n)`. An infinite result of a
/// finite value is an error.
fn outcome(x: f64, n: i32) -> String {
    let r = round(x, n);
    if x.is_finite() && r.is_infinite() {
        "OverflowError".into()
    } else {
        let mut out = [0; REPR_MAX];
        let len = repr(r, &mut out);
        String::from_utf8_lossy(out.get(..len).expect("len fits")).into_owned()
    }
}

fn cases() -> Vec<(f64, i32)> {
    let specials = [
        0.0,
        f64::INFINITY,
        f64::NAN,
        5e-324,
        1e-323,
        2.5e-323,
        1e-310,
        f64::MIN_POSITIVE,
        f64::MAX,
        1.7976931348623157e308 / 2.0,
        9.5e307,
        f64::EPSILON,
        0.5,
        1.5,
        2.5,
        2.675,
        0.125,
        0.375,
        1e15 + 0.5,
        4503599627370495.5,
        9007199254740993.0,
        1e22,
        5e22,
        1e300,
    ];
    let digits = [
        -400, -309, -308, -307, -305, -300, -23, -22, -16, -3, -2, -1, 0, 1, 2, 3, 15, 16, 17, 300,
        307, 308, 309, 320, 322, 323, 324, 400,
    ];
    let mut cases: Vec<(f64, i32)> = specials
        .iter()
        .flat_map(|&x| [x, -x])
        .flat_map(|x| digits.iter().map(move |&n| (x, n)))
        .collect();
    let mut random = SplitMix64(1);
    for _ in 0..4000 {
        // Short decimals and their ties, as in `round(2.675, 2)`.
        let places = (random.next() % 6) as i32;
        let x = format!("{}e-{places}", random.next() % 10_000_000)
            .parse::<f64>()
            .expect("a float");
        let n = places - 1 - (random.next() % 3) as i32;
        cases.push((x, n));
        cases.push((-x, n));
        // Ties of `int` values, as in `round(25.0, -1)`.
        let x = (random.next() % 2_000_000) as f64 - 1e6;
        cases.push((x, -((random.next() % 7) as i32)));
        // Random bits, with digits near the first one.
        let x = f64::from_bits(random.next());
        let exp = if x.is_finite() && x != 0.0 {
            x.abs().log10().floor() as i32
        } else {
            0
        };
        let n = -exp + (random.next() % 40) as i32 - 20;
        cases.push((x, n));
        // Subnormals and tiny normals.
        let x = f64::from_bits(random.next() % (1 << 54));
        cases.push((x, 300 + (random.next() % 26) as i32));
        // Near the largest float.
        let x = f64::from_bits(0x7fef_ffff_ffff_ffff - random.next() % (1 << 48));
        cases.push((x, -300 - (random.next() % 10) as i32));
    }
    cases
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
