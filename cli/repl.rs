use std::io::IsTerminal;
use std::time::Instant;

use engine::Level;
use engine::completion::{self, PYTHON_BOOLEANS, PYTHON_CONSTANTS, PYTHON_KEYWORDS, TabAction};
use rustpython_vm::{
    AsObject, PyResult, VirtualMachine,
    builtins::{PyBaseExceptionRef, PyDictRef},
    compiler,
    scope::Scope,
};
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

fn is_dedent_keyword(trimmed: &str) -> bool {
    let word = trimmed.split_whitespace().next().unwrap_or("");
    matches!(word, "return" | "pass" | "break" | "continue" | "raise")
}

impl rustyline::ConditionalEventHandler for SmartKeys {
    fn handle(
        &self,
        evt: &Event,
        _n: rustyline::RepeatCount,
        _positive: bool,
        ctx: &rustyline::EventContext<'_>,
    ) -> Option<Cmd> {
        let line = ctx.line();
        let pos = ctx.pos();
        let before = &line[..pos];
        let line_start = before.rfind('\n').map_or(0, |i| i + 1);
        let current_line = &before[line_start..];

        match Self::key_kind(evt) {
            KeyKind::Enter => {
                let trimmed = current_line.trim_end();
                let indent: String = current_line.chars().take_while(|c| *c == ' ').collect();

                if trimmed.ends_with(':') {
                    return Some(Cmd::Insert(1, format!("\n{indent}    ")));
                }

                if indent.len() >= 4 && is_dedent_keyword(trimmed) {
                    let dedented = &indent[4..];
                    return Some(Cmd::Insert(1, format!("\n{dedented}")));
                }

                if line.contains('\n') && !trimmed.is_empty() {
                    return Some(Cmd::Insert(1, format!("\n{indent}")));
                }

                None
            }
            KeyKind::Backspace => {
                if current_line.len() > 1 && current_line.bytes().all(|b| b == b' ') {
                    let spaces = current_line.len();
                    let remove = if spaces.is_multiple_of(4) {
                        4
                    } else {
                        spaces % 4
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
}

impl<'vm> ReplHelper<'vm> {
    const fn new(vm: &'vm VirtualMachine, globals: PyDictRef) -> Self {
        ReplHelper { vm, globals }
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
        match completion::tab_action(self.vm, &self.globals, line, pos) {
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
        std::borrow::Cow::Owned(highlight_python(line))
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
        let input = ctx.input();

        if input.trim().is_empty() {
            return Ok(ValidationResult::Valid(None));
        }

        if input.ends_with('\n') && ctx.cursor_at_end() {
            return Ok(ValidationResult::Valid(None));
        }

        let source = format!("{input}\n");
        match self
            .vm
            .compile(&source, compiler::Mode::Single, "<stdin>".to_owned())
        {
            Ok(_) => {
                if input_has_block(input) {
                    Ok(ValidationResult::Incomplete)
                } else {
                    Ok(ValidationResult::Valid(None))
                }
            }
            Err(err) => {
                if is_incomplete_error(&err) {
                    Ok(ValidationResult::Incomplete)
                } else {
                    Ok(ValidationResult::Valid(None))
                }
            }
        }
    }
}

impl rustyline::Helper for ReplHelper<'_> {}

fn input_has_block(input: &str) -> bool {
    input
        .lines()
        .any(|line| line.trim().ends_with(':') && !line.trim().starts_with('#'))
}

fn is_incomplete_error(err: &rustpython_compiler::CompileError) -> bool {
    use rustpython_compiler::{
        CompileError, ParseError,
        parser::{InterpolatedStringErrorType, LexicalErrorType, ParseErrorType},
    };
    match err {
        CompileError::Parse(ParseError {
            error:
                ParseErrorType::Lexical(
                    LexicalErrorType::Eof
                    | LexicalErrorType::UnclosedStringError
                    | LexicalErrorType::FStringError(
                        InterpolatedStringErrorType::UnterminatedTripleQuotedString,
                    ),
                ),
            ..
        }) => true,
        CompileError::Parse(ParseError {
            error: ParseErrorType::OtherError(msg),
            ..
        }) => msg.starts_with("Expected an indented block"),
        _ => false,
    }
}

fn format_elapsed(elapsed: std::time::Duration) -> String {
    if elapsed.as_secs() > 0 {
        format!("{:.2} s", elapsed.as_secs_f64())
    } else if elapsed.as_millis() > 0 {
        format!("{} ms", elapsed.as_millis())
    } else if elapsed.as_micros() > 0 {
        format!("{} us", elapsed.as_micros())
    } else {
        format!("{} ns", elapsed.as_nanos())
    }
}

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

fn highlight_python(line: &str) -> String {
    let t = current_theme();
    let b = line.as_bytes();
    let len = b.len();
    let mut out = String::with_capacity(len + 64);
    let mut i = 0;

    // REPL commands: color the command keyword, highlight the argument normally.
    let trimmed = line.trim();
    for cmd in &[":help", ":quit", ":level", ":type", ":time", ":theme"] {
        if trimmed
            .strip_prefix(cmd)
            .is_some_and(|rest| rest.is_empty() || rest.starts_with(' '))
        {
            let cmd_start = line.find(cmd).unwrap();
            out.push_str(&line[..cmd_start]);
            out.push_str(t.keyword);
            out.push_str(cmd);
            out.push_str(RESET);
            let rest = &line[cmd_start + cmd.len()..];
            if !rest.is_empty() {
                out.push_str(&highlight_python(rest));
            }
            return out;
        }
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

fn repl_exec(vm: &VirtualMachine, source: &str, scope: Scope) -> Result<(), PyBaseExceptionRef> {
    match vm.compile(source, compiler::Mode::Single, "<stdin>".to_owned()) {
        Ok(code) => vm.run_code_obj(code, scope).map(|_| ()),
        Err(err) => Err(vm.new_syntax_error(&err, Some(source))),
    }
}

pub fn run_repl(vm: &VirtualMachine, scope: Scope, mut level: Level) -> PyResult<()> {
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

        repl.set_helper(Some(ReplHelper::new(vm, scope.globals.clone())));
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
        let mut accumulated_source = String::new();

        loop {
            match repl.readline(&prompt) {
                Ok(line) => {
                    let _ = repl.add_history_entry(line.trim_end());
                    let trimmed = line.trim();

                    // REPL commands.
                    if trimmed == ":help" {
                        println!(
                            ":type <expr>    Show the type of an expression\n\
                             :time <expr>    Evaluate and show execution time\n\
                             :level [n]      Show or change the teaching level (0-5)\n\
                             :theme [name]   Show or change theme (light, dark)\n\
                             :quit           Exit the REPL"
                        );
                        continue;
                    }
                    if trimmed == ":quit" {
                        break;
                    }
                    if trimmed == ":level" {
                        println!("level {level}");
                        continue;
                    }
                    if let Some(n) = trimmed.strip_prefix(":level ") {
                        match n.trim().parse::<u8>().ok().and_then(Level::from_u8) {
                            Some(new_level) => {
                                // Re-check accumulated source at the new level.
                                if !accumulated_source.is_empty()
                                    && !engine::type_check_repl_input(
                                        "",
                                        &accumulated_source,
                                        new_level,
                                        std::io::stderr().is_terminal(),
                                    )
                                {
                                    eprintln!(
                                        "Cannot change to level {new_level}: \
                                         accumulated source has errors at that level."
                                    );
                                } else {
                                    level = new_level;
                                    println!("level {level}");
                                    crate::config::save(level, current_theme_name());
                                }
                            }
                            None => {
                                eprintln!("Invalid level: must be 0-5");
                            }
                        }
                        continue;
                    }
                    if trimmed == ":theme" {
                        let name = if USE_LIGHT_THEME.load(Ordering::Relaxed) {
                            "light"
                        } else {
                            "dark"
                        };
                        println!("{name}");
                        continue;
                    }
                    if let Some(name) = trimmed.strip_prefix(":theme ") {
                        match name.trim() {
                            "light" => {
                                set_theme(true);
                                println!("light");
                                crate::config::save(level, "light");
                            }
                            "dark" => {
                                set_theme(false);
                                println!("dark");
                                crate::config::save(level, "dark");
                            }
                            _ => eprintln!("Invalid theme: use 'light' or 'dark'"),
                        }
                        continue;
                    }
                    // :type is handled here and in engine::repl_run (for WASM).
                    if trimmed == ":type" {
                        eprintln!("Usage: :type <expression>");
                        continue;
                    }
                    if let Some(expr) = trimmed.strip_prefix(":type ") {
                        match engine::infer_expression_type(&accumulated_source, expr) {
                            Some(ty) => println!("{ty}"),
                            None => eprintln!("Could not infer type"),
                        }
                        continue;
                    }
                    // :time is handled here and in engine::repl_run (for WASM).
                    if trimmed == ":time" {
                        eprintln!("Usage: :time <expression>");
                        continue;
                    }

                    let (source, timed) = if let Some(expr) = trimmed.strip_prefix(":time ") {
                        if expr.trim().is_empty() {
                            eprintln!("Usage: :time <expression>");
                            continue;
                        }
                        (format!("{expr}\n"), true)
                    } else {
                        (format!("{line}\n"), false)
                    };

                    // Type-check with accumulated context.
                    let use_color = std::io::stderr().is_terminal();
                    if !engine::type_check_repl_input(
                        &accumulated_source,
                        &source,
                        level,
                        use_color,
                    ) {
                        continue;
                    }

                    // Execute.
                    let start = timed.then(Instant::now);
                    match repl_exec(vm, &source, scope.clone()) {
                        Ok(_) => {
                            if !accumulated_source.is_empty() {
                                accumulated_source.push('\n');
                            }
                            accumulated_source.push_str(&source);
                            if let Some(start) = start {
                                println!("Time: {}", format_elapsed(start.elapsed()));
                            }
                        }
                        Err(exc) => {
                            if exc.fast_isinstance(vm.ctx.exceptions.system_exit) {
                                let _ = repl.save_history(&repl_history_path);
                                return Err(exc);
                            }
                            vm.print_exception(exc);
                        }
                    }
                }
                Err(rustyline::error::ReadlineError::Interrupted) => {
                    let exc =
                        vm.new_exception_empty(vm.ctx.exceptions.keyboard_interrupt.to_owned());
                    vm.print_exception(exc);
                }
                Err(rustyline::error::ReadlineError::Eof) => {
                    break;
                }
                Err(err) => {
                    eprintln!("Readline error: {err:?}");
                    break;
                }
            }
        }
        let _ = repl.save_history(&repl_history_path);
    }

    Ok(())
}
