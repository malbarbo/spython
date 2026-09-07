//! The line the terminal reads, and the colours it draws it in.

use std::cell::{Cell, RefCell};
use std::io::IsTerminal as _;
use std::path::PathBuf;
use std::rc::Rc;

use engine::Shell;
use engine::completion::{PYTHON_BOOLEANS, PYTHON_CONSTANTS, PYTHON_KEYWORDS, TabAction};
use engine::input::INDENT;
use rustyline::{
    Cmd, CompletionType, ConditionalEventHandler, Config, Context, Editor, EventContext,
    EventHandler, KeyCode, KeyEvent, Modifiers, Movement, RepeatCount,
    completion::Completer,
    error::ReadlineError,
    highlight::Highlighter,
    hint::Hinter,
    history::FileHistory,
    validate::{ValidationContext, ValidationResult, Validator},
};

const HISTORY_DIR: &str = "spython";
const HISTORY_FILE: &str = "history";

pub struct ReplReader {
    editor: Editor<InputHelper, FileHistory>,
    /// Where the history is read from and written back to, for as long as the
    /// session lasts.
    history: Option<PathBuf>,
    /// Whether the read that ends the session has already happened, as a
    /// reader that answered `None` is not asked to read again.
    done: bool,
}

impl ReplReader {
    pub fn new(
        shell: Rc<RefCell<Shell>>,
        commands: Vec<(&'static str, bool)>,
        theme: Rc<Cell<Theme>>,
    ) -> rustyline::Result<ReplReader> {
        let mut editor = Editor::with_config(
            Config::builder()
                .completion_type(CompletionType::List)
                .tab_stop(INDENT as u8)
                .bracketed_paste(false)
                .build(),
        )?;

        let color = std::io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none();
        editor.set_helper(Some(InputHelper {
            shell,
            commands,
            pending: RefCell::new(String::new()),
            color,
            theme,
        }));

        // Enter and Backspace move by a level of indentation, and the two
        // control characters the terminal may send instead of them do too.
        // Alt-Enter arrives as escape and return, and opens a line as Enter
        // does, which is what the terminal without the binding swallows.
        for key in [
            KeyEvent(KeyCode::Enter, Modifiers::NONE),
            KeyEvent(KeyCode::Enter, Modifiers::ALT),
            KeyEvent(KeyCode::Char('M'), Modifiers::CTRL),
            KeyEvent(KeyCode::Char('J'), Modifiers::CTRL),
        ] {
            editor.bind_sequence(key, EventHandler::Conditional(Box::new(AutoIndentHandler)));
        }
        for key in [
            KeyEvent(KeyCode::Backspace, Modifiers::NONE),
            KeyEvent(KeyCode::Char('H'), Modifiers::CTRL),
        ] {
            editor.bind_sequence(key, EventHandler::Conditional(Box::new(SmartBackspace)));
        }

        let history = history_path();
        if let Some(history) = &history {
            let _ = editor.load_history(history);
        }

        Ok(ReplReader {
            editor,
            history,
            done: false,
        })
    }
}

struct ReplPrompt;

impl rustyline::Prompt for ReplPrompt {
    fn raw(&self) -> &str {
        ">>> "
    }

    fn continuation_raw(&self) -> &str {
        "... "
    }
}

impl Iterator for ReplReader {
    type Item = String;

    fn next(&mut self) -> Option<String> {
        loop {
            if self.done {
                return None;
            }
            match self.editor.readline(&ReplPrompt) {
                Ok(input) => {
                    let _ = self.editor.add_history_entry(input.trim_end());
                    return Some(input);
                }
                // Ctrl-C throws the input away, so nothing stays pending and
                // nothing is run. The next read starts a fresh input.
                Err(ReadlineError::Interrupted) => {
                    take_pending(&self.editor);
                    eprintln!("KeyboardInterrupt");
                }
                Err(err) => {
                    if !matches!(err, ReadlineError::Eof) {
                        eprintln!("Readline error: {err:?}");
                    }
                    self.done = true;
                    // An input that the file cut off still runs, as the user
                    // wants to read what is wrong with it.
                    let pending = take_pending(&self.editor);
                    return (!pending.trim().is_empty()).then_some(pending);
                }
            }
        }
    }
}

impl Drop for ReplReader {
    /// The history outlives the session, whatever ended it: the end of the
    /// input, `:quit`, or an `exit()` inside an input.
    fn drop(&mut self) {
        if let Some(history) = &self.history {
            let _ = self.editor.save_history(history);
        }
    }
}

fn take_pending(editor: &Editor<InputHelper, FileHistory>) -> String {
    editor
        .helper()
        .map_or_else(String::new, |helper| helper.pending.take())
}

fn history_path() -> Option<PathBuf> {
    dirs::data_dir().map(|mut p| {
        p.push(HISTORY_DIR);
        let _ = std::fs::create_dir_all(&p);
        p.push(HISTORY_FILE);
        p
    })
}

struct InputHelper {
    /// The shell, which completes a name with the interpreter the repl owns.
    /// The editor holds it only while it is drawing a line, and the loop only
    /// while it is running one, so the two never ask for it at once.
    shell: Rc<RefCell<Shell>>,
    /// The shell's commands: every name, and whether it takes an argument.
    commands: Vec<(&'static str, bool)>,
    /// The unfinished input the editor is still reading for. Ctrl-C abandons
    /// it; an input that runs out is run anyway, so the user is shown what is
    /// wrong with it instead of nothing at all.
    pending: RefCell<String>,
    /// Whether the terminal takes colour.
    color: bool,
    /// The theme the colours come from, which `:theme` writes and the next
    /// line the editor draws reads.
    theme: Rc<Cell<Theme>>,
}

impl Completer for InputHelper {
    type Candidate = String;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &Context,
    ) -> rustyline::Result<(usize, Vec<String>)> {
        match self.shell.borrow().complete(line, pos) {
            TabAction::Indent(spaces) => Ok((pos, vec![spaces])),
            TabAction::Complete(startpos, candidates) => Ok((startpos, candidates)),
            TabAction::Nothing => Ok((pos, vec![])),
        }
    }
}

impl Hinter for InputHelper {
    type Hint = String;
}

impl Highlighter for InputHelper {
    fn highlight<'l>(&self, line: &'l str, _pos: usize) -> std::borrow::Cow<'l, str> {
        if !self.color || line.is_empty() {
            return std::borrow::Cow::Borrowed(line);
        }
        std::borrow::Cow::Owned(highlight_python(
            line,
            &self.commands,
            self.theme.get().palette(),
        ))
    }

    fn highlight_prompt<'b, 's: 'b, 'p: 'b>(
        &'s self,
        prompt: &'p str,
        _default: bool,
    ) -> std::borrow::Cow<'b, str> {
        if !self.color {
            return std::borrow::Cow::Borrowed(prompt);
        }
        let t = self.theme.get().palette();
        std::borrow::Cow::Owned(format!("{}{prompt}{RESET}", t.prompt))
    }

    fn highlight_char(
        &self,
        _line: &str,
        _pos: usize,
        _forced: rustyline::highlight::CmdKind,
    ) -> bool {
        self.color
    }
}

impl Validator for InputHelper {
    fn validate(&self, ctx: &mut ValidationContext) -> rustyline::Result<ValidationResult> {
        if engine::shell::ready_state(ctx.input(), ctx.input().len()) < 0 {
            self.pending.borrow_mut().clear();
            Ok(ValidationResult::Valid(None))
        } else {
            *self.pending.borrow_mut() = ctx.input().to_owned();
            Ok(ValidationResult::Incomplete)
        }
    }
}

impl rustyline::Helper for InputHelper {}

/// What Enter does with the input as it stands: `None` accepts it, and
/// `Some(indent)` opens a line that far in.
///
/// Where the cursor is is part of the question, and the engine answers all of
/// it: the prompt in the browser asks the same thing through `repl_ready`, so
/// the two read the key the same way.
fn enter_action(line: &str, pos: usize) -> Option<usize> {
    match engine::shell::ready_state(line, pos) {
        -1 => None,
        indent => Some(usize::try_from(indent).unwrap_or(0)),
    }
}

/// Enter: a line accepted, or a line opened at the caret.
struct AutoIndentHandler;

impl ConditionalEventHandler for AutoIndentHandler {
    fn handle(
        &self,
        _evt: &rustyline::Event,
        _n: RepeatCount,
        _positive: bool,
        ctx: &EventContext,
    ) -> Option<Cmd> {
        enter_action(ctx.line(), ctx.pos())
            .map(|indent| Cmd::Insert(1, format!("\n{:indent$}", "")))
    }
}

/// Backspace takes back a whole level of indentation on a line with only
/// spaces before the caret.
struct SmartBackspace;

impl ConditionalEventHandler for SmartBackspace {
    fn handle(
        &self,
        _evt: &rustyline::Event,
        _n: RepeatCount,
        _positive: bool,
        ctx: &EventContext,
    ) -> Option<Cmd> {
        let before = &ctx.line()[..ctx.pos()];
        let current_line = &before[before.rfind('\n').map_or(0, |i| i + 1)..];
        if current_line.len() > 1 && current_line.bytes().all(|b| b == b' ') {
            let spaces = current_line.len();
            let remove = if spaces.is_multiple_of(INDENT) {
                INDENT
            } else {
                spaces % INDENT
            };
            Some(Cmd::Kill(Movement::BackwardChar(remove as u16)))
        } else {
            None
        }
    }
}

// ── Syntax highlighting ─────────────────────────────────────────────

const RESET: &str = "\x1b[0m";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Theme {
    Dark,
    Light,
}

impl Theme {
    pub fn parse(name: &str) -> Option<Theme> {
        match name {
            "dark" => Some(Theme::Dark),
            "light" => Some(Theme::Light),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Theme::Dark => "dark",
            Theme::Light => "light",
        }
    }

    fn palette(self) -> &'static Palette {
        match self {
            Theme::Dark => &ONE_DARK,
            Theme::Light => &ONE_LIGHT,
        }
    }
}

struct Palette {
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

const ONE_DARK: Palette = Palette {
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

const ONE_LIGHT: Palette = Palette {
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

/// The scanner walks `line` byte by byte and only stops on a char boundary, so
/// every cut it asks for is one.
fn cut<I>(line: &str, range: I) -> &str
where
    I: std::slice::SliceIndex<str, Output = str>,
{
    line.get(range)
        .expect("the scanner cuts at a char boundary")
}

fn highlight_python(line: &str, commands: &[(&'static str, bool)], t: &Palette) -> String {
    let b = line.as_bytes();
    let mut out = String::with_capacity(b.len() + 64);
    let mut i = 0;

    // A `:` command is coloured as a keyword and what follows it as Python.
    // Reading the head with the engine's own `split` keeps the highlighter
    // agreeing with the dispatch about where a command name ends.
    if let Some((name, cmd_start)) = engine::shell::split(line)
        .map(|(name, _)| name)
        .filter(|name| commands.iter().any(|(c, _)| c == name))
        .and_then(|name| line.find(name).map(|start| (name, start)))
    {
        out.push_str(cut(line, ..cmd_start));
        out.push_str(t.keyword);
        out.push_str(name);
        out.push_str(RESET);
        let rest = cut(line, cmd_start + name.len()..);
        if !rest.is_empty() {
            out.push_str(&highlight_python(rest, commands, t));
        }
        return out;
    }

    while let Some(&c) = b.get(i) {
        match c {
            b'#' => {
                out.push_str(t.comment);
                out.push_str(cut(line, i..));
                out.push_str(RESET);
                return out;
            }
            b'\'' | b'"' => {
                let start = i;
                i = skip_string(b, i);
                out.push_str(t.string);
                out.push_str(cut(line, start..i));
                out.push_str(RESET);
            }
            b'0'..=b'9' => {
                let start = i;
                i = skip_number(b, i);
                out.push_str(t.number);
                out.push_str(cut(line, start..i));
                out.push_str(RESET);
            }
            b'.' if b.get(i + 1).is_some_and(u8::is_ascii_digit) => {
                let start = i;
                i = skip_number(b, i);
                out.push_str(t.number);
                out.push_str(cut(line, start..i));
                out.push_str(RESET);
            }
            b'a'..=b'z' | b'A'..=b'Z' | b'_' => {
                let start = i;
                while b
                    .get(i)
                    .is_some_and(|c| c.is_ascii_alphanumeric() || *c == b'_')
                {
                    i += 1;
                }
                let word = cut(line, start..i);
                if b.get(i).is_some_and(|c| *c == b'\'' || *c == b'"') && is_string_prefix(word) {
                    let str_end = skip_string(b, i);
                    out.push_str(t.string);
                    out.push_str(cut(line, start..str_end));
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
                while b
                    .get(i)
                    .is_some_and(|c| c.is_ascii_alphanumeric() || *c == b'_' || *c == b'.')
                {
                    i += 1;
                }
                out.push_str(t.decorator);
                out.push_str(cut(line, start..i));
                out.push_str(RESET);
            }
            _ => match cut(line, i..).chars().next() {
                Some(ch) => {
                    out.push(ch);
                    i += ch.len_utf8();
                }
                None => break,
            },
        }
    }

    out
}

fn skip_string(b: &[u8], mut i: usize) -> usize {
    let len = b.len();
    let Some(&quote) = b.get(i) else {
        return len;
    };
    let triple = |b: &[u8], i: usize| {
        b.get(i) == Some(&quote) && b.get(i + 1) == Some(&quote) && b.get(i + 2) == Some(&quote)
    };
    if triple(b, i) {
        i += 3;
        while i + 2 < len {
            if b.get(i) == Some(&b'\\') {
                i += 2;
                continue;
            }
            if triple(b, i) {
                return i + 3;
            }
            i += 1;
        }
        return len;
    }
    i += 1;
    while let Some(&c) = b.get(i) {
        if c == b'\\' && i + 1 < len {
            i += 2;
            continue;
        }
        if c == quote {
            return i + 1;
        }
        i += 1;
    }
    len
}

fn skip_number(b: &[u8], mut i: usize) -> usize {
    let digits = |b: &[u8], i: &mut usize| {
        while b.get(*i).is_some_and(|c| c.is_ascii_digit() || *c == b'_') {
            *i += 1;
        }
    };
    if b.get(i) == Some(&b'.') {
        i += 1;
        digits(b, &mut i);
        return skip_exponent(b, i);
    }
    if b.get(i) == Some(&b'0') {
        let radix: Option<fn(&u8) -> bool> = match b.get(i + 1) {
            Some(b'x' | b'X') => Some(u8::is_ascii_hexdigit),
            Some(b'o' | b'O') => Some(|c| (b'0'..=b'7').contains(c)),
            Some(b'b' | b'B') => Some(|c| *c == b'0' || *c == b'1'),
            _ => None,
        };
        if let Some(is_digit) = radix {
            i += 2;
            while b.get(i).is_some_and(|c| is_digit(c) || *c == b'_') {
                i += 1;
            }
            return i;
        }
    }
    digits(b, &mut i);
    if b.get(i) == Some(&b'.') {
        i += 1;
        digits(b, &mut i);
    }
    skip_exponent(b, i)
}

fn skip_exponent(b: &[u8], mut i: usize) -> usize {
    if b.get(i).is_some_and(|c| *c == b'e' || *c == b'E') {
        i += 1;
        if b.get(i).is_some_and(|c| *c == b'+' || *c == b'-') {
            i += 1;
        }
        while b.get(i).is_some_and(|c| c.is_ascii_digit() || *c == b'_') {
            i += 1;
        }
    }
    if b.get(i).is_some_and(|c| *c == b'j' || *c == b'J') {
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
        // And so does the last line itself with the cursor back in it: the
        // key is being pressed to correct `y`, not to run the two lines.
        assert_eq!(enter_action("x = 1\ny = 2", 10), Some(0));
    }
}
