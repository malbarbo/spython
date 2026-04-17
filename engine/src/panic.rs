use crate::LONG_VERSION;

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
            version = LONG_VERSION,
            os = std::env::consts::OS,
        );
    }));
}
