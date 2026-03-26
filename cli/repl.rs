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
        if let Some(k) = evt.get(0) {
            match k {
                KeyEvent(KeyCode::Enter, _)
                | KeyEvent(KeyCode::Char('M' | 'J'), Modifiers::CTRL) => KeyKind::Enter,
                KeyEvent(KeyCode::Backspace, _) | KeyEvent(KeyCode::Char('H'), Modifiers::CTRL) => {
                    KeyKind::Backspace
                }
                _ => KeyKind::Other,
            }
        } else {
            KeyKind::Other
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
        std::borrow::Cow::Owned(format!("\x1b[34m{prompt}\x1b[0m"))
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
    for line in input.lines() {
        let trimmed = line.trim();
        if trimmed.ends_with(':') && !trimmed.starts_with('#') {
            return true;
        }
    }
    false
}

fn is_incomplete_error(err: &rustpython_compiler::CompileError) -> bool {
    use rustpython_compiler::{
        CompileError, ParseError,
        parser::{InterpolatedStringErrorType, LexicalErrorType, ParseErrorType},
    };
    match err {
        CompileError::Parse(ParseError {
            error: ParseErrorType::Lexical(LexicalErrorType::Eof),
            ..
        }) => true,
        CompileError::Parse(ParseError {
            error:
                ParseErrorType::Lexical(LexicalErrorType::FStringError(
                    InterpolatedStringErrorType::UnterminatedTripleQuotedString,
                )),
            ..
        }) => true,
        CompileError::Parse(ParseError {
            error: ParseErrorType::Lexical(LexicalErrorType::UnclosedStringError),
            ..
        }) => true,
        CompileError::Parse(ParseError {
            error: ParseErrorType::OtherError(msg),
            ..
        }) => msg.starts_with("Expected an indented block"),
        _ => false,
    }
}

// ── Syntax highlighting ─────────────────────────────────────────────

// Colors from the Zed One Dark theme.
const RESET: &str = "\x1b[0m";
const COLOR_COMMENT: &str = "\x1b[38;2;93;99;111m"; // #5d636f
const COLOR_STRING: &str = "\x1b[38;2;161;193;129m"; // #a1c181
const COLOR_NUMBER: &str = "\x1b[38;2;191;149;106m"; // #bf956a
const COLOR_KEYWORD: &str = "\x1b[38;2;180;119;207m"; // #b477cf
const COLOR_BUILTIN: &str = "\x1b[38;2;115;173;233m"; // #73ade9
const COLOR_BOOLEAN: &str = "\x1b[38;2;191;149;106m"; // #bf956a (same as number)
const COLOR_CONSTANT: &str = "\x1b[38;2;223;193;132m"; // #dfc184
const COLOR_DECORATOR: &str = "\x1b[38;2;116;174;232m"; // #74ade8

fn highlight_python(line: &str) -> String {
    let b = line.as_bytes();
    let len = b.len();
    let mut out = String::with_capacity(len + 64);
    let mut i = 0;

    while i < len {
        match b[i] {
            b'#' => {
                out.push_str(COLOR_COMMENT);
                out.push_str(&line[i..]);
                out.push_str(RESET);
                return out;
            }
            b'\'' | b'"' => {
                let start = i;
                i = skip_string(b, i);
                out.push_str(COLOR_STRING);
                out.push_str(&line[start..i]);
                out.push_str(RESET);
            }
            b'0'..=b'9' => {
                let start = i;
                i = skip_number(b, i);
                out.push_str(COLOR_NUMBER);
                out.push_str(&line[start..i]);
                out.push_str(RESET);
            }
            b'.' if i + 1 < len && b[i + 1].is_ascii_digit() => {
                let start = i;
                i = skip_number(b, i);
                out.push_str(COLOR_NUMBER);
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
                    out.push_str(COLOR_STRING);
                    out.push_str(&line[start..str_end]);
                    out.push_str(RESET);
                    i = str_end;
                } else if is_python_boolean(word) {
                    out.push_str(COLOR_BOOLEAN);
                    out.push_str(word);
                    out.push_str(RESET);
                } else if is_python_constant(word) {
                    out.push_str(COLOR_CONSTANT);
                    out.push_str(word);
                    out.push_str(RESET);
                } else if is_python_keyword(word) {
                    out.push_str(COLOR_KEYWORD);
                    out.push_str(word);
                    out.push_str(RESET);
                } else if is_python_builtin(word) {
                    out.push_str(COLOR_BUILTIN);
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
                out.push_str(COLOR_DECORATOR);
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

fn is_python_boolean(word: &str) -> bool {
    PYTHON_BOOLEANS.contains(&word)
}

fn is_python_constant(word: &str) -> bool {
    PYTHON_CONSTANTS.contains(&word)
}

fn is_python_keyword(word: &str) -> bool {
    PYTHON_KEYWORDS.contains(&word)
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

pub fn run_repl(vm: &VirtualMachine, scope: Scope, level: Level) -> PyResult<()> {
    let repl_history_path = match dirs::config_dir() {
        Some(mut path) => {
            path.push("spython");
            path.push("repl_history.txt");
            path
        }
        None => ".repl_history.txt".into(),
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
                    let source = format!("{line}\n");

                    // Type-check with accumulated context.
                    let use_color = std::io::IsTerminal::is_terminal(&std::io::stderr());
                    if !engine::type_check_repl_input(
                        &accumulated_source,
                        &source,
                        level,
                        use_color,
                    ) {
                        continue;
                    }

                    // Execute.
                    match repl_exec(vm, &source, scope.clone()) {
                        Ok(_) => {
                            if !accumulated_source.is_empty() {
                                accumulated_source.push('\n');
                            }
                            accumulated_source.push_str(&source);
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
