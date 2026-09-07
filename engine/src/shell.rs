//! The `:` commands, over a repl that knows none of them.

/// What became of an input. `repl_run` answers with these numbers.
#[repr(u32)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Status {
    Ok = 0,
    Error = 1,
    Quit = 2,
}

/// What a command takes after its name.
#[derive(Clone, Copy)]
pub(crate) enum Arg {
    /// Nothing.
    None,
    /// Python, required. It is also what says whether the input is finished,
    /// so `:time [1,` waits for the rest of the list.
    Expr,
    /// A word, which the help shows exactly as it stands here.
    Word(&'static str),
}

#[derive(Clone, Copy)]
pub(crate) enum Builtin {
    Help,
    Quit,
    Type,
    Time,
}

/// The commands every frontend has. A terminal adds its own.
pub(crate) const BUILTINS: &[(&str, Arg, &str, Builtin)] = &[
    (":help", Arg::None, "Show this help", Builtin::Help),
    (":quit", Arg::None, "Exit the REPL", Builtin::Quit),
    (
        ":type",
        Arg::Expr,
        "Show the type of an expression",
        Builtin::Type,
    ),
    (
        ":time",
        Arg::Expr,
        "Evaluate and show execution time",
        Builtin::Time,
    ),
];

/// The name of the command at the head of the input and what follows it, or
/// none for Python, which never starts with `:`. Public so a frontend reads
/// the head of a line exactly the way the dispatch does.
pub fn split(input: &str) -> Option<(&str, &str)> {
    let trimmed = input.trim();
    if !trimmed.starts_with(':') {
        return None;
    }
    let end = trimmed.find(char::is_whitespace).unwrap_or(trimmed.len());
    let (name, arg) = trimmed.split_at(end);
    Some((name, arg.trim_start()))
}

/// Where the Python of an input begins: at its start, or past the `:type` or
/// `:time` that carries it. Every other command is the whole of its input,
/// the host's included, and has nothing to go on reading for.
///
/// An offset and not the text itself, so that a cursor given in the input's
/// own coordinates can be moved into the Python's.
fn code_start(input: &str) -> Option<usize> {
    let Some((name, arg)) = split(input) else {
        return Some(0);
    };
    BUILTINS
        .iter()
        .find(|(n, ..)| *n == name)
        .filter(|(_, arg, ..)| matches!(arg, Arg::Expr))
        // `arg` is the tail of the trimmed input, so what is missing from it
        // is everything before it.
        .map(|_| input.trim_end().len() - arg.len())
}

/// What the prompt does on Enter with the input as it stands and where the
/// cursor is in it: `-1` runs the input, and anything else is how far in the
/// line the cursor opens starts, in spaces. A command that carries Python is
/// answered for the Python it carries, and any other command is run as it
/// stands.
///
/// This is the whole of the question a reader asks on Enter, and both readers
/// ask it here -- the one in the terminal and the one the browser calls
/// through `repl_ready` (see SimpleCode's ENGINE.md).
pub fn ready_state(input: &str, cursor: usize) -> i32 {
    let Some(start) = code_start(input) else {
        return -1;
    };
    let src = &input[start..];
    // A cursor before the Python is not in it at all -- Enter pressed inside
    // `:time` opens the line the code asks for, the way it would from the
    // end. Clamping to 0 instead answers for a cursor at its start.
    crate::input::ready_state(src, cursor.checked_sub(start).unwrap_or(src.len()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The cursor at the end of the input, which is where the tests written
    /// before there was a cursor put it.
    fn ready_state(src: &str) -> i32 {
        super::ready_state(src, src.len())
    }

    /// The `|` in `marked` is where the cursor is.
    fn at_cursor(marked: &str) -> i32 {
        let (above, below) = marked.split_once('|').expect("the cursor is a `|`");
        super::ready_state(&format!("{above}{below}"), above.len())
    }

    #[test]
    fn a_command_is_told_apart_from_python() {
        assert_eq!(split(" :quit "), Some((":quit", "")));
        assert_eq!(split(":type 1"), Some((":type", "1")));
        assert_eq!(split(":time  f()\n"), Some((":time", "f()")));
        assert_eq!(split(":type"), Some((":type", "")));
        assert_eq!(split(" 1 + 1 "), None);
    }

    #[test]
    fn a_command_that_carries_no_python_is_run_as_it_stands() {
        for input in [
            ":quit",
            ":typ x",
            ":type",
            ":quit now",
            ":theme dark",
            ":theme {",
            ":level 3",
        ] {
            assert_eq!(ready_state(input), -1, "{input:?}");
        }
    }

    #[test]
    fn a_command_that_carries_python_is_answered_for_what_it_carries() {
        assert_eq!(ready_state(":time [1,"), 4);
        assert_eq!(ready_state(":type 1 +\n"), -1);
        // The cursor is in `:time`, not in the Python, so there is no line
        // being opened inside the code: it answers the way it does from the
        // end.
        assert_eq!(at_cursor(":ti|me [1,"), 4);
        assert_eq!(at_cursor("|:time 1 + 1"), -1);
    }
}
