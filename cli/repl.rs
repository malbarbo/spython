use std::cell::RefCell;
use std::io::IsTerminal;

use engine::completion::{self, PYTHON_BOOLEANS, PYTHON_CONSTANTS, PYTHON_KEYWORDS, TabAction};
use engine::parser::INDENT;
use engine::{Level, ReplOutcome, ReplSession};
use rustpython_vm::{PyResult, VirtualMachine, builtins::PyDictRef, scope::Scope};
use rustyline::{
    Cmd, CompletionType, Config, Context, Editor, Event, EventHandler, KeyCode, KeyEvent,
    Modifiers,
    completion::Completer,
    highlight::Highlighter,
    hint::Hinter,
    validate::{ValidationContext, ValidationResult, Validator},
};

// ── Prompt ───────────────────────────────────────────────────────────

struct ReplPrompt {
    ps1: String,
    ps2: String,
}

impl rustyline::Prompt for ReplPrompt {
    fn raw(&self) -> &str {
        &self.ps1
    }

    fn continuation_raw(&self) -> &str {
        &self.ps2
    }
}

// ── Smart keys (auto-indent / smart backspace) ──────────────────────

struct SmartKeys;

enum KeyKind {
    Enter,
    Backspace,
    Other,
}

impl SmartKeys {
    fn key_kind(evt: &Event) -> KeyKind {
        let Some(k) = evt.get(0) else {
            return KeyKind::Other;
        };
        match k {
            KeyEvent(KeyCode::Enter, _) | KeyEvent(KeyCode::Char('M' | 'J'), Modifiers::CTRL) => {
                KeyKind::Enter
            }
            KeyEvent(KeyCode::Backspace, _) | KeyEvent(KeyCode::Char('H'), Modifiers::CTRL) => {
                KeyKind::Backspace
            }
            _ => KeyKind::Other,
        }
    }
}

/// What Enter does with the input as it stands: `None` accepts it, and
/// `Some(indent)` opens a line that far in.
///
/// With text after the cursor Enter never runs anything: it opens a line for
/// the block the cursor is in, not for the one the input ends in.
fn enter_action(line: &str, pos: usize) -> Option<usize> {
    let above = line[pos..].contains('\n');
    let input = if above { &line[..pos] } else { line };
    match engine::parser::ready_state(input) {
        -1 if !above => None,
        indent => Some(usize::try_from(indent).unwrap_or(0)),
    }
}

impl rustyline::ConditionalEventHandler for SmartKeys {
    fn handle(
        &self,
        evt: &Event,
        _n: rustyline::RepeatCount,
        _positive: bool,
        ctx: &rustyline::EventContext<'_>,
    ) -> Option<Cmd> {
        match Self::key_kind(evt) {
            KeyKind::Enter => enter_action(ctx.line(), ctx.pos())
                .map(|indent| Cmd::Insert(1, format!("\n{:indent$}", ""))),
            KeyKind::Backspace => {
                let before = &ctx.line()[..ctx.pos()];
                let current_line = &before[before.rfind('\n').map_or(0, |i| i + 1)..];
                if current_line.len() > 1 && current_line.bytes().all(|b| b == b' ') {
                    let spaces = current_line.len();
                    let remove = if spaces.is_multiple_of(INDENT) {
                        INDENT
                    } else {
                        spaces % INDENT
                    };
                    Some(Cmd::Kill(rustyline::Movement::BackwardChar(remove as u16)))
                } else {
                    None
                }
            }
            KeyKind::Other => None,
        }
    }
}

// ── Helper (completion, highlighting, validation) ─────────────

struct ReplHelper<'vm> {
    vm: &'vm VirtualMachine,
    globals: PyDictRef,
    /// The session's commands: every name, and whether it takes an argument.
    commands: Vec<(&'static str, bool)>,
    /// The unfinished input the editor is still reading for. Ctrl-C abandons
    /// it; an input that runs out is run anyway, so the user is shown what is
    /// wrong with it instead of nothing at all.
    pending: RefCell<String>,
}

impl<'vm> ReplHelper<'vm> {
    fn new(
        vm: &'vm VirtualMachine,
        globals: PyDictRef,
        commands: Vec<(&'static str, bool)>,
    ) -> Self {
        ReplHelper {
            vm,
            globals,
            commands,
            pending: RefCell::new(String::new()),
        }
    }
}

impl Completer for ReplHelper<'_> {
    type Candidate = String;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &Context,
    ) -> rustyline::Result<(usize, Vec<String>)> {
        match completion::tab_action(self.vm, &self.globals, line, pos, &self.commands) {
            TabAction::Indent(spaces) => Ok((pos, vec![spaces])),
            TabAction::Complete(startpos, candidates) => Ok((startpos, candidates)),
            TabAction::Nothing => Ok((pos, vec![])),
        }
    }
}

impl Hinter for ReplHelper<'_> {
    type Hint = String;
}

impl Highlighter for ReplHelper<'_> {
    fn highlight<'l>(&self, line: &'l str, _pos: usize) -> std::borrow::Cow<'l, str> {
        if line.is_empty() {
            return std::borrow::Cow::Borrowed(line);
        }
        std::borrow::Cow::Owned(highlight_python(line, &self.commands))
    }

    fn highlight_prompt<'b, 's: 'b, 'p: 'b>(
        &'s self,
        prompt: &'p str,
        _default: bool,
    ) -> std::borrow::Cow<'b, str> {
        let t = current_theme();
        std::borrow::Cow::Owned(format!("{}{prompt}{RESET}", t.prompt))
    }

    fn highlight_char(
        &self,
        _line: &str,
        _pos: usize,
        _forced: rustyline::highlight::CmdKind,
    ) -> bool {
        true
    }
}

impl Validator for ReplHelper<'_> {
    fn validate(&self, ctx: &mut ValidationContext) -> rustyline::Result<ValidationResult> {
        if engine::parser::ready_state(ctx.input()) < 0 {
            self.pending.borrow_mut().clear();
            Ok(ValidationResult::Valid(None))
        } else {
            *self.pending.borrow_mut() = ctx.input().to_owned();
            Ok(ValidationResult::Incomplete)
        }
    }
}

fn take_pending(repl: &Editor<ReplHelper<'_>, rustyline::history::DefaultHistory>) -> String {
    repl.helper()
        .map_or_else(String::new, |helper| helper.pending.take())
}

impl rustyline::Helper for ReplHelper<'_> {}

// ── Syntax highlighting ─────────────────────────────────────────────

const RESET: &str = "\x1b[0m";

struct Theme {
    comment: &'static str,
    string: &'static str,
    number: &'static str,
    keyword: &'static str,
    builtin: &'static str,
    boolean: &'static str,
    constant: &'static str,
    decorator: &'static str,
    prompt: &'static str,
}

const ONE_DARK: Theme = Theme {
    comment: "\x1b[38;2;93;99;111m",     // #5d636f
    string: "\x1b[38;2;161;193;129m",    // #a1c181
    number: "\x1b[38;2;191;149;106m",    // #bf956a
    keyword: "\x1b[38;2;180;119;207m",   // #b477cf
    builtin: "\x1b[38;2;115;173;233m",   // #73ade9
    boolean: "\x1b[38;2;191;149;106m",   // #bf956a
    constant: "\x1b[38;2;223;193;132m",  // #dfc184
    decorator: "\x1b[38;2;116;174;232m", // #74ade8
    prompt: "\x1b[38;2;116;174;232m",    // blue
};

const ONE_LIGHT: Theme = Theme {
    comment: "\x1b[38;2;162;163;167m",  // #a2a3a7
    string: "\x1b[38;2;100;159;87m",    // #649f57
    number: "\x1b[38;2;173;110;37m",    // #ad6e25
    keyword: "\x1b[38;2;164;73;171m",   // #a449ab
    builtin: "\x1b[38;2;91;121;227m",   // #5b79e3
    boolean: "\x1b[38;2;173;110;37m",   // #ad6e25
    constant: "\x1b[38;2;193;132;1m",   // #c18401
    decorator: "\x1b[38;2;92;120;226m", // #5c78e2
    prompt: "\x1b[38;2;91;121;227m",    // blue
};

use std::sync::atomic::{AtomicBool, Ordering};

static USE_LIGHT_THEME: AtomicBool = AtomicBool::new(false);

fn current_theme() -> &'static Theme {
    if USE_LIGHT_THEME.load(Ordering::Relaxed) {
        &ONE_LIGHT
    } else {
        &ONE_DARK
    }
}

pub fn set_theme(light: bool) {
    USE_LIGHT_THEME.store(light, Ordering::Relaxed);
}

fn current_theme_name() -> &'static str {
    if USE_LIGHT_THEME.load(Ordering::Relaxed) {
        "light"
    } else {
        "dark"
    }
}

fn highlight_python(line: &str, commands: &[(&'static str, bool)]) -> String {
    let t = current_theme();
    let b = line.as_bytes();
    let len = b.len();
    let mut out = String::with_capacity(len + 64);
    let mut i = 0;

    // A `:` command is coloured as a keyword and what follows it as Python.
    // Reading the head with the engine's own `split` keeps the highlighter
    // agreeing with the dispatch about where a command name ends.
    if let Some(name) = engine::repl::split(line)
        .map(|(name, _)| name)
        .filter(|name| commands.iter().any(|(c, _)| c == name))
    {
        let cmd_start = line.find(name).unwrap();
        out.push_str(&line[..cmd_start]);
        out.push_str(t.keyword);
        out.push_str(name);
        out.push_str(RESET);
        let rest = &line[cmd_start + name.len()..];
        if !rest.is_empty() {
            out.push_str(&highlight_python(rest, commands));
        }
        return out;
    }

    while i < len {
        match b[i] {
            b'#' => {
                out.push_str(t.comment);
                out.push_str(&line[i..]);
                out.push_str(RESET);
                return out;
            }
            b'\'' | b'"' => {
                let start = i;
                i = skip_string(b, i);
                out.push_str(t.string);
                out.push_str(&line[start..i]);
                out.push_str(RESET);
            }
            b'0'..=b'9' => {
                let start = i;
                i = skip_number(b, i);
                out.push_str(t.number);
                out.push_str(&line[start..i]);
                out.push_str(RESET);
            }
            b'.' if i + 1 < len && b[i + 1].is_ascii_digit() => {
                let start = i;
                i = skip_number(b, i);
                out.push_str(t.number);
                out.push_str(&line[start..i]);
                out.push_str(RESET);
            }
            b'a'..=b'z' | b'A'..=b'Z' | b'_' => {
                let start = i;
                while i < len && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                    i += 1;
                }
                let word = &line[start..i];
                if i < len && (b[i] == b'\'' || b[i] == b'"') && is_string_prefix(word) {
                    let str_end = skip_string(b, i);
                    out.push_str(t.string);
                    out.push_str(&line[start..str_end]);
                    out.push_str(RESET);
                    i = str_end;
                } else if PYTHON_BOOLEANS.contains(&word) {
                    out.push_str(t.boolean);
                    out.push_str(word);
                    out.push_str(RESET);
                } else if PYTHON_CONSTANTS.contains(&word) {
                    out.push_str(t.constant);
                    out.push_str(word);
                    out.push_str(RESET);
                } else if PYTHON_KEYWORDS.contains(&word) {
                    out.push_str(t.keyword);
                    out.push_str(word);
                    out.push_str(RESET);
                } else if is_python_builtin(word) {
                    out.push_str(t.builtin);
                    out.push_str(word);
                    out.push_str(RESET);
                } else {
                    out.push_str(word);
                }
            }
            b'@' => {
                let start = i;
                i += 1;
                while i < len && (b[i].is_ascii_alphanumeric() || b[i] == b'_' || b[i] == b'.') {
                    i += 1;
                }
                out.push_str(t.decorator);
                out.push_str(&line[start..i]);
                out.push_str(RESET);
            }
            _ => {
                let ch = line[i..].chars().next().unwrap();
                out.push(ch);
                i += ch.len_utf8();
            }
        }
    }

    out
}

fn skip_string(b: &[u8], mut i: usize) -> usize {
    let len = b.len();
    let quote = b[i];
    if i + 2 < len && b[i + 1] == quote && b[i + 2] == quote {
        i += 3;
        while i + 2 < len {
            if b[i] == b'\\' {
                i += 2;
                continue;
            }
            if b[i] == quote && b[i + 1] == quote && b[i + 2] == quote {
                return i + 3;
            }
            i += 1;
        }
        return len;
    }
    i += 1;
    while i < len {
        if b[i] == b'\\' && i + 1 < len {
            i += 2;
            continue;
        }
        if b[i] == quote {
            return i + 1;
        }
        i += 1;
    }
    len
}

fn skip_number(b: &[u8], mut i: usize) -> usize {
    let len = b.len();
    if b[i] == b'.' {
        i += 1;
        while i < len && (b[i].is_ascii_digit() || b[i] == b'_') {
            i += 1;
        }
        return skip_exponent(b, i);
    }
    if b[i] == b'0' && i + 1 < len {
        match b[i + 1] {
            b'x' | b'X' => {
                i += 2;
                while i < len && (b[i].is_ascii_hexdigit() || b[i] == b'_') {
                    i += 1;
                }
                return i;
            }
            b'o' | b'O' => {
                i += 2;
                while i < len && ((b'0'..=b'7').contains(&b[i]) || b[i] == b'_') {
                    i += 1;
                }
                return i;
            }
            b'b' | b'B' => {
                i += 2;
                while i < len && (b[i] == b'0' || b[i] == b'1' || b[i] == b'_') {
                    i += 1;
                }
                return i;
            }
            _ => {}
        }
    }
    while i < len && (b[i].is_ascii_digit() || b[i] == b'_') {
        i += 1;
    }
    if i < len && b[i] == b'.' {
        i += 1;
        while i < len && (b[i].is_ascii_digit() || b[i] == b'_') {
            i += 1;
        }
    }
    skip_exponent(b, i)
}

fn skip_exponent(b: &[u8], mut i: usize) -> usize {
    let len = b.len();
    if i < len && (b[i] == b'e' || b[i] == b'E') {
        i += 1;
        if i < len && (b[i] == b'+' || b[i] == b'-') {
            i += 1;
        }
        while i < len && (b[i].is_ascii_digit() || b[i] == b'_') {
            i += 1;
        }
    }
    if i < len && (b[i] == b'j' || b[i] == b'J') {
        i += 1;
    }
    i
}

fn is_string_prefix(word: &str) -> bool {
    matches!(
        word,
        "f" | "F"
            | "r"
            | "R"
            | "b"
            | "B"
            | "u"
            | "U"
            | "fr"
            | "fR"
            | "Fr"
            | "FR"
            | "rf"
            | "rF"
            | "Rf"
            | "RF"
            | "br"
            | "bR"
            | "Br"
            | "BR"
            | "rb"
            | "rB"
            | "Rb"
            | "RB"
    )
}

fn is_python_builtin(word: &str) -> bool {
    matches!(
        word,
        "abs"
            | "all"
            | "any"
            | "bin"
            | "bool"
            | "bytes"
            | "callable"
            | "chr"
            | "classmethod"
            | "complex"
            | "dict"
            | "dir"
            | "divmod"
            | "enumerate"
            | "filter"
            | "float"
            | "format"
            | "frozenset"
            | "getattr"
            | "hasattr"
            | "hex"
            | "id"
            | "input"
            | "int"
            | "isinstance"
            | "issubclass"
            | "iter"
            | "len"
            | "list"
            | "map"
            | "max"
            | "min"
            | "next"
            | "object"
            | "oct"
            | "open"
            | "ord"
            | "pow"
            | "print"
            | "property"
            | "range"
            | "repr"
            | "reversed"
            | "round"
            | "set"
            | "setattr"
            | "sorted"
            | "staticmethod"
            | "str"
            | "sum"
            | "super"
            | "tuple"
            | "type"
            | "zip"
            | "ArithmeticError"
            | "AssertionError"
            | "AttributeError"
            | "EOFError"
            | "Exception"
            | "FileNotFoundError"
            | "ImportError"
            | "IndexError"
            | "KeyError"
            | "KeyboardInterrupt"
            | "NameError"
            | "NotImplementedError"
            | "OSError"
            | "OverflowError"
            | "RuntimeError"
            | "StopIteration"
            | "SyntaxError"
            | "TypeError"
            | "ValueError"
            | "ZeroDivisionError"
    )
}

// ── REPL entry point ────────────────────────────────────────────────

/// The commands only a terminal has, as opposed to the ones the engine
/// answers for every frontend. The session checks and completes them like its
/// own; adding one here without answering it below does not compile.
#[derive(Clone, Copy)]
pub enum Host {
    Level,
    Theme,
}

/// `:level` on its own says which level the REPL is on; with a number it moves
/// to that level, which everything that has already run has to survive.
fn level_command(session: &mut ReplSession<Host>, arg: &str) {
    if arg.is_empty() {
        println!("level {}", session.level());
        return;
    }
    let Some(new_level) = arg.parse::<u8>().ok().and_then(Level::from_u8) else {
        eprintln!("Invalid level: must be 0-5");
        return;
    };
    // Everything that has already run has to survive the new level.
    if !session.set_level(new_level) {
        eprintln!(
            "Cannot change to level {new_level}: \
             accumulated source has errors at that level."
        );
        return;
    }
    println!("level {new_level}");
    crate::config::save(new_level, current_theme_name());
}

/// `:theme` on its own says which theme is on; with a name it switches to it.
fn theme_command(session: &ReplSession<Host>, arg: &str) {
    match arg {
        "" => println!("{}", current_theme_name()),
        "light" | "dark" => {
            set_theme(arg == "light");
            println!("{arg}");
            crate::config::save(session.level(), arg);
        }
        _ => eprintln!("Invalid theme: use 'light' or 'dark'"),
    }
}

/// Run the terminal REPL until the user leaves it, checking every input
/// against `accumulated` -- what the file the session started from ran -- and
/// the `modules` that file imports.
pub fn run_repl(
    vm: &VirtualMachine,
    scope: Scope,
    accumulated: String,
    level: Level,
    modules: Vec<(String, String)>,
) -> PyResult<()> {
    let session =
        &mut ReplSession::new(accumulated, level, std::io::stderr().is_terminal(), modules);
    session.add(
        ":level",
        Some("[n]"),
        "Show or change the teaching level (0-5)",
        Host::Level,
    );
    session.add(
        ":theme",
        Some("[name]"),
        "Show or change theme (light, dark)",
        Host::Theme,
    );

    let repl_history_path = match dirs::data_dir() {
        Some(mut path) => {
            path.push("spython");
            let _ = std::fs::create_dir_all(&path);
            path.push("history");
            path
        }
        None => ".spython_history".into(),
    };

    {
        let mut repl = Editor::with_config(
            Config::builder()
                .completion_type(CompletionType::List)
                .tab_stop(4)
                .bracketed_paste(false)
                .build(),
        )
        .expect("failed to initialize line editor");

        repl.set_helper(Some(ReplHelper::new(
            vm,
            scope.globals.clone(),
            session.command_names(),
        )));
        repl.bind_sequence(Event::Any, EventHandler::Conditional(Box::new(SmartKeys)));
        let _ = repl.load_history(&repl_history_path);

        let ps1 = vm
            .sys_module
            .get_attr("ps1", vm)
            .and_then(|p| p.str(vm))
            .map(|s| s.expect_str().to_owned())
            .unwrap_or_default();
        let ps2 = vm
            .sys_module
            .get_attr("ps2", vm)
            .and_then(|p| p.str(vm))
            .map(|s| s.expect_str().to_owned())
            .unwrap_or_default();
        let prompt = ReplPrompt { ps1, ps2 };

        loop {
            let line = match repl.readline(&prompt) {
                Ok(line) => line,
                Err(rustyline::error::ReadlineError::Interrupted) => {
                    let _ = take_pending(&repl);
                    let exc =
                        vm.new_exception_empty(vm.ctx.exceptions.keyboard_interrupt.to_owned());
                    vm.print_exception(exc);
                    continue;
                }
                Err(rustyline::error::ReadlineError::Eof) => {
                    // An input the file ended in the middle of is still an
                    // input: run it and let the error speak.
                    let pending = take_pending(&repl);
                    if pending.trim().is_empty() {
                        break;
                    }
                    pending
                }
                Err(err) => {
                    eprintln!("Readline error: {err:?}");
                    break;
                }
            };
            let _ = repl.add_history_entry(line.trim_end());

            match session.run(vm, scope.clone(), &line) {
                ReplOutcome::Quit(None) => break,
                ReplOutcome::Quit(Some(exc)) => {
                    let _ = repl.save_history(&repl_history_path);
                    return Err(exc);
                }
                ReplOutcome::Host(Host::Level, arg) => level_command(session, &arg),
                ReplOutcome::Host(Host::Theme, arg) => theme_command(session, &arg),
                ReplOutcome::Ok | ReplOutcome::Error => {}
            }
        }
        let _ = repl.save_history(&repl_history_path);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enter_at_the_end_of_the_input_runs_it_or_reads_on() {
        assert_eq!(enter_action("x = 1", 5), None);
        assert_eq!(enter_action("def f() -> int:", 15), Some(4));
        // Wherever on the last line the cursor is: Enter runs the input, as
        // it does in every other line editor.
        assert_eq!(enter_action("print(1)", 6), None);
        assert_eq!(enter_action("print(1)", 0), None);
    }

    #[test]
    fn enter_above_the_last_line_opens_a_line_there() {
        let input = "def f() -> None:\n    print(";
        // At the end of the header, where the block it opens starts.
        assert_eq!(enter_action(input, 16), Some(4));
        // And at the end, where the call left a bracket open.
        assert_eq!(enter_action(input, input.len()), Some(8));
        // A finished line above the last one opens a line and runs nothing.
        assert_eq!(enter_action("x = 1\ny = 2", 5), Some(0));
    }
}
