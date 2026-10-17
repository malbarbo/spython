#![allow(clippy::missing_safety_doc)]

use engine::{Loaded, Repl, Shell, format_source, print_type_errors};
use std::ffi::CString;
use std::sync::atomic::{AtomicBool, Ordering};
use std::{alloc, ptr};

static INIT: AtomicBool = AtomicBool::new(false);

fn init() {
    if !INIT.swap(true, Ordering::Relaxed) {
        engine::panic::add_handler();
    }
}

// --- Memory helpers ---

#[unsafe(no_mangle)]
pub extern "C" fn string_allocate(size: usize) -> *mut u8 {
    init();
    if size == 0 {
        return ptr::NonNull::dangling().as_ptr();
    }
    let layout = string_layout(size);
    let ptr = unsafe { alloc::alloc(layout) };
    if ptr.is_null() {
        alloc::handle_alloc_error(layout);
    }
    ptr
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn string_deallocate(ptr: *mut u8, size: usize) {
    init();
    assert!(!ptr.is_null());
    if size == 0 {
        return;
    }
    assert_ne!(ptr, ptr::NonNull::dangling().as_ptr());
    unsafe { alloc::dealloc(ptr, string_layout(size)) };
}

/// The page asks for bytes and reads them back as text, so the size it gave
/// is the size the block has to be freed with.
fn string_layout(size: usize) -> alloc::Layout {
    alloc::Layout::from_size_align(size, 1).expect("more bytes than a layout holds")
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cstr_deallocate(ptr: *mut std::ffi::c_char) {
    init();
    assert!(!ptr.is_null());
    unsafe {
        let _ = CString::from_raw(ptr);
    }
}

fn new_string(ptr: *mut u8, len: usize) -> String {
    assert!(!ptr.is_null());
    let slice = unsafe { std::slice::from_raw_parts(ptr, len) };
    String::from_utf8_lossy(slice).into_owned()
}

/// A string the page takes over, or null for one with a nul inside it, which
/// no C string holds.
fn to_cstr(s: String) -> *mut std::ffi::c_char {
    match CString::new(s) {
        Ok(cstr) => cstr.into_raw(),
        Err(_) => ptr::null_mut(),
    }
}

// --- Config parsing ---

/// Parse `level=<n>` from a space-separated `key=value` config string.
/// Defaults to level 5. Unknown keys are ignored.
fn parse_config_level(config: &str) -> engine::Level {
    for entry in config.split_whitespace() {
        if let Some(value) = entry.strip_prefix("level=")
            && let Ok(n) = value.parse::<u8>()
            && let Some(level) = engine::Level::from_u8(n)
        {
            return level;
        }
    }
    engine::Level::Full
}

// --- Checking ---

/// Check `source` at `level`, print what the check found, and say whether the
/// file holds. Everything that takes a whole file starts here.
fn check_and_report(source: &str, level: engine::Level, use_color: bool) -> bool {
    match engine::check_source(source, level, false) {
        Err(te) => {
            print_type_errors(&te.db, &te.diagnostics, use_color);
            false
        }
        Ok(warnings) => {
            warnings.show(use_color);
            true
        }
    }
}

// --- REPL ---

#[unsafe(no_mangle)]
pub unsafe extern "C" fn repl_new(
    code_ptr: *mut u8,
    code_len: usize,
    config_ptr: *mut u8,
    config_len: usize,
) -> *mut Shell {
    init();
    let source = new_string(code_ptr, code_len);
    let config = new_string(config_ptr, config_len);
    let level = parse_config_level(&config);
    let has_source = !source.trim().is_empty();
    // The page renders the ANSI codes, so its diagnostics keep them, and it
    // has one file, so there is nothing to import.
    if has_source && !check_and_report(&source, level, true) {
        return ptr::null_mut();
    }
    let mut repl = Repl::new(level, true);
    repl.install_displayhook();
    // A file that raises or calls `exit()` ends the session in the CLI. The
    // protocol has no way to open a session that is already over, so the page
    // opens an empty one instead of keeping names the checker cannot know.
    if has_source {
        match repl.load(&source, "user.py", None, Vec::new()) {
            Ok(Loaded::Ran) => {
                let _ = repl.run_doctests();
            }
            Ok(Loaded::Exited(_)) | Err(_) => {
                drop(repl);
                repl = Repl::new(level, true);
                repl.install_displayhook();
            }
        }
    }
    Box::into_raw(Box::new(Shell::new(repl)))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn repl_run(repl: *mut Shell, ptr: *mut u8, len: usize) -> u32 {
    init();
    assert!(!repl.is_null());
    let shell = unsafe { &mut *repl };
    let code = new_string(ptr, len);
    shell.run(&code) as u32
}

/// `repl_ready` (see SimpleCode's ENGINE.md), answered by
/// `engine::shell::ready_state`. The session is not asked: what a Python
/// input is waiting on is in its own text, which the parser reads without a
/// running interpreter.
///
/// `cursor` is a byte offset into the text, as `repl_complete` takes one.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn repl_ready(
    _repl: *mut Shell,
    ptr: *mut u8,
    len: usize,
    cursor: usize,
) -> i32 {
    init();
    engine::shell::ready_state(&new_string(ptr, len), cursor)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn repl_destroy(repl: *mut Shell) {
    init();
    // `repl_new` answers with null when it has no shell to give, and the page
    // hands that null back here, taking it for a shell.
    if repl.is_null() {
        return;
    }
    unsafe {
        let _ = Box::from_raw(repl);
    }
}

// --- The command line, for the dual-run ---
//
// `run`, `check` and `test` as the binary runs them, each over a source the
// caller has already read. Only compiled when the `wasm-backend` feature is
// enabled, for the `wasm/spython.ts` wrapper that backs the dual-run
// integration tests. The distributed production binary, which reaches Python
// through `repl_new`, does not include them.
//
// Nothing here takes colour: what the tests compare is what a redirected
// stream holds.

/// Type-check `code` and, if it holds, run it as a script. `filename` is what
/// a traceback names. Returns 0 when the run finished and 1 when it did not.
#[cfg(feature = "wasm-backend")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn run_source(
    code_ptr: *mut u8,
    code_len: usize,
    filename_ptr: *mut u8,
    filename_len: usize,
    config_ptr: *mut u8,
    config_len: usize,
) -> u32 {
    init();
    let source = new_string(code_ptr, code_len);
    let filename = new_string(filename_ptr, filename_len);
    let level = parse_config_level(&new_string(config_ptr, config_len));
    if !check_and_report(&source, level, false) {
        return 1;
    }
    u32::from(engine::run_source(&source, &filename, "/").is_err())
}

/// Type-check `code` and say what the check found, running nothing. Returns 0
/// when it holds and 1 when it does not.
#[cfg(feature = "wasm-backend")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn check_source(
    code_ptr: *mut u8,
    code_len: usize,
    config_ptr: *mut u8,
    config_len: usize,
) -> u32 {
    init();
    let source = new_string(code_ptr, code_len);
    let level = parse_config_level(&new_string(config_ptr, config_len));
    u32::from(!check_and_report(&source, level, false))
}

/// Type-check `code` and, if it holds, run the doctests of the file
/// `filename` holding it. Returns 0 when every example passed and 1 when one
/// did not.
#[cfg(feature = "wasm-backend")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn test_source(
    code_ptr: *mut u8,
    code_len: usize,
    filename_ptr: *mut u8,
    filename_len: usize,
    config_ptr: *mut u8,
    config_len: usize,
) -> u32 {
    init();
    let source = new_string(code_ptr, code_len);
    let filename = new_string(filename_ptr, filename_len);
    let level = parse_config_level(&new_string(config_ptr, config_len));
    if !check_and_report(&source, level, false) {
        return 1;
    }
    u32::from(engine::run_test_source(&source, &filename).is_err())
}

// --- Completion ---

#[unsafe(no_mangle)]
pub unsafe extern "C" fn repl_complete(
    repl: *mut Shell,
    text_ptr: *mut u8,
    text_len: usize,
    cursor_pos: usize,
) -> *mut std::ffi::c_char {
    init();
    assert!(!repl.is_null());
    let shell = unsafe { &*repl };
    let text = new_string(text_ptr, text_len);

    let result = shell.complete(&text, cursor_pos);

    let output = match result {
        engine::completion::TabAction::Indent(spaces) => format!("i {spaces}"),
        // The offset, then one candidate per line, so that a candidate can
        // carry the space that follows it.
        engine::completion::TabAction::Complete(startpos, candidates) => {
            let mut s = format!("c {startpos}");
            for c in &candidates {
                s.push('\n');
                s.push_str(c);
            }
            s
        }
        engine::completion::TabAction::Nothing => return ptr::null_mut(),
    };

    to_cstr(output)
}

// --- Version ---

#[unsafe(no_mangle)]
pub extern "C" fn version() -> *mut std::ffi::c_char {
    init();
    to_cstr(engine::version())
}

// --- Formatting ---

#[unsafe(no_mangle)]
pub unsafe extern "C" fn format(ptr: *mut u8, len: usize) -> *mut std::ffi::c_char {
    init();
    let source = new_string(ptr, len);
    format_source(&source).map_or(ptr::null_mut(), to_cstr)
}

// --- Fonts ---

/// Face `i` of the fonts that the frames of a World draw with, for the canvas
/// of the page. It gives the pointer and the length of the bytes, the pointer
/// and the length of the family name, the weight, and 1 for italic. Past the
/// last face it gives null. See ENGINE.md in simplecode.
// The records hold 32-bit pointers, which only wasm32 has.
#[cfg(target_arch = "wasm32")]
#[unsafe(no_mangle)]
pub extern "C" fn font_face(i: u32) -> *const [u32; 6] {
    sinteract::text::face_record(i).map_or(ptr::null(), |face| face as *const [u32; 6])
}
