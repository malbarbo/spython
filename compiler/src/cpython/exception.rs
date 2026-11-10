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
            #[cfg(test)]
            const ALL: &[Class] = &[$(Class::$name,)*];

            /// The name of the class in a traceback.
            pub fn name(self) -> &'static str {
                match self {
                    $(Class::$name => stringify!($name),)*
                    Class::Limit => "spython",
                }
            }

            /// The class that the builtins name `name`, when a `raise`
            /// with at most one argument, a `str`, writes the traceback
            /// that the compiler writes for it.
            pub fn of(name: &str) -> Option<Class> {
                match name {
                    $(stringify!($name) => Some(Class::$name),)*
                    _ => None,
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

// The builtins have more classes. An exception group, `KeyboardInterrupt`
// and `SystemExit` do not end the program with the usual traceback,
// `KeyError` writes the `repr` of its argument, `SyntaxError` and the
// classes that derive from it write the place of the error, and the three
// errors of Unicode take five arguments.
classes! {
    BaseException: None,
    GeneratorExit: Some(BaseException),
    Exception: Some(BaseException),
    ArithmeticError: Some(Exception),
    FloatingPointError: Some(ArithmeticError),
    OverflowError: Some(ArithmeticError),
    ZeroDivisionError: Some(ArithmeticError),
    AssertionError: Some(Exception),
    AttributeError: Some(Exception),
    BufferError: Some(Exception),
    EOFError: Some(Exception),
    ImportError: Some(Exception),
    ModuleNotFoundError: Some(ImportError),
    LookupError: Some(Exception),
    IndexError: Some(LookupError),
    MemoryError: Some(Exception),
    NameError: Some(Exception),
    UnboundLocalError: Some(NameError),
    OSError: Some(Exception),
    BlockingIOError: Some(OSError),
    ChildProcessError: Some(OSError),
    ConnectionError: Some(OSError),
    BrokenPipeError: Some(ConnectionError),
    ConnectionAbortedError: Some(ConnectionError),
    ConnectionRefusedError: Some(ConnectionError),
    ConnectionResetError: Some(ConnectionError),
    FileExistsError: Some(OSError),
    FileNotFoundError: Some(OSError),
    InterruptedError: Some(OSError),
    IsADirectoryError: Some(OSError),
    NotADirectoryError: Some(OSError),
    PermissionError: Some(OSError),
    ProcessLookupError: Some(OSError),
    TimeoutError: Some(OSError),
    ReferenceError: Some(Exception),
    RuntimeError: Some(Exception),
    NotImplementedError: Some(RuntimeError),
    PythonFinalizationError: Some(RuntimeError),
    RecursionError: Some(RuntimeError),
    StopAsyncIteration: Some(Exception),
    StopIteration: Some(Exception),
    SystemError: Some(Exception),
    TypeError: Some(Exception),
    ValueError: Some(Exception),
    UnicodeError: Some(ValueError),
    Warning: Some(Exception),
    BytesWarning: Some(Warning),
    DeprecationWarning: Some(Warning),
    EncodingWarning: Some(Warning),
    FutureWarning: Some(Warning),
    ImportWarning: Some(Warning),
    PendingDeprecationWarning: Some(Warning),
    ResourceWarning: Some(Warning),
    RuntimeWarning: Some(Warning),
    SyntaxWarning: Some(Warning),
    UnicodeWarning: Some(Warning),
    UserWarning: Some(Warning),
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

#[cfg(test)]
mod tests {
    use std::process::Command;

    use super::{Class, last_line};

    /// Writes, for each class that the arguments name, its base and the
    /// last line of the traceback of an exception without an argument, of
    /// one with an empty `str` and of one with a `str`.
    const DRIVER: &str = r#"
import builtins, sys, traceback
for name in sys.argv[1:]:
    c = getattr(builtins, name)
    last = [traceback.format_exception_only(e)[-1].rstrip("\n") for e in (c(), c(""), c("a b"))]
    print(name, c.__base__.__name__, *last, sep="|")
"#;

    #[test]
    fn each_class_has_the_base_and_the_last_line_of_cpython() {
        let python = std::env::var("PYTHON").unwrap_or_else(|_| "python3".to_owned());
        let output = Command::new(python)
            .args(["-c", DRIVER])
            .args(Class::ALL.iter().map(|class| class.name()))
            .output()
            .expect("python3 to run");
        assert!(output.status.success(), "{output:?}");
        let ours: Vec<String> = Class::ALL
            .iter()
            .map(|class| {
                let base = class.base().map_or("object", Class::name);
                let lines = ["", "", "a b"].map(|text| last_line(*class, text));
                format!("{}|{base}|{}", class.name(), lines.join("|"))
            })
            .collect();
        let theirs = String::from_utf8(output.stdout).expect("utf-8");
        assert_eq!(ours, theirs.lines().collect::<Vec<_>>());
    }

    #[test]
    fn a_class_has_the_name_that_names_it() {
        for class in Class::ALL {
            assert_eq!(Class::of(class.name()), Some(*class));
        }
        assert_eq!(Class::of("spython"), None);
        assert_eq!(Class::of("KeyError"), None);
    }
}
