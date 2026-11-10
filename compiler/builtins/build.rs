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
    // CPython takes a letter as alphabetic, and a code point with a decimal,
    // a digit or a numeric value in the UCD as decimal, digit or numeric.
    set(&mut code, "ALPHA", |cp| {
        category(&categories, cp).starts_with('L')
    });
    let data = read("UnicodeData.txt");
    for (name, field) in [("DECIMAL", 6), ("DIGIT", 7)] {
        let members = field_members(&data, field);
        set(&mut code, name, |cp| {
            members.get(cp as usize).copied().unwrap_or(false)
        });
    }
    // Each decimal digit is in a run of the ten digits from its zero.
    let zeros: Vec<u32> = data
        .lines()
        .filter_map(|line| {
            let fields: Vec<&str> = line.split(';').collect();
            (fields.get(6) == Some(&"0")).then(|| code_point(fields.first().expect("a code point")))
        })
        .collect();
    code.push_str(&format!(
        "static DECIMAL_ZEROS: [u32; {}] = {zeros:?};\n",
        zeros.len()
    ));
    let numeric = listed(&read("DerivedNumericValues.txt"));
    set(&mut code, "NUMERIC", |cp| {
        numeric.get(cp as usize).copied().unwrap_or(false)
    });
    let derived = read("DerivedCoreProperties.txt");
    for (name, property) in [
        ("CASED", "Cased"),
        ("CASE_IGNORABLE", "Case_Ignorable"),
        ("UPPERCASE", "Uppercase"),
        ("LOWERCASE", "Lowercase"),
        ("XID_START", "XID_Start"),
        ("XID_CONTINUE", "XID_Continue"),
    ] {
        let members = property_members(&derived, property);
        set(&mut code, name, |cp| {
            members.get(cp as usize).copied().unwrap_or(false)
        });
    }
    let cases = Cases::new(&read("UnicodeData.txt"), &read("SpecialCasing.txt"));
    mapping(&mut code, "UPPER", &cases.upper);
    mapping(&mut code, "LOWER", &cases.lower);
    mapping(&mut code, "TITLE", &cases.title);
    mapping(&mut code, "FOLD", &fold(&read("CaseFolding.txt")));
    std::fs::write(out.join("unicode.rs"), code).expect("the tables written");
}

/// A code point in hexadecimal.
fn code_point(text: &str) -> u32 {
    u32::from_str_radix(text.trim(), 16).expect("a code point in hexadecimal")
}

/// Whether each code point has a value in the field `field` of
/// `UnicodeData.txt`. A range of code points has no such value.
fn field_members(data: &str, field: usize) -> Vec<bool> {
    let mut members = vec![false; END as usize];
    for line in data.lines() {
        let fields: Vec<&str> = line.split(';').collect();
        if fields.get(field).is_some_and(|value| !value.is_empty()) {
            let cp = code_point(fields.first().expect("a code point"));
            *members.get_mut(cp as usize).expect("a code point") = true;
        }
    }
    members
}

/// Whether a line of `data`, a file of the UCD whose lines start with a
/// code point or a range, lists each code point.
fn listed(data: &str) -> Vec<bool> {
    let mut members = vec![false; END as usize];
    for line in data.lines() {
        let line = line.split('#').next().unwrap_or_default();
        let Some((range, _)) = line.split_once(';') else {
            continue;
        };
        let (first, last) = match range.split_once("..") {
            Some((first, last)) => (code_point(first), code_point(last)),
            None => (code_point(range), code_point(range)),
        };
        for cp in first..=last {
            *members.get_mut(cp as usize).expect("a code point") = true;
        }
    }
    members
}

/// Whether each code point has `property` in `DerivedCoreProperties.txt`,
/// whose lines are a code point or a range, `;`, and the property.
fn property_members(data: &str, property: &str) -> Vec<bool> {
    let mut members = vec![false; END as usize];
    for line in data.lines() {
        let line = line.split('#').next().unwrap_or_default();
        let Some((range, name)) = line.split_once(';') else {
            continue;
        };
        if name.trim() != property {
            continue;
        }
        let (first, last) = match range.split_once("..") {
            Some((first, last)) => (code_point(first), code_point(last)),
            None => (code_point(range), code_point(range)),
        };
        for cp in first..=last {
            *members.get_mut(cp as usize).expect("a code point") = true;
        }
    }
    members
}

/// The code points that `str.upper`, `str.lower` and `str.title` of
/// CPython give for each code point that they change.
struct Cases {
    upper: Vec<(u32, Vec<u32>)>,
    lower: Vec<(u32, Vec<u32>)>,
    title: Vec<(u32, Vec<u32>)>,
}

impl Cases {
    /// CPython takes the mapping of one code point from the fields 12, 13
    /// and 14 of `UnicodeData.txt`, and the mapping of `SpecialCasing.txt`
    /// for a code point that has one there without a condition. A code
    /// point without a title case takes its upper case.
    fn new(data: &str, special: &str) -> Cases {
        let mut upper = std::collections::BTreeMap::new();
        let mut lower = std::collections::BTreeMap::new();
        let mut title = std::collections::BTreeMap::new();
        for line in data.lines() {
            let fields: Vec<&str> = line.split(';').collect();
            let cp = code_point(fields.first().expect("a code point"));
            for (map, field) in [(&mut upper, 12), (&mut lower, 13), (&mut title, 14)] {
                match fields.get(field) {
                    Some(mapped) if !mapped.is_empty() => {
                        map.insert(cp, vec![code_point(mapped)]);
                    }
                    _ => {}
                }
            }
            if let (None, Some(mapped)) = (title.get(&cp), upper.get(&cp)) {
                title.insert(cp, mapped.clone());
            }
        }
        for line in special.lines() {
            let line = line.split('#').next().unwrap_or_default();
            let fields: Vec<&str> = line.split(';').map(str::trim).collect();
            // A line with a condition has it in a fifth field.
            let [cp, to_lower, to_title, to_upper, ""] = fields.as_slice() else {
                continue;
            };
            let cp = code_point(cp);
            for (map, mapped) in [
                (&mut upper, to_upper),
                (&mut lower, to_lower),
                (&mut title, to_title),
            ] {
                map.insert(cp, mapped.split_whitespace().map(code_point).collect());
            }
        }
        let changed = |map: std::collections::BTreeMap<u32, Vec<u32>>| {
            map.into_iter()
                .filter(|(cp, mapped)| mapped.as_slice() != [*cp])
                .collect()
        };
        Cases {
            upper: changed(upper),
            lower: changed(lower),
            title: changed(title),
        }
    }
}

/// The mapping of `str.casefold` of CPython, which is the full folding of
/// `CaseFolding.txt`, with the status `C` or `F`. A code point without one
/// stays the same.
fn fold(folding: &str) -> Vec<(u32, Vec<u32>)> {
    let mut fold = Vec::new();
    for line in folding.lines() {
        let line = line.split('#').next().unwrap_or_default();
        let fields: Vec<&str> = line.split(';').map(str::trim).collect();
        let [cp, "C" | "F", mapped, ""] = fields.as_slice() else {
            continue;
        };
        fold.push((
            code_point(cp),
            mapped.split_whitespace().map(code_point).collect(),
        ));
    }
    fold
}

/// Writes the statics `{name}_SIMPLE` and `{name}_SPECIAL` of a mapping of
/// code points, as `mapped` of `src/unicode.rs` reads them.
///
/// `{name}_SIMPLE` has the code points that go to one code point, as
/// ranges of `[first, last, delta]`, where each code point of the range
/// goes to itself plus `delta`. A range whose `last` has the bit 31 has only
/// every other code point from `first`. `{name}_SPECIAL` has the code
/// points that go to two or three, as `[code point, a, b, c]`, with 0 for a
/// `c` that is not there.
fn mapping(code: &mut String, name: &str, map: &[(u32, Vec<u32>)]) {
    // Each range as its first and last code points, the delta, and the step
    // between its code points once it has two of them.
    let mut ranges: Vec<(u32, u32, u32, Option<u32>)> = Vec::new();
    let mut special = Vec::new();
    for (cp, mapped) in map {
        let &[to] = mapped.as_slice() else {
            assert!(mapped.len() <= 3, "a mapping to more than 3 code points");
            let mut entry = [*cp, 0, 0, 0];
            for (slot, to) in entry.iter_mut().skip(1).zip(mapped) {
                *slot = *to;
            }
            special.push(entry);
            continue;
        };
        let delta = to.wrapping_sub(*cp);
        match ranges.last_mut() {
            Some((_, last, last_delta, step))
                if *last_delta == delta
                    && matches!(*cp - *last, 1 | 2)
                    && step.is_none_or(|step| step == *cp - *last) =>
            {
                *step = Some(*cp - *last);
                *last = *cp;
            }
            _ => ranges.push((*cp, *cp, delta, None)),
        }
    }
    let simple: Vec<[u32; 3]> = ranges
        .into_iter()
        .map(|(first, last, delta, step)| {
            let every_other = u32::from(step == Some(2)) << 31;
            [first, last | every_other, delta]
        })
        .collect();
    writeln!(
        code,
        "static {name}_SIMPLE: [[u32; 3]; {}] = {simple:?};\n\
         static {name}_SPECIAL: [[u32; 4]; {}] = {special:?};",
        simple.len(),
        special.len(),
    )
    .expect("a write to a String");
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
/// set of code points, and the constant `{name}_ASCII`, the bits of the
/// ASCII code points of the set, as `contains` of `src/unicode.rs` reads
/// them. The set has the code points that `accepts` accepts.
fn set(code: &mut String, name: &str, accepts: impl Fn(u32) -> bool) {
    let ascii = (0..128)
        .filter(|&cp| accepts(cp))
        .fold(0u128, |bits, cp| bits | 1 << cp);
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
        "const {name}_ASCII: u128 = {ascii};\nstatic {name}_BMP: [u16; {}] = {bmp:?};\nstatic {name}_ASTRAL: [u32; {}] = {astral:?};",
        bmp.len(),
        astral.len(),
    )
    .expect("a write to a String");
}
