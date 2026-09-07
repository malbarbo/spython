#![allow(clippy::missing_safety_doc)]

use engine::{Repl, Shell, check_source, format_source, print_type_errors};
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
/// Defaults to level 0. Unknown keys are ignored.
fn parse_config_level(config: &str) -> engine::Level {
    for entry in config.split_whitespace() {
        if let Some(value) = entry.strip_prefix("level=")
            && let Ok(n) = value.parse::<u8>()
            && let Some(level) = engine::Level::from_u8(n)
        {
            return level;
        }
    }
    engine::Level::Functions
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
    if has_source {
        match check_source(&source, level, false) {
            Err(te) => {
                print_type_errors(&te.db, &te.diagnostics, true);
                return ptr::null_mut();
            }
            Ok(warnings) => warnings.show(true),
        }
    }
    // The page renders the ANSI codes, so its diagnostics keep them, and it
    // has one file, so there is nothing to import.
    let mut repl = Repl::new(level, true);
    // A file that raises still opens the session, so that the student can
    // look at what the definitions above the error left behind.
    if has_source && repl.load(&source, "user.py", None, Vec::new()).is_ok() {
        let _ = repl.run_doctests();
    }
    Box::into_raw(Box::new(Shell::new(repl)))
}

/// Type-check `code` and, if no errors, execute it as a script.
/// Returns 0 on success, 1 on type errors or execution failure.
/// `filename` is used in Python tracebacks.
///
/// Only compiled when the `wasm-backend` feature is enabled — used by the
/// `wasm/spython.ts` wrapper that backs dual-run integration tests. The
/// distributed production binary does not include this export.
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
    let config = new_string(config_ptr, config_len);
    let level = parse_config_level(&config);
    match check_source(&source, level, false) {
        Err(te) => {
            print_type_errors(&te.db, &te.diagnostics, false);
            return 1;
        }
        Ok(warnings) => warnings.show(false),
    }
    if engine::run_source(&source, &filename, "/").is_ok() {
        0
    } else {
        1
    }
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
