use std::io::Write;
use std::process::{Command, Stdio};

/// The lines that CPython prints for `script`, which reads `input` from
/// the standard input. The key of the hash of a `str` is the one of
/// `PYTHONHASHSEED=0`.
fn cpython(script: &str, input: String) -> Vec<String> {
    let mut python = Command::new(std::env::var("PYTHON").unwrap_or_else(|_| "python3".into()))
        .args(["-c", script])
        .env("PYTHONHASHSEED", "0")
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
    assert!(output.status.success(), "python3 should succeed");
    String::from_utf8(output.stdout)
        .expect("digits are ASCII")
        .lines()
        .map(str::to_owned)
        .collect()
}

/// Asserts that `got` has the hashes of `want`, and shows the first ones
/// that differ.
fn assert_same(inputs: &[String], got: &[i64], want: &[String]) {
    assert_eq!(got.len(), want.len(), "one line per input");
    let mismatches: Vec<String> = inputs
        .iter()
        .zip(got.iter().zip(want))
        .filter(|(_, (got, want))| got.to_string() != **want)
        .take(20)
        .map(|(input, (got, want))| format!("{input}: {got} != {want}"))
        .collect();
    assert!(mismatches.is_empty(), "{}", mismatches.join("\n"));
}

/// Compares the hash of a float with CPython on special values, every
/// power of 2 and of 10, integers, short decimals and random bits.
#[test]
fn the_hash_of_a_float_matches_cpython() {
    let floats = floats();
    let inputs: Vec<String> = floats
        .iter()
        .map(|f| format!("{:x}", f.to_bits()))
        .collect();
    let want = cpython(
        "import sys, struct\n\
         for line in sys.stdin:\n    \
             print(hash(struct.unpack('<d', struct.pack('<Q', int(line, 16)))[0]))",
        inputs.iter().map(|line| format!("{line}\n")).collect(),
    );
    let got: Vec<i64> = floats.iter().map(|&f| builtins::hash::float(f)).collect();
    assert_same(&inputs, &got, &want);
}

/// CPython takes the hash of a NaN from the address of the object.
#[test]
fn the_hash_of_a_nan_is_0() {
    assert_eq!(builtins::hash::float(f64::NAN), 0);
    assert_eq!(builtins::hash::float(-f64::NAN), 0);
}

fn floats() -> Vec<f64> {
    let mut floats = vec![
        0.0,
        -0.0,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::MIN_POSITIVE,
        f64::MAX,
        f64::MIN,
        f64::EPSILON,
        5e-324,
        -1.0,
        -2.0,
        0.5,
        1.5,
        -2.5e-7,
        0.1 + 0.2,
        2305843009213693951.0,
        2305843009213693952.0,
        -2305843009213693952.0,
        123456789012345678.0,
    ];
    floats.extend((-1074..1024).map(|e| 2f64.powi(e)));
    floats.extend((-1074..1024).map(|e| -(2f64.powi(e))));
    floats.extend((-324..309).map(|e| format!("1e{e}").parse::<f64>().expect("a float")));
    let mut random = SplitMix64(1);
    for _ in 0..50_000 {
        let f = f64::from_bits(random.next());
        if !f.is_nan() {
            floats.push(f);
        }
    }
    for _ in 0..20_000 {
        let digits = random.next() % 20;
        let n = (random.next() % 10u64.pow(digits as u32)) as f64;
        floats.push(if random.next().is_multiple_of(2) {
            n
        } else {
            -n
        });
    }
    for _ in 0..20_000 {
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
