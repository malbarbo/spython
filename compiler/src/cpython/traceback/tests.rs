//! Each case marks the instruction with `«` and `»`, and the test compares
//! the lines with the ones that the `traceback` module of python3 writes.

use std::process::Command;

use ruff_text_size::{TextRange, TextSize};

use super::{FileLines, source_lines};

/// Writes the source of each case to a file, and the character offsets of
/// the instruction to another, then asks python3 for the lines of each.
const DRIVER: &str = r#"
import re, sys, traceback
out = []
for i in range(int(sys.argv[2])):
    path = f"{sys.argv[1]}/case{i}.py"
    start, end = map(int, open(f"{sys.argv[1]}/case{i}.pos").read().split())
    with open(path, encoding="utf-8-sig", newline="") as f:
        src = f.read()
    def position(offset):
        before = src[:offset]
        lines = re.split("\r\n|\r|\n", before)
        return len(lines), len(lines[-1].encode())
    lineno, colno = position(start)
    end_lineno, end_colno = position(end)
    frame = traceback.FrameSummary(path, lineno, "f", end_lineno=end_lineno,
                                   colno=colno, end_colno=end_colno)
    text = traceback.StackSummary().format_frame_summary(frame)
    out.append(text.split("\n", 1)[1])
sys.stdout.write("\0".join(out))
"#;

const CASES: &[&str] = &[
    "print(«7 // 0»)\n",
    "print(«7//0»)\n",
    "«7 // 0»\n",
    "x = «7 // (a - 1)»\n",
    "def f(a):\n    return «7 // a»\n",
    "def f(a):\n\treturn «7 // a»\n",
    "def f():\n    return «g(0)»\n",
    "y = «g(0)»\n",
    "y = z = «g(0)»\n",
    "y: int = «g(0)»\n",
    "return «a.g(0)»\n",
    "print(«g(0)»)\n",
    "def f():\n    return g(«h(0)»)\n",
    "x = «a[i + 1]»\n",
    "print(«-x»)\n",
    "«-x»\n",
    "print(«x ** 2», «x % 3»)\n",
    "print(«x << 40»)\n",
    "print(«((a)) // b»)\n",
    "print(«('#' * 2) // 0»)\n",
    "x = («1 +\n     2 //\n     0»)\n",
    "x = («1 // (\n     2\n     + 3)»)\n",
    "x = «f(\n    1,\n    2,\n    3,\n    4,\n    5,\n)»\n",
    "x = «a[\n    1\n]»\n",
    "x = «f(\n    1,\n    2,\n    3) + g(\n    4,\n    5,\n    6)»\n",
    "x = («a  # comentário\n     // b»)\n",
    "x = «7 \\\n    // 0»\n",
    "x = («7\n\n     // 0»)\n",
    "print('日本', «1 // 0»)\n",
    "print(«'日本' + 1»)\n",
    "print('😀', «g(1)»)\n",
    "print('ção', «7 // 0»)\n",
    "s = '\u{1c}'; print(«7 // 0»)\n",
    "\u{c}print(«7 // 0»)\n",
    "x = 1\r\nprint(«7 // 0»)\r\n",
    "x = 1\rprint(«7 // 0»)\r",
    "\u{feff}print(«7 // 0»)\n",
    "print(«7 // 0»)   \n",
    "if x:\n    y = 1\n    print(«7 // 0»)\n",
    "x = («g(1)\n     + h(2)»)\n",
    "«g(1)\n»",
];

fn split(case: &str) -> (String, usize, usize) {
    let start = case.find('«').expect("a start mark");
    let end = case.find('»').expect("an end mark");
    let source = case.replace(['«', '»'], "");
    let end = end - '«'.len_utf8();
    (source, start, end)
}

#[test]
fn the_lines_of_a_frame_are_the_ones_of_cpython() {
    let dir = std::env::temp_dir().join(format!("spython-traceback-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("the directory of the cases");
    let mut ours = Vec::new();
    for (i, case) in CASES.iter().enumerate() {
        let (source, start, end) = split(case);
        std::fs::write(dir.join(format!("case{i}.py")), &source).expect("the case written");
        // Python reads the file without the byte order mark, and counts in
        // characters.
        let text = source.strip_prefix('\u{feff}').unwrap_or(&source);
        let bom = source.len() - text.len();
        let chars = |offset: usize| {
            text.get(..offset - bom)
                .expect("an offset on a character")
                .chars()
                .count()
        };
        std::fs::write(
            dir.join(format!("case{i}.pos")),
            format!("{} {}", chars(start), chars(end)),
        )
        .expect("the positions written");
        let range = TextRange::new(
            TextSize::try_from(start).expect("a small offset"),
            TextSize::try_from(end).expect("a small offset"),
        );
        ours.push(source_lines(&FileLines::new(&source), range));
    }
    let python = std::env::var("PYTHON").unwrap_or_else(|_| "python3".to_owned());
    let output = Command::new(python)
        .args(["-c", DRIVER])
        .arg(&dir)
        .arg(CASES.len().to_string())
        .output()
        .expect("python3 to start");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("utf-8 from python3");
    let theirs: Vec<&str> = stdout.split('\0').collect();
    assert_eq!(theirs.len(), CASES.len());
    std::fs::remove_dir_all(&dir).expect("the cases removed");
    for ((case, ours), theirs) in CASES.iter().zip(&ours).zip(theirs) {
        assert_eq!(ours, theirs, "case {case:?}");
    }
}

#[test]
fn the_wide_characters_are_the_ones_of_python() {
    let python = std::env::var("PYTHON").unwrap_or_else(|_| "python3".to_owned());
    let output = Command::new(python)
        .args([
            "-c",
            "import unicodedata as u\n\
             print(' '.join(str(c) for c in range(0x110000)\n\
             if u.east_asian_width(chr(c)) in 'WF'))",
        ])
        .output()
        .expect("python3 to start");
    let theirs: Vec<u32> = String::from_utf8_lossy(&output.stdout)
        .split_whitespace()
        .map(|c| c.parse().expect("a code point"))
        .collect();
    let ours: Vec<u32> = (0..0x110000)
        .filter(|&c| char::from_u32(c).is_some_and(super::is_wide))
        .collect();
    // The surrogates are not chars in Rust, and none of them is wide.
    assert_eq!(ours, theirs);
}
