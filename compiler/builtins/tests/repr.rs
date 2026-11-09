use std::io::Write;
use std::process::{Command, Stdio};

use builtins::{REPR_MAX, repr};

/// Compares `repr` with CPython on special values, every power of 2 and of
/// 10, random bits, subnormals, integers and short decimals.
#[test]
fn repr_matches_cpython() {
    let floats = floats();
    let input: String = floats
        .iter()
        .map(|f| format!("{:x}\n", f.to_bits()))
        .collect();
    let mut python = Command::new(std::env::var("PYTHON").unwrap_or_else(|_| "python3".into()))
        .args([
            "-c",
            "import sys, struct\n\
             for line in sys.stdin:\n    \
                 print(repr(struct.unpack('<d', struct.pack('<Q', int(line, 16)))[0]))",
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
    for &f in &floats {
        let mut out = [0; REPR_MAX];
        let len = repr(f, &mut out);
        let got = std::str::from_utf8(out.get(..len).expect("len fits")).expect("ASCII");
        let want = lines.next().expect("one line per float");
        if got != want {
            mismatches.push(format!("{:x}: {got} != {want}", f.to_bits()));
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

fn floats() -> Vec<f64> {
    let mut floats = vec![
        0.0,
        -0.0,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NAN,
        -f64::NAN,
        f64::MIN_POSITIVE,
        f64::MAX,
        f64::MIN,
        f64::EPSILON,
        5e-324,
        -2.2250738585072014e-308,
        -2.9205048131065683e-196,
        1e16,
        1e15,
        9999999999999998.0,
        1e-4,
        1e-5,
        0.1 + 0.2,
        1.5,
        -2.5e-7,
        123456789012345678.0,
    ];
    floats.extend((-1074..1024).map(|e| 2f64.powi(e)));
    floats.extend((-324..309).map(|e| format!("1e{e}").parse::<f64>().expect("a float")));
    let mut random = SplitMix64(1);
    for _ in 0..200_000 {
        floats.push(f64::from_bits(random.next()));
    }
    for _ in 0..50_000 {
        floats.push(f64::from_bits(random.next() >> 12));
    }
    for _ in 0..50_000 {
        let digits = random.next() % 20;
        floats.push((random.next() % 10u64.pow(digits as u32)) as f64);
    }
    for _ in 0..50_000 {
        let digits = random.next() % 17 + 1;
        let mantissa = random.next() % 10u64.pow(digits as u32);
        let exp = (random.next() % 61) as i32 - 30;
        floats.push(format!("{mantissa}e{exp}").parse::<f64>().expect("a float"));
    }
    floats
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
