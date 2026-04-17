#![allow(clippy::missing_safety_doc)]

#[cfg(feature = "wasm-backend")]
use engine::execute_source;
use engine::{ReplState, format_source, print_type_errors, type_check_source};
use std::sync::atomic::{AtomicBool, Ordering};

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
    let mut buf = Vec::with_capacity(size);
    let ptr = buf.as_mut_ptr();
    std::mem::forget(buf);
    ptr
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn string_deallocate(ptr: *mut u8, size: usize) {
    init();
    assert!(!ptr.is_null());
    unsafe {
        let _ = Vec::from_raw_parts(ptr, 0, size);
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cstr_deallocate(ptr: *mut std::ffi::c_char) {
    init();
    assert!(!ptr.is_null());
    unsafe {
        let _ = std::ffi::CString::from_raw(ptr);
    }
}

fn new_string(ptr: *mut u8, len: usize) -> String {
    assert!(!ptr.is_null());
    let slice = unsafe { std::slice::from_raw_parts(ptr, len) };
    String::from_utf8_lossy(slice).into_owned()
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
) -> *mut ReplState {
    init();
    let source = new_string(code_ptr, code_len);
    let config = new_string(config_ptr, config_len);
    let level = parse_config_level(&config);
    if !source.trim().is_empty() {
        match type_check_source(&source, level, false) {
            Err(te) => {
                print_type_errors(&te.db, &te.diagnostics, true);
                return std::ptr::null_mut();
            }
            Ok(Some(te)) => {
                print_type_errors(&te.db, &te.diagnostics, true);
            }
            Ok(None) => {}
        }
    }
    Box::leak(engine::repl_new(&source, level))
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
    match type_check_source(&source, level, false) {
        Err(te) => {
            print_type_errors(&te.db, &te.diagnostics, false);
            return 1;
        }
        Ok(Some(te)) => {
            print_type_errors(&te.db, &te.diagnostics, false);
        }
        Ok(None) => {}
    }
    if execute_source(&source, &filename, "/") {
        0
    } else {
        1
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn repl_run(repl: *mut ReplState, ptr: *mut u8, len: usize) -> u32 {
    init();
    assert!(!repl.is_null());
    let state = unsafe { &mut *repl };
    let code = new_string(ptr, len);
    engine::repl_run(state, &code)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn repl_destroy(repl: *mut ReplState) {
    init();
    assert!(!repl.is_null());
    unsafe {
        let _ = Box::from_raw(repl);
    }
}

// --- Completion ---

#[unsafe(no_mangle)]
pub unsafe extern "C" fn repl_complete(
    repl: *mut ReplState,
    text_ptr: *mut u8,
    text_len: usize,
    cursor_pos: usize,
) -> *mut std::ffi::c_char {
    init();
    assert!(!repl.is_null());
    let state = unsafe { &*repl };
    let text = new_string(text_ptr, text_len);

    let result =
        state.with_vm(|vm, globals| engine::completion::tab_action(vm, globals, &text, cursor_pos));

    let output = match result {
        engine::completion::TabAction::Indent(spaces) => format!("i {spaces}"),
        engine::completion::TabAction::Complete(startpos, candidates) => {
            let mut s = format!("c {startpos}");
            for c in &candidates {
                s.push(' ');
                s.push_str(c);
            }
            s
        }
        engine::completion::TabAction::Nothing => return std::ptr::null_mut(),
    };

    match std::ffi::CString::new(output) {
        Ok(cstr) => cstr.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

// --- Version ---

#[unsafe(no_mangle)]
pub extern "C" fn version() -> *mut std::ffi::c_char {
    init();
    let v = format!("spython {}", engine::LONG_VERSION);
    std::ffi::CString::new(v).unwrap().into_raw()
}

// --- Formatting ---

#[unsafe(no_mangle)]
pub unsafe extern "C" fn format(ptr: *mut u8, len: usize) -> *mut std::ffi::c_char {
    init();
    let source = new_string(ptr, len);
    format_source(&source)
        .ok()
        .and_then(|s| std::ffi::CString::new(s).ok())
        .map_or(std::ptr::null_mut(), |cstr| cstr.into_raw())
}
