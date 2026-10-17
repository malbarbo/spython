use crate::version_short;

/// Install a panic hook that prints a formatted "internal error" message
/// pointing users at the issue tracker. Shared by the CLI and WASM entry
/// points so the same message is shown regardless of how spython is invoked.
pub fn add_handler() {
    std::panic::set_hook(Box::new(|info| {
        let message = match (
            info.payload().downcast_ref::<&str>(),
            info.payload().downcast_ref::<String>(),
        ) {
            (Some(s), _) => (*s).to_string(),
            (_, Some(s)) => s.to_string(),
            (None, None) => "unknown error".into(),
        };
        if is_a_failed_print(&message) {
            // The report would go to the stream that just failed, and a pipe
            // the reader closed is not a bug of ours.
            std::process::exit(1);
        }
        let location = match info.location() {
            Some(loc) => format!("{}:{}\n  ", loc.file(), loc.line()),
            None => String::new(),
        };
        eprintln!(
            "\n\
            spython: internal error\n\n\
            This is a bug in spython.\n\
            Please report it at https://github.com/malbarbo/spython/issues\n\
            and include this error message.\n\n\
            Panic: {location}{message}\n\
            Version: {version}\n\
            OS: {os}",
            version = version_short(),
            os = std::env::consts::OS,
        );
    }));
}

fn is_a_failed_print(message: &str) -> bool {
    message.starts_with("failed printing to ")
}

#[cfg(test)]
mod tests {
    use super::is_a_failed_print;

    /// The messages are std's, from `print_to` in `std::io::stdio`.
    #[test]
    fn a_write_that_failed_is_not_a_bug() {
        assert!(is_a_failed_print(
            "failed printing to stdout: Broken pipe (os error 32)"
        ));
        assert!(is_a_failed_print(
            "failed printing to stderr: Rohrleitung unterbrochen (os error 32)"
        ));
        assert!(!is_a_failed_print("index out of bounds: the len is 0"));
    }
}
