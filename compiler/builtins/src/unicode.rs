//! The properties of the code points of Unicode 15.1, the version of
//! CPython 3.13. `build.rs` writes the tables from the files in `ucd/`.
//!
//! A set of code points is two lists of the code points where the set
//! starts or stops, in order. The first list has the ones below `0x10000`,
//! and the first code point starts the set. No table holds the address of
//! another, because the linker of the compiler follows only the addresses
//! in the code.

include!(concat!(env!("OUT_DIR"), "/unicode.rs"));

/// Returns `true` if `cp` is in the set of `bmp` and `astral`, `false`
/// otherwise.
fn contains(bmp: &[u16], astral: &[u32], cp: u32) -> bool {
    let before = match u16::try_from(cp) {
        Ok(cp) => bmp.partition_point(|&at| at <= cp),
        Err(_) => bmp.len() + astral.partition_point(|&at| at <= cp),
    };
    before % 2 == 1
}

/// Returns 1 if `str.isprintable` of CPython accepts `cp`, 0 otherwise.
#[unsafe(no_mangle)]
pub extern "C" fn str_is_printable(cp: u32) -> u32 {
    u32::from(contains(&PRINTABLE_BMP, &PRINTABLE_ASTRAL, cp))
}

/// The code point `i` of the code points that `cp` goes to in the mapping
/// of `simple` and `special`, or -1 when the mapping has `i` code points or
/// fewer. `build.rs` says how the two tables hold the mapping.
fn mapped(simple: &[[u32; 3]], special: &[[u32; 4]], cp: u32, i: u32) -> i32 {
    let found = special
        .binary_search_by_key(&cp, |&[from, ..]| from)
        .ok()
        .and_then(|at| special.get(at));
    let to = match (found, i) {
        (Some(&[_, a, _, _]), 0) => a,
        (Some(&[_, _, b, _]), 1) => b,
        (Some(&[_, _, _, c]), 2) if c != 0 => c,
        (None, 0) => {
            let range = simple
                .partition_point(|&[first, ..]| first <= cp)
                .checked_sub(1)
                .and_then(|at| simple.get(at));
            match range {
                Some(&[first, last, delta]) => {
                    let (last, every_other) = (last & !(1 << 31), last >> 31 == 1);
                    if cp <= last && !(every_other && (cp - first) % 2 == 1) {
                        cp.wrapping_add(delta)
                    } else {
                        cp
                    }
                }
                None => cp,
            }
        }
        _ => return -1,
    };
    // A code point is below 2^21.
    to as i32
}

/// The code point `i` of `str.upper` of the `str` of the code point `cp`,
/// or -1 when the result has `i` code points or fewer.
#[unsafe(no_mangle)]
pub extern "C" fn str_upper_at(cp: u32, i: u32) -> i32 {
    mapped(&UPPER_SIMPLE, &UPPER_SPECIAL, cp, i)
}

/// The code point `i` of `str.lower` of the `str` of the code point `cp`,
/// or -1 when the result has `i` code points or fewer. A capital sigma goes
/// to the small sigma that is not the final one.
#[unsafe(no_mangle)]
pub extern "C" fn str_lower_at(cp: u32, i: u32) -> i32 {
    mapped(&LOWER_SIMPLE, &LOWER_SPECIAL, cp, i)
}

/// The code point `i` of the title case of the code point `cp`, or -1 when
/// the title case has `i` code points or fewer.
#[unsafe(no_mangle)]
pub extern "C" fn str_title_at(cp: u32, i: u32) -> i32 {
    mapped(&TITLE_SIMPLE, &TITLE_SPECIAL, cp, i)
}

/// The code point `i` of `str.casefold` of the `str` of the code point
/// `cp`, or -1 when the result has `i` code points or fewer.
#[unsafe(no_mangle)]
pub extern "C" fn str_fold_at(cp: u32, i: u32) -> i32 {
    mapped(&FOLD_SIMPLE, &FOLD_SPECIAL, cp, i)
}

/// Returns 1 if `cp` has the property `XID_Start`, 0 otherwise.
#[unsafe(no_mangle)]
pub extern "C" fn str_is_xid_start(cp: u32) -> u32 {
    u32::from(contains(&XID_START_BMP, &XID_START_ASTRAL, cp))
}

/// Returns 1 if `cp` has the property `XID_Continue`, 0 otherwise.
#[unsafe(no_mangle)]
pub extern "C" fn str_is_xid_continue(cp: u32) -> u32 {
    u32::from(contains(&XID_CONTINUE_BMP, &XID_CONTINUE_ASTRAL, cp))
}

/// Returns 1 if `str.isalpha` accepts `cp`, 0 otherwise.
#[unsafe(no_mangle)]
pub extern "C" fn str_is_alpha(cp: u32) -> u32 {
    u32::from(contains(&ALPHA_BMP, &ALPHA_ASTRAL, cp))
}

/// Returns 1 if `str.isdecimal` accepts `cp`, 0 otherwise.
#[unsafe(no_mangle)]
pub extern "C" fn str_is_decimal(cp: u32) -> u32 {
    u32::from(contains(&DECIMAL_BMP, &DECIMAL_ASTRAL, cp))
}

/// Returns 1 if `str.isdigit` accepts `cp`, 0 otherwise.
#[unsafe(no_mangle)]
pub extern "C" fn str_is_digit(cp: u32) -> u32 {
    u32::from(contains(&DIGIT_BMP, &DIGIT_ASTRAL, cp))
}

/// Returns 1 if `str.isnumeric` accepts `cp`, 0 otherwise.
#[unsafe(no_mangle)]
pub extern "C" fn str_is_numeric(cp: u32) -> u32 {
    u32::from(contains(&NUMERIC_BMP, &NUMERIC_ASTRAL, cp))
}

/// Returns 1 if `cp` has the property `Uppercase`, 0 otherwise.
#[unsafe(no_mangle)]
pub extern "C" fn str_is_uppercase(cp: u32) -> u32 {
    u32::from(contains(&UPPERCASE_BMP, &UPPERCASE_ASTRAL, cp))
}

/// Returns 1 if `cp` has the property `Lowercase`, 0 otherwise.
#[unsafe(no_mangle)]
pub extern "C" fn str_is_lowercase(cp: u32) -> u32 {
    u32::from(contains(&LOWERCASE_BMP, &LOWERCASE_ASTRAL, cp))
}

/// Returns 1 if `cp` has the property `Cased`, 0 otherwise.
#[unsafe(no_mangle)]
pub extern "C" fn str_is_cased(cp: u32) -> u32 {
    u32::from(contains(&CASED_BMP, &CASED_ASTRAL, cp))
}

/// Returns 1 if `cp` has the property `Case_Ignorable`, 0 otherwise.
#[unsafe(no_mangle)]
pub extern "C" fn str_is_case_ignorable(cp: u32) -> u32 {
    u32::from(contains(&CASE_IGNORABLE_BMP, &CASE_IGNORABLE_ASTRAL, cp))
}
