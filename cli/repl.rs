use rustpython_vm::{
    AsObject, PyResult, TryFromObject, VirtualMachine,
    builtins::{PyBaseExceptionRef, PyDictRef, PyStrRef},
    compiler,
    function::ArgIterable,
    identifier,
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

// ── Execution ─────────────────────────────────────────────────

fn repl_exec(vm: &VirtualMachine, source: &str, scope: Scope) -> Result<(), PyBaseExceptionRef> {
    match vm.compile(source, compiler::Mode::Single, "<stdin>".to_owned()) {
        Ok(code) => vm.run_code_obj(code, scope).map(|_| ()),
        Err(err) => Err(vm.new_syntax_error(&err, Some(source))),
    }
}

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

    fn get_available_completions<'w>(
        &self,
        words: &'w [String],
    ) -> Option<(&'w str, impl Iterator<Item = PyResult<PyStrRef>> + 'vm)> {
        let (first, rest) = words.split_first().unwrap();

        let str_iter_method = |obj, name| {
            let iter = self.vm.call_special_method(obj, name, ())?;
            ArgIterable::<PyStrRef>::try_from_object(self.vm, iter)?.iter(self.vm)
        };

        let (word_start, iter1, iter2) = if let Some((last, parents)) = rest.split_last() {
            let mut current = self
                .globals
                .get_item_opt(first.as_str(), self.vm)
                .ok()
                .flatten()
                .or_else(|| {
                    let attr = self.vm.ctx.new_str(first.as_str());
                    self.vm.builtins.get_attr(&attr, self.vm).ok()
                })?;

            for attr in parents {
                let attr = self.vm.ctx.new_str(attr.as_str());
                current = current.get_attr(&attr, self.vm).ok()?;
            }

            let current_iter = str_iter_method(&current, identifier!(self.vm, __dir__)).ok()?;

            (last, current_iter, None)
        } else {
            let globals =
                str_iter_method(self.globals.as_object(), identifier!(self.vm, keys)).ok()?;
            let builtins =
                str_iter_method(self.vm.builtins.as_object(), identifier!(self.vm, __dir__))
                    .ok()?;
            (first, globals, Some(builtins))
        };
        Some((word_start, iter1.chain(iter2.into_iter().flatten())))
    }

    fn complete_opt(&self, line: &str) -> Option<(usize, Vec<String>)> {
        let (startpos, words) = if line.is_empty() {
            (0, vec![String::new()])
        } else {
            split_idents_on_dot(line)?
        };

        let (word_start, iter) = self.get_available_completions(&words)?;

        let all_completions: Vec<_> = iter
            .filter_map(|res| res.ok())
            .filter(|s| s.as_bytes().starts_with(word_start.as_bytes()))
            .collect();
        let completions = if word_start.starts_with('_') {
            all_completions
        } else {
            let no_underscore = all_completions
                .iter()
                .filter(|&s| !s.as_bytes().starts_with(b"_"))
                .cloned()
                .collect::<Vec<_>>();

            if no_underscore.is_empty() {
                all_completions
            } else {
                no_underscore
            }
        };

        let mut result: Vec<String> = completions
            .into_iter()
            .map(|s| s.expect_str().to_owned())
            .collect();

        // Add keyword completions for top-level (no dot).
        if words.len() == 1 {
            for kw in PYTHON_KEYWORDS {
                if kw.starts_with(word_start) && !result.contains(&kw.to_string()) {
                    result.push(kw.to_string());
                }
            }
        }

        result.sort();

        Some((startpos, result))
    }
}

fn reverse_string(s: &mut String) {
    let rev = s.chars().rev().collect();
    *s = rev;
}

fn split_idents_on_dot(line: &str) -> Option<(usize, Vec<String>)> {
    let mut words = vec![String::new()];
    let mut startpos = 0;
    for (i, c) in line.chars().rev().enumerate() {
        match c {
            '.' => {
                if i != 0 && words.last().is_some_and(|s| s.is_empty()) {
                    return None;
                }
                reverse_string(words.last_mut().unwrap());
                if words.len() == 1 {
                    startpos = line.len() - i;
                }
                words.push(String::new());
            }
            c if c.is_alphanumeric() || c == '_' => words.last_mut().unwrap().push(c),
            _ => {
                if words.len() == 1 {
                    if words.last().unwrap().is_empty() {
                        return None;
                    }
                    startpos = line.len() - i;
                }
                break;
            }
        }
    }
    if words == [String::new()] {
        return None;
    }
    reverse_string(words.last_mut().unwrap());
    words.reverse();

    Some((startpos, words))
}

impl Completer for ReplHelper<'_> {
    type Candidate = String;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &Context,
    ) -> rustyline::Result<(usize, Vec<String>)> {
        let before = &line[..pos];
        let current_line_start = before.rfind('\n').map_or(0, |i| i + 1);
        let current_line = &before[current_line_start..];

        // If current line is empty or whitespace-only, decide: indent or complete.
        if current_line.bytes().all(|b| b == b' ') {
            let current_indent = current_line.len();
            let expected = expected_indent(&before[..current_line_start]);
            if current_indent < expected {
                // Under-indented → insert spaces to reach expected level.
                let spaces = " ".repeat(expected - current_indent);
                return Ok((pos, vec![spaces]));
            }
            // At or above expected indent → show all completions.
            return Ok(self
                .complete_opt("")
                .map(|(_, candidates)| (pos, candidates))
                .unwrap_or_else(|| (pos, vec![])));
        }

        Ok(self.complete_opt(before).unwrap_or_else(|| (pos, vec![])))
    }
}

/// Compute the expected indentation level based on previous lines.
/// If the previous line ends with ':', expects indent + 4.
/// Otherwise, expects the same indent as the previous line.
/// Returns 0 if there are no previous lines (top-level).
fn expected_indent(lines_before: &str) -> usize {
    let prev_line = lines_before.lines().rev().find(|l| !l.trim().is_empty());
    match prev_line {
        None => 0,
        Some(line) => {
            let indent = line.len() - line.trim_start().len();
            if line.trim_end().ends_with(':') {
                indent + 4
            } else {
                indent
            }
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

const RESET: &str = "\x1b[0m";
const COLOR_COMMENT: &str = "\x1b[90m";
const COLOR_STRING: &str = "\x1b[32m";
const COLOR_NUMBER: &str = "\x1b[33m";
const COLOR_KEYWORD: &str = "\x1b[35m";
const COLOR_BUILTIN: &str = "\x1b[36m";
const COLOR_DECORATOR: &str = "\x1b[34m";

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

const PYTHON_KEYWORDS: &[&str] = &[
    "False", "None", "True", "and", "as", "assert", "async", "await", "break", "case", "class",
    "continue", "def", "del", "elif", "else", "except", "finally", "for", "from", "global", "if",
    "import", "in", "is", "lambda", "match", "nonlocal", "not", "or", "pass", "raise", "return",
    "try", "while", "with", "yield",
];

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

pub fn run_repl(vm: &VirtualMachine, scope: Scope) -> PyResult<()> {
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

    let repl_history_path = match dirs::config_dir() {
        Some(mut path) => {
            path.push("spython");
            path.push("repl_history.txt");
            path
        }
        None => ".repl_history.txt".into(),
    };

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
        let result = match repl.readline(&prompt) {
            Ok(line) => {
                let _ = repl.add_history_entry(line.trim_end());

                let source = format!("{line}\n");
                match repl_exec(vm, &source, scope.clone()) {
                    Ok(()) => Ok(()),
                    Err(err) => Err(err),
                }
            }
            Err(rustyline::error::ReadlineError::Interrupted) => {
                let keyboard_interrupt =
                    vm.new_exception_empty(vm.ctx.exceptions.keyboard_interrupt.to_owned());
                Err(keyboard_interrupt)
            }
            Err(rustyline::error::ReadlineError::Eof) => {
                break;
            }
            Err(err) => {
                eprintln!("Readline error: {err:?}");
                break;
            }
        };

        if let Err(exc) = result {
            if exc.fast_isinstance(vm.ctx.exceptions.system_exit) {
                let _ = repl.save_history(&repl_history_path);
                return Err(exc);
            }
            vm.print_exception(exc);
        }
    }
    let _ = repl.save_history(&repl_history_path);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustyline::completion::Completer;

    fn with_helper(f: impl FnOnce(&ReplHelper<'_>)) {
        let interp = engine::new_interpreter();
        interp.enter(|vm| {
            let scope = vm.new_scope_with_main().unwrap();
            let helper = ReplHelper::new(vm, scope.globals.clone());
            f(&helper);
        });
    }

    fn ctx() -> Context<'static> {
        // Leak to get 'static lifetime for tests.
        let history = Box::leak(Box::new(rustyline::history::DefaultHistory::new()));
        Context::new(history)
    }

    #[test]
    fn complete_builtin_name() {
        with_helper(|h| {
            let (start, candidates) = h.complete("pri", 3, &ctx()).unwrap();
            assert_eq!(start, 0);
            assert!(
                candidates.contains(&"print".to_owned()),
                "expected 'print' in {candidates:?}"
            );
        });
    }

    #[test]
    fn complete_keyword() {
        with_helper(|h| {
            let (start, candidates) = h.complete("de", 2, &ctx()).unwrap();
            assert_eq!(start, 0);
            assert!(
                candidates.contains(&"def".to_owned()),
                "expected 'def' in {candidates:?}"
            );
        });
    }

    #[test]
    fn complete_keyword_for() {
        with_helper(|h| {
            let (start, candidates) = h.complete("fo", 2, &ctx()).unwrap();
            assert_eq!(start, 0);
            assert!(
                candidates.contains(&"for".to_owned()),
                "expected 'for' in {candidates:?}"
            );
        });
    }

    #[test]
    fn complete_builtin_attribute() {
        with_helper(|h| {
            let (start, candidates) = h.complete("str.up", 6, &ctx()).unwrap();
            assert_eq!(start, 4);
            assert!(
                candidates.contains(&"upper".to_owned()),
                "expected 'upper' in {candidates:?}"
            );
        });
    }

    #[test]
    fn complete_empty_line_shows_globals_and_builtins() {
        with_helper(|h| {
            let (start, candidates) = h.complete("", 0, &ctx()).unwrap();
            assert_eq!(start, 0);
            assert!(
                candidates.contains(&"print".to_owned()),
                "expected 'print' in {candidates:?}"
            );
            assert!(
                candidates.contains(&"int".to_owned()),
                "expected 'int' in {candidates:?}"
            );
        });
    }

    #[test]
    fn complete_multiline_under_indented_indents() {
        with_helper(|h| {
            // After ':', current indent 0, expected 4 → indent
            let ctx = &ctx();
            let (start, candidates) = h.complete("def f():\n", 9, ctx).unwrap();
            assert_eq!(start, 9);
            assert_eq!(candidates, vec!["    "]);
        });
    }

    #[test]
    fn complete_multiline_partially_indented_indents() {
        with_helper(|h| {
            // After ':', current indent 2, expected 4 → indent 2 more
            let ctx = &ctx();
            let (start, candidates) = h.complete("def f():\n  ", 11, ctx).unwrap();
            assert_eq!(start, 11);
            assert_eq!(candidates, vec!["  "]);
        });
    }

    #[test]
    fn complete_multiline_correctly_indented_completes() {
        with_helper(|h| {
            // After ':', current indent 4, expected 4 → complete
            let ctx = &ctx();
            let (_, candidates) = h.complete("def f():\n    ", 13, ctx).unwrap();
            assert!(
                candidates.contains(&"print".to_owned()),
                "expected completions, got {candidates:?}"
            );
        });
    }

    #[test]
    fn complete_multiline_continuation_under_indented_indents() {
        with_helper(|h| {
            // Continuation: previous line at indent 4, current at 0 → indent
            let ctx = &ctx();
            let (start, candidates) = h.complete("def f():\n    x = 1\n", 19, ctx).unwrap();
            assert_eq!(start, 19);
            assert_eq!(candidates, vec!["    "]);
        });
    }

    #[test]
    fn complete_multiline_continuation_correctly_indented_completes() {
        with_helper(|h| {
            // Continuation: previous line at indent 4, current at 4 → complete
            let ctx = &ctx();
            let (_, candidates) = h.complete("def f():\n    x = 1\n    ", 23, ctx).unwrap();
            assert!(
                candidates.contains(&"print".to_owned()),
                "expected completions, got {candidates:?}"
            );
        });
    }

    #[test]
    fn complete_single_line_whitespace_completes() {
        with_helper(|h| {
            // Single line with spaces → complete (no block context)
            let ctx = &ctx();
            let (_, candidates) = h.complete("    ", 4, ctx).unwrap();
            assert!(
                candidates.contains(&"print".to_owned()),
                "expected completions, got {candidates:?}"
            );
        });
    }
}
