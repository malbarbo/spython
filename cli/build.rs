use std::{env, fmt::Write, fs, path::Path};

/// One test per input file, so they run in parallel: a single test walking the
/// directory is as slow as the sum of its files.
fn main() {
    println!("cargo:rerun-if-changed=tests/inputs");
    println!("cargo:rerun-if-changed=tests/images");

    let mut src = String::new();
    tests(&mut src, "tests/inputs", "run_file", 4);
    tests(&mut src, "tests/images", "run_images", 5);

    let out = env::var("OUT_DIR").expect("OUT_DIR");
    fs::write(Path::new(&out).join("file_tests.rs"), src).expect("write the generated tests");
}

fn tests(src: &mut String, dir: &str, snapshot: &str, level: u8) {
    let mut names: Vec<String> = fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("read {dir}: {e}"))
        .map(|entry| entry.expect("a directory entry").file_name())
        .map(|name| name.to_str().expect("a utf8 file name").to_string())
        .filter(|name| name.ends_with(".py"))
        .collect();
    names.sort();

    for name in names {
        let ident = format!("{snapshot}_{}", sanitize(&name));
        // The image snapshots are long and many; CI runs them with --ignored.
        if snapshot == "run_images" {
            let _ = writeln!(src, "#[ignore]");
        }
        // An image is a whole snapshot of its own: the svg, and nothing around it.
        let check = if snapshot == "run_images" {
            "check_image"
        } else {
            "check"
        };
        let _ = writeln!(
            src,
            "#[test]\nfn {ident}() {{ {check}(\"{snapshot}\", {level}, \"{dir}/{name}\"); }}"
        );
    }
}

/// A file name is not an identifier: `err-type.py` is a valid input, and the
/// test named after it has to be a valid name.
fn sanitize(name: &str) -> String {
    let stem = name.strip_suffix(".py").unwrap_or(name);
    stem.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect()
}
