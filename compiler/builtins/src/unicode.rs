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
