//! The `:` commands, over a repl that knows none of them.

use std::time::Duration;

use crate::Level;
use crate::completion::TabAction;
use crate::repl::{Failed, Repl};

/// What the terminal says when the session opens.
pub fn welcome_message(level: Level) -> String {
    format!(
        "Welcome to {}, level {level}.\nType ctrl-d or \":quit\" to exit.\n",
        crate::version()
    )
}

/// What became of an input. `repl_run` answers with these numbers.
#[repr(u32)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Status {
    Ok = 0,
    Error = 1,
    Quit = 2,
}

pub struct Shell {
    repl: Repl,
    commands: Vec<Command>,
}

struct Command {
    name: &'static str,
    arg: Arg,
    help: &'static str,
    action: Action,
}

enum Action {
    Builtin(Builtin),
    Host(HostAction),
}

/// A command of the host's. It gets the repl, so that a command can change
/// the session.
type HostAction = Box<dyn FnMut(&mut Repl, &str) -> Result<(), String>>;

/// What a command takes after its name.
#[derive(Clone, Copy)]
enum Arg {
    /// Nothing.
    None,
    /// Python, required. It is also what says whether the input is finished,
    /// so `:time [1,` waits for the rest of the list.
    Expr,
    /// A word, which the help shows exactly as it stands here.
    Word(&'static str),
}

#[derive(Clone, Copy)]
enum Builtin {
    Help,
    Quit,
    Type,
    Time,
}

/// The commands every frontend has. A terminal adds its own.
const BUILTINS: &[(&str, Arg, &str, Builtin)] = &[
    (":help", Arg::None, "Show this help", Builtin::Help),
    (":quit", Arg::None, "Exit the REPL", Builtin::Quit),
    (
        ":type",
        Arg::Expr,
        "Show the type of an expression without running it",
        Builtin::Type,
    ),
    (
        ":time",
        Arg::Expr,
        "Evaluate and show execution time",
        Builtin::Time,
    ),
];

impl Shell {
    pub fn new(repl: Repl) -> Shell {
        let commands = BUILTINS
            .iter()
            .map(|&(name, arg, help, builtin)| Command {
                name,
                arg,
                help,
                action: Action::Builtin(builtin),
            })
            .collect();
        Shell { repl, commands }
    }

    /// A command of the host's. `word` is what the help shows after the name,
    /// for one that takes a word; `run` gets the repl and what followed the
    /// name, and answers with what to tell the user when it refuses.
    pub fn add(
        &mut self,
        name: &'static str,
        word: Option<&'static str>,
        help: &'static str,
        run: impl FnMut(&mut Repl, &str) -> Result<(), String> + 'static,
    ) {
        debug_assert!(self.commands.iter().all(|c| c.name != name));
        self.commands.push(Command {
            name,
            arg: word.map_or(Arg::None, Arg::Word),
            help,
            action: Action::Host(Box::new(run)),
        });
    }

    pub fn repl(&self) -> &Repl {
        &self.repl
    }

    pub fn run(&mut self, input: &str) -> Status {
        let Some((name, arg)) = split(input) else {
            let ran = self.repl.run(input);
            return self.status(ran.map(drop));
        };
        let Shell { repl, commands } = self;
        let Some(command) = commands.iter_mut().find(|c| c.name == name) else {
            eprintln!("Unknown command {name}. Type :help to see the commands.");
            return Status::Error;
        };
        match (command.arg, arg.is_empty()) {
            (Arg::None, false) => {
                eprintln!("The {name} command takes nothing after it.");
                return Status::Error;
            }
            (Arg::Expr, true) => {
                eprintln!("The {name} command takes an expression.");
                return Status::Error;
            }
            _ => {}
        }
        let builtin = match &mut command.action {
            Action::Host(run) => {
                return match run(repl, arg) {
                    Ok(()) => Status::Ok,
                    Err(message) => {
                        eprintln!("{message}");
                        Status::Error
                    }
                };
            }
            Action::Builtin(builtin) => *builtin,
        };
        self.builtin(builtin, arg)
    }

    fn builtin(&mut self, builtin: Builtin, arg: &str) -> Status {
        match builtin {
            Builtin::Help => {
                let usages: Vec<String> = self.commands.iter().map(Command::usage).collect();
                let width = usages.iter().map(String::len).max().unwrap_or(0);
                println!("Commands:");
                for (usage, c) in usages.iter().zip(&self.commands) {
                    println!("  {usage:<width$}  {}", c.help);
                }
                Status::Ok
            }
            Builtin::Quit => Status::Quit,
            Builtin::Type => match self.repl.type_of(arg) {
                Some(type_) => {
                    println!("{type_}");
                    Status::Ok
                }
                None => {
                    eprintln!("Could not infer type");
                    Status::Error
                }
            },
            Builtin::Time => match self.repl.run(arg) {
                Ok(elapsed) => {
                    println!("Time: {}", format_duration(elapsed));
                    Status::Ok
                }
                Err(Failed) => self.status(Err(Failed)),
            },
        }
    }

    /// Every command's name, sorted, and whether it takes anything after it.
    /// What a completer offers when the line so far is a colon, and what the
    /// terminal's highlighter matches a line against.
    pub fn command_names(&self) -> Vec<(&'static str, bool)> {
        let mut names: Vec<(&'static str, bool)> = self
            .commands
            .iter()
            .map(|c| (c.name, !matches!(c.arg, Arg::None)))
            .collect();
        names.sort();
        names
    }

    /// What the Tab key should do at `cursor`, over the session's names and
    /// the commands. The repl answers for the names, which it can only read
    /// off the interpreter it owns.
    pub fn complete(&self, text: &str, cursor: usize) -> TabAction {
        self.repl.complete(text, cursor, &self.command_names())
    }

    /// An input that left an `exit()` behind ends the session, whether the
    /// input itself ran or not.
    fn status(&self, result: Result<(), Failed>) -> Status {
        if self.repl.exit_status().is_some() {
            Status::Quit
        } else if result.is_ok() {
            Status::Ok
        } else {
            Status::Error
        }
    }
}

impl Command {
    fn usage(&self) -> String {
        match self.arg {
            Arg::None => self.name.to_owned(),
            Arg::Expr => format!("{} <expression>", self.name),
            Arg::Word(word) => format!("{} {word}", self.name),
        }
    }
}

fn format_duration(elapsed: Duration) -> String {
    if elapsed.as_secs() > 0 {
        format!("{:.2} s", elapsed.as_secs_f64())
    } else if elapsed.as_millis() > 0 {
        format!("{} ms", elapsed.as_millis())
    } else if elapsed.as_micros() > 0 {
        format!("{} \u{b5}s", elapsed.as_micros())
    } else {
        format!("{} ns", elapsed.as_nanos())
    }
}

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
/// ask it here, the one in the terminal and the one the browser calls
/// through `repl_ready` (see SimpleCode's ENGINE.md).
pub fn ready_state(input: &str, cursor: usize) -> i32 {
    let Some(start) = code_start(input) else {
        return -1;
    };
    let src = &input[start..];
    // A cursor before the Python is not in it at all. Enter pressed inside
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
    fn a_duration_is_said_in_its_own_unit() {
        assert_eq!(format_duration(Duration::from_secs(2)), "2.00 s");
        assert_eq!(format_duration(Duration::from_millis(5)), "5 ms");
        assert_eq!(format_duration(Duration::from_micros(7)), "7 \u{b5}s");
        assert_eq!(format_duration(Duration::from_nanos(9)), "9 ns");
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
