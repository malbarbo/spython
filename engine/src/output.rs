//! Where a captured run's output goes.

use std::cell::RefCell;

thread_local! {
    static STDOUT_BUF: RefCell<String> = RefCell::new(String::new());
    static STDERR_BUF: RefCell<String> = RefCell::new(String::new());
}

pub fn capture_output<F: FnOnce()>(f: F) -> (String, String) {
    STDOUT_BUF.with(|b| b.borrow_mut().clear());
    STDERR_BUF.with(|b| b.borrow_mut().clear());
    f();
    let out = STDOUT_BUF.with(|b| b.borrow().clone());
    let err = STDERR_BUF.with(|b| b.borrow().clone());
    (out, err)
}

pub fn write_stdout(s: &str) {
    STDOUT_BUF.with(|b| b.borrow_mut().push_str(s));
}

pub fn write_stderr(s: &str) {
    STDERR_BUF.with(|b| b.borrow_mut().push_str(s));
}

// The macros below shadow the standard print ones, so what a run prints goes
// to the buffers above instead of the process's own stdout and stderr.

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => { $crate::output::write_stdout(&format!($($arg)*)) };
}

#[macro_export]
macro_rules! println {
    () => { $crate::output::write_stdout("\n") };
    ($($arg:tt)*) => {{
        $crate::output::write_stdout(&format!($($arg)*));
        $crate::output::write_stdout("\n");
    }};
}

#[macro_export]
macro_rules! eprint {
    ($($arg:tt)*) => { $crate::output::write_stderr(&format!($($arg)*)) };
}

#[macro_export]
macro_rules! eprintln {
    () => { $crate::output::write_stderr("\n") };
    ($($arg:tt)*) => {{
        $crate::output::write_stderr(&format!($($arg)*));
        $crate::output::write_stderr("\n");
    }};
}
