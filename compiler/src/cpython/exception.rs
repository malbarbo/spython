//! The classes of the builtin exceptions, and an exception as its class
//! and the text of its argument.

/// Declares each class with its base, a base before the classes that
/// derive from it.
macro_rules! classes {
    ($($name:ident: $base:expr,)*) => {
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

            /// The class that this one derives from.
            pub fn base(self) -> Option<Class> {
                use Class::*;
                match self {
                    $($name => $base,)*
                    Limit => None,
                }
            }
        }
    };
}

classes! {
    BaseException: None,
    Exception: Some(BaseException),
    ArithmeticError: Some(Exception),
    OverflowError: Some(ArithmeticError),
    ZeroDivisionError: Some(ArithmeticError),
    LookupError: Some(Exception),
    IndexError: Some(LookupError),
    MemoryError: Some(Exception),
    NameError: Some(Exception),
    UnboundLocalError: Some(NameError),
    OSError: Some(Exception),
    BlockingIOError: Some(OSError),
    ConnectionError: Some(OSError),
    BrokenPipeError: Some(ConnectionError),
    FileExistsError: Some(OSError),
    FileNotFoundError: Some(OSError),
    InterruptedError: Some(OSError),
    IsADirectoryError: Some(OSError),
    NotADirectoryError: Some(OSError),
    PermissionError: Some(OSError),
    RuntimeError: Some(Exception),
    RecursionError: Some(RuntimeError),
    ValueError: Some(Exception),
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
