#![allow(clippy::missing_safety_doc)]

use engine::{ReplState, format_source, print_type_errors, type_check_source};

// --- Memory helpers ---

#[unsafe(no_mangle)]
pub extern "C" fn string_allocate(size: usize) -> *mut u8 {
    let mut buf = Vec::with_capacity(size);
    let ptr = buf.as_mut_ptr();
    std::mem::forget(buf);
    ptr
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn string_deallocate(ptr: *mut u8, size: usize) {
    assert!(!ptr.is_null());
    unsafe {
        let _ = Vec::from_raw_parts(ptr, 0, size);
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cstr_deallocate(ptr: *mut std::ffi::c_char) {
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
    let source = new_string(code_ptr, code_len);
    let config = new_string(config_ptr, config_len);
    let level = parse_config_level(&config);
    let mut has_errors = false;
    if !source.trim().is_empty() {
        match type_check_source(&source, level) {
            Err(te) => {
                print_type_errors(&te.db, &te.diagnostics, true);
                has_errors = true;
            }
            Ok(Some(te)) => {
                print_type_errors(&te.db, &te.diagnostics, true);
            }
            Ok(None) => {}
        }
    }
    if has_errors {
        std::ptr::null_mut()
    } else {
        Box::leak(engine::repl_new(&source, level))
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn repl_run(repl: *mut ReplState, ptr: *mut u8, len: usize) -> u32 {
    assert!(!repl.is_null());
    let state = unsafe { &mut *repl };
    let code = new_string(ptr, len);
    engine::repl_run(state, &code)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn repl_destroy(repl: *mut ReplState) {
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
    let v = concat!(
        "spython ",
        env!("CARGO_PKG_VERSION"),
        " (rustpython 0.5.0, ty/ruff 0.15.6)"
    );
    match std::ffi::CString::new(v) {
        Ok(cstr) => cstr.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

// --- Formatting ---

#[unsafe(no_mangle)]
pub unsafe extern "C" fn format(ptr: *mut u8, len: usize) -> *mut std::ffi::c_char {
    let source = new_string(ptr, len);
    match format_source(&source) {
        Ok(formatted) => match std::ffi::CString::new(formatted) {
            Ok(cstr) => cstr.into_raw(),
            Err(_) => std::ptr::null_mut(),
        },
        Err(_) => std::ptr::null_mut(),
    }
}
