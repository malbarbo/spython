//! Writes the tables of `src/unicode.rs` from the files of the Unicode
//! Character Database in `ucd/`, which are the ones of Unicode 15.1, the
//! version of CPython 3.13.

use std::fmt::Write;
use std::path::PathBuf;

/// One past the last code point.
const END: u32 = 0x11_0000;

fn main() {
    let dir = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").expect("cargo sets it"));
    let out = PathBuf::from(std::env::var_os("OUT_DIR").expect("cargo sets it"));
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=ucd");
    let read = |name: &str| {
        std::fs::read_to_string(dir.join("ucd").join(name))
            .unwrap_or_else(|error| panic!("ucd/{name}: {error}"))
    };

    let categories = categories(&read("UnicodeData.txt"));
    let mut code = String::new();
    // CPython takes a code point as printable when its category is not
    // `C` or `Z`, and the space as printable too.
    set(&mut code, "PRINTABLE", |cp| {
        cp == 0x20 || !matches!(category(&categories, cp).as_bytes(), [b'C' | b'Z', ..])
    });
    std::fs::write(out.join("unicode.rs"), code).expect("the tables written");
}

/// The general category of `cp`, which is `Cn` when no file gives one.
fn category(categories: &[String], cp: u32) -> &str {
    categories.get(cp as usize).map_or("Cn", String::as_str)
}

/// The general category of each code point of `UnicodeData.txt`. A pair of
/// lines whose names end in `First>` and `Last>` gives the category of every
/// code point between the two.
fn categories(data: &str) -> Vec<String> {
    let mut categories = vec!["Cn".to_owned(); END as usize];
    let mut first = None;
    for line in data.lines() {
        let mut fields = line.split(';');
        let (Some(cp), Some(name), Some(category)) = (fields.next(), fields.next(), fields.next())
        else {
            panic!("a line of UnicodeData.txt without three fields: {line}");
        };
        let cp = u32::from_str_radix(cp, 16).expect("a code point in hexadecimal");
        if name.ends_with(", First>") {
            first = Some(cp);
            continue;
        }
        let start = if name.ends_with(", Last>") {
            first.take().expect("a First line before a Last line")
        } else {
            cp
        };
        for cp in start..=cp {
            let slot = categories.get_mut(cp as usize).expect("a code point");
            category.clone_into(slot);
        }
    }
    categories
}

/// Writes the statics `{name}_BMP` and `{name}_ASTRAL`, the two lists of a
/// set of code points, as `contains` of `src/unicode.rs` reads them. The set
/// has the code points that `accepts` accepts.
fn set(code: &mut String, name: &str, accepts: impl Fn(u32) -> bool) {
    let (mut bmp, mut astral) = (Vec::new(), Vec::new());
    let mut inside = false;
    for cp in 0..END {
        if accepts(cp) != inside {
            inside = !inside;
            match u16::try_from(cp) {
                Ok(cp) => bmp.push(cp),
                Err(_) => astral.push(cp),
            }
        }
    }
    writeln!(
        code,
        "static {name}_BMP: [u16; {}] = {bmp:?};\nstatic {name}_ASTRAL: [u32; {}] = {astral:?};",
        bmp.len(),
        astral.len(),
    )
    .expect("a write to a String");
}
