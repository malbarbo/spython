#![allow(clippy::missing_safety_doc)]

use spython_core::{ReplState, format_source, print_type_errors, type_check_source};

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

// --- REPL ---

#[unsafe(no_mangle)]
pub unsafe extern "C" fn repl_new(ptr: *mut u8, len: usize, level: u8) -> *mut ReplState {
    let source = new_string(ptr, len);
    let level = spython_core::Level::from_u8(level).unwrap_or(spython_core::Level::Functions);
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
        Box::leak(spython_core::repl_new(&source))
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn repl_run(repl: *mut ReplState, ptr: *mut u8, len: usize) -> bool {
    assert!(!repl.is_null());
    let state = unsafe { &mut *repl };
    let code = new_string(ptr, len);
    spython_core::repl_run(state, &code)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn repl_destroy(repl: *mut ReplState) {
    assert!(!repl.is_null());
    unsafe {
        let _ = Box::from_raw(repl);
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
