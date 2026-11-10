//! The classes of the builtin exceptions, and an exception as its class
//! and the text of its argument.

/// Declares each class by its name in a traceback.
macro_rules! classes {
    ($($name:ident,)*) => {
        /// A class of exception of CPython.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub(crate) enum Class {
            $($name,)*
            /// Not a class of CPython. A program that leaves a limit of
            /// spython stops with a traceback whose last line names
            /// `spython`.
            Limit,
        }

        impl Class {
            /// The name of the class in a traceback.
            pub fn name(self) -> &'static str {
                match self {
                    $(Class::$name => stringify!($name),)*
                    Class::Limit => "spython",
                }
            }
        }
    };
}

classes! {
    OverflowError,
    ZeroDivisionError,
    IndexError,
    MemoryError,
    NameError,
    UnboundLocalError,
    OSError,
    BlockingIOError,
    BrokenPipeError,
    FileExistsError,
    FileNotFoundError,
    InterruptedError,
    IsADirectoryError,
    NotADirectoryError,
    PermissionError,
    RecursionError,
    ValueError,
}

/// An exception of `class` whose argument has `text`, which is empty for
/// an exception without an argument.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Message<'a> {
    pub class: Class,
    pub text: &'a str,
}

impl<'a> Message<'a> {
    pub const fn new(class: Class, text: &'a str) -> Self {
        Message { class, text }
    }

    /// The last line of the traceback of the exception.
    pub fn line(&self) -> String {
        last_line(self.class, self.text)
    }
}

/// The last line of the traceback of an exception of `class` whose
/// argument has `text`. CPython writes the class alone for an empty text.
pub(crate) fn last_line(class: Class, text: &str) -> String {
    if text.is_empty() {
        class.name().to_owned()
    } else {
        format!("{}: {text}", class.name())
    }
}
