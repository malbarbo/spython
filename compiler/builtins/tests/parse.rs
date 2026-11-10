use std::process::Command;

use builtins::f64_parse;

/// Each text has the form that the prelude passes, and reads as the `float`
/// of CPython, bit for bit, apart from the sign of a NaN.
#[test]
fn parse_reads_a_text_as_cpython_does() {
    let texts = [
        "0",
        "-0",
        "+1",
        "1.5",
        ".5",
        "5.",
        "1.e5",
        "1e10",
        "1E-10",
        "1.025e-09",
        "1e400",
        "-1e400",
        "1.5e-400",
        "4.9e-324",
        "2.4703282292062328e-324",
        "2.4703282292062327e-324",
        "1.7976931348623157e308",
        "1.7976931348623158e308",
        "1.7976931348623159e308",
        "9007199254740993",
        "0.1",
        "123456789012345678901234567890",
        "inf",
        "-InFiNiTy",
        "iNf",
        "+nan",
        "-nan",
        "NaN",
        "0.000000000000000000000000000000000000001",
    ];
    let program = format!(
        "import math, struct\n\
         for t in {texts:?}:\n    \
         x = float(t)\n    \
         print('nan' if math.isnan(x) else struct.unpack('<Q', struct.pack('<d', x))[0])"
    );
    let output = Command::new(std::env::var("PYTHON").unwrap_or_else(|_| "python3".into()))
        .args(["-c", &program])
        .output()
        .expect("python3 should run");
    assert!(output.status.success(), "{output:?}");
    let expected = String::from_utf8(output.stdout).expect("digits are ASCII");
    for (text, want) in texts.iter().zip(expected.lines()) {
        let got = unsafe { f64_parse(text.as_ptr(), text.len()) };
        let got = if got.is_nan() {
            "nan".to_owned()
        } else {
            got.to_bits().to_string()
        };
        assert_eq!(got, want, "{text}");
    }
}
