//! Doctest extraction, format validation, and type-check scaffolding.
//!
//! Walks the AST to find docstrings attached to the module, functions,
//! classes, and methods, and reads their examples with the grammar of the
//! runner in `doctest_runner.py`. A line that makes the runner refuse the
//! docstring, such as `>>>foo`, is reported here, where the student sees
//! where it is.
//!
//! A synthetic Python source can be assembled from a file's original text
//! plus one `def __spython_doctest_<N>__() -> None:` stub per snippet; that
//! synthetic source is then type-checked with the usual pipeline. Diagnostics
//! coming out of the synthetic functions are remapped back to the original
//! docstring locations via `SyntheticMap` / `remap_diagnostics`.

use std::borrow::Cow;

use ruff_db::diagnostic::{Annotation, Diagnostic, Span};
use ruff_db::files::File;
use ruff_python_ast::helpers::any_over_expr;
use ruff_python_ast::visitor::{Visitor, walk_stmt};
use ruff_python_ast::{Expr, ExprStringLiteral, ModModule, Stmt};
use ruff_python_parser::{parse_expression, parse_module};
use ruff_text_size::{Ranged, TextRange, TextSize};

use crate::checker::make_lint_diagnostic;

/// The source came in as one `String`, so it fits in the `u32` a `TextSize` holds.
fn text_size(offset: usize) -> TextSize {
    TextSize::try_from(offset).expect("a source under 4 GiB")
}

use crate::lints::{
    DOCTEST_DIRECTIVE, DOCTEST_INCONSISTENT_INDENTATION, DOCTEST_MALFORMED_PROMPT,
    DOCTEST_MULTIPLE_STATEMENTS, DOCTEST_NEVER_RUNS, DOCTEST_UNDER_NAME_CHECK,
};

/// Prefix/suffix used for synthetic doctest wrapper functions. The checker
/// recognizes functions whose name matches `__spython_doctest_<N>__` and
/// relaxes the `BARE_EXPRESSION` lint inside them (so `>>> x` is allowed).
///
/// A student who (very implausibly) writes a real function named
/// `__spython_doctest_0__` would have `BARE_EXPRESSION` suppressed inside it.
/// The dunder convention makes accidental collisions unlikely enough that we
/// accept the risk rather than thread an extra flag through the checker.
pub const SYNTHETIC_FN_PREFIX: &str = "__spython_doctest_";
pub const SYNTHETIC_FN_SUFFIX: &str = "__";

/// Returns `true` if `name` is one of the synthetic doctest wrapper
/// functions produced by `build_synthetic_source`.
pub fn is_synthetic_fn_name(name: &str) -> bool {
    let Some(rest) = name.strip_prefix(SYNTHETIC_FN_PREFIX) else {
        return false;
    };
    let Some(middle) = rest.strip_suffix(SYNTHETIC_FN_SUFFIX) else {
        return false;
    };
    !middle.is_empty() && middle.chars().all(|c| c.is_ascii_digit())
}

/// The examples of one docstring, which run one after the other in the
/// same namespace, so they are checked as one function body.
#[derive(Debug, Clone)]
pub struct DoctestSnippet {
    /// Human-friendly name of the doctest's container: `"<module>"`, a
    /// function name, or `"Class.method"`. Used in diagnostic messages.
    pub owner: String,
    /// The source of the examples with the prompts removed, joined by `\n`.
    pub code: String,
    /// Where a diagnostic goes when it has no line of its own: the first
    /// line of code, or the whole docstring when `exact` is false.
    pub anchor_range: TextRange,
    /// For each line of `code`, where it is in the file.
    /// `line_ranges.len() == code.split('\n').count()`.
    pub line_ranges: Vec<TextRange>,
    /// Whether each line of `code` is the text of its range in the file. It
    /// is not when the docstring has an escape, a tab, or a second string,
    /// and then every range is the whole docstring.
    pub exact: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MalformedKind {
    /// `>>>x`.
    Primary,
    /// A `>>>` with nothing after it, which the runner skips.
    EmptyPrompt,
    /// `...x`.
    Continuation,
    /// A `...` that does not start in the column of its `>>>`.
    ContinuationIndent,
    /// A line of output indented less than its `>>>`.
    OutputIndent,
    /// A `# doctest:` directive, which the runner does not follow.
    Directive,
    /// A second statement on a later line of the same example, which the
    /// runner refuses.
    SecondStatement,
}

/// A line that makes the runner refuse its docstring, or a `>>>` with no
/// example, which the runner skips.
#[derive(Debug, Clone)]
pub struct Malformed {
    pub range: TextRange,
    pub kind: MalformedKind,
}

/// Result of walking a module for doctests.
#[derive(Debug, Default)]
pub struct DoctestExtraction {
    pub snippets: Vec<DoctestSnippet>,
    pub malformed: Vec<Malformed>,
    /// The docstrings with examples that the runner never gets, because
    /// they belong to a definition inside a function.
    pub never_run: Vec<TextRange>,
    /// The docstrings with examples that the runner never gets, because
    /// they belong to a definition under a check of `__name__`, which the
    /// `test` command and the page answer differently.
    pub under_name_check: Vec<TextRange>,
    /// The name of the owner and the value of each docstring that the runner
    /// gets, in the order of the source.
    pub docstrings: Vec<(String, String)>,
}

/// Walk `module` looking for docstrings and extract doctest snippets from them.
/// `source` must be the exact source text the module was parsed from.
pub fn extract_doctests(module: &ModModule, source: &str) -> DoctestExtraction {
    let mut visitor = DoctestVisitor {
        source,
        scope: MODULE_OWNER.to_owned(),
        reachable: true,
        under_name_check: false,
        out: DoctestExtraction::default(),
    };
    visitor.docstring(&module.body);
    visitor.visit_body(&module.body);
    visitor.out
}

/// The docstrings of `source` that the runner gets, as the pairs of
/// [`DoctestExtraction::docstrings`], or `None` for a source that does not
/// parse.
pub fn docstrings(source: &str) -> Option<Vec<(String, String)>> {
    let parsed = parse_module(source).ok()?;
    Some(extract_doctests(parsed.syntax(), source).docstrings)
}

/// The owner of the docstring of the module. No function or class can have
/// this name.
const MODULE_OWNER: &str = "<module>";

/// Walks the module collecting doctests from docstrings on the module,
/// functions, and classes. Tracks the qualified scope name (`"<module>"`,
/// `"func"`, `"Class.method"`, …) for snippet ownership.
struct DoctestVisitor<'a> {
    source: &'a str,
    scope: String,
    /// Whether the runner gets the docstrings of the definitions here: at the
    /// top of the module or of a class, and not in the body of a function,
    /// whose names do not exist where the examples run.
    reachable: bool,
    /// Whether a definition here exists only for one answer to a check of
    /// `__name__`.
    under_name_check: bool,
    out: DoctestExtraction,
}

impl DoctestVisitor<'_> {
    /// Push a child name onto the scope: `<module>` → `name`, otherwise
    /// `parent` → `parent.name`. Returns the previous scope to restore.
    fn enter_scope(&mut self, name: &str) -> String {
        let new_scope = if self.scope == MODULE_OWNER {
            name.to_owned()
        } else {
            format!("{}.{}", self.scope, name)
        };
        std::mem::replace(&mut self.scope, new_scope)
    }

    /// Read the docstring of `body`, a string as its first statement, as the
    /// compiler takes it.
    fn docstring(&mut self, body: &[Stmt]) {
        let Some(Stmt::Expr(expr_stmt)) = body.first() else {
            return;
        };
        let Expr::StringLiteral(s) = &*expr_stmt.value else {
            return;
        };
        let doc = clean_doc(&docstring_value(s, self.source));
        if self.reachable && !self.under_name_check {
            scan_docstring(s, &doc, self.source, &self.scope, &mut self.out);
            self.out.docstrings.push((self.scope.clone(), doc));
            return;
        }
        let parsed = parse_doc(&doc);
        if parsed.examples.is_empty() && parsed.malformed.is_empty() {
            return;
        }
        if self.reachable {
            self.out.under_name_check.push(s.range());
        } else {
            self.out.never_run.push(s.range());
        }
    }
}

impl<'a> Visitor<'a> for DoctestVisitor<'_> {
    fn visit_stmt(&mut self, stmt: &'a Stmt) {
        match stmt {
            Stmt::FunctionDef(func) => {
                let prev = self.enter_scope(func.name.as_str());
                self.docstring(&func.body);
                let reachable = std::mem::replace(&mut self.reachable, false);
                self.visit_body(&func.body);
                self.reachable = reachable;
                self.scope = prev;
            }
            Stmt::ClassDef(cls) => {
                let prev = self.enter_scope(cls.name.as_str());
                self.docstring(&cls.body);
                self.visit_body(&cls.body);
                self.scope = prev;
            }
            // The `test` command runs the file under a name of its own, and
            // the page as `__main__`.
            Stmt::If(_) | Stmt::While(_) | Stmt::Match(_) if mentions_name(stmt) => {
                let under = std::mem::replace(&mut self.under_name_check, true);
                walk_stmt(self, stmt);
                self.under_name_check = under;
            }
            // A definition under an `if`, a `try`, or a loop is still an
            // attribute of the module or of the class.
            _ => walk_stmt(self, stmt),
        }
    }

    // No expression holds a definition with a docstring.
    fn visit_expr(&mut self, _expr: &'a Expr) {}
}

/// Returns `true` if a condition of `stmt`, an `if`, a `while`, or a
/// `match`, reads `__name__`, `false` otherwise.
fn mentions_name(stmt: &Stmt) -> bool {
    let reads_name = |expr: &Expr| {
        any_over_expr(
            expr,
            |e| matches!(e, Expr::Name(name) if name.id.as_str() == "__name__"),
        )
    };
    match stmt {
        Stmt::If(s) => {
            reads_name(&s.test)
                || s.elif_else_clauses
                    .iter()
                    .any(|clause| clause.test.as_ref().is_some_and(reads_name))
        }
        Stmt::While(s) => reads_name(&s.test),
        Stmt::Match(s) => {
            reads_name(&s.subject)
                || s.cases
                    .iter()
                    .any(|case| case.guard.as_deref().is_some_and(reads_name))
        }
        _ => false,
    }
}

/// Read the examples of the docstring `s` the way the runner does, from
/// `doc`, the value the interpreter gives the docstring.
fn scan_docstring(
    s: &ExprStringLiteral,
    doc: &str,
    source: &str,
    owner: &str,
    out: &mut DoctestExtraction,
) {
    let parsed = parse_doc(doc);
    let lines = raw_lines(s, source);
    let range_of = |line: usize| {
        lines
            .as_ref()
            .and_then(|lines| lines.get(line).copied())
            .unwrap_or(s.range())
    };

    for &(line, kind) in &parsed.malformed {
        out.malformed.push(Malformed {
            range: range_of(line),
            kind,
        });
    }

    let code_lines: Vec<&(usize, String)> = parsed.examples.iter().flatten().collect();
    for (line, code) in &code_lines {
        if has_directive(code) {
            out.malformed.push(Malformed {
                range: range_of(*line),
                kind: MalformedKind::Directive,
            });
        }
    }
    for example in &parsed.examples {
        if let Some(line) = second_statement_line(example) {
            out.malformed.push(Malformed {
                range: range_of(line),
                kind: MalformedKind::SecondStatement,
            });
        }
    }
    let line_ranges: Vec<TextRange> = code_lines
        .iter()
        .map(|(line, code)| match &lines {
            // The code is what is left of the line once the margin, the
            // indentation, and the prompt are gone, so it ends the line.
            Some(_) => {
                let line = range_of(*line);
                TextRange::new(line.end() - text_size(code.len()), line.end())
            }
            None => s.range(),
        })
        .collect();
    let Some(&anchor_range) = line_ranges.first() else {
        return;
    };
    out.snippets.push(DoctestSnippet {
        owner: owner.to_owned(),
        code: code_lines
            .iter()
            .map(|(_, code)| code.as_str())
            .collect::<Vec<_>>()
            .join("\n"),
        anchor_range,
        line_ranges,
        exact: lines.is_some(),
    });
}

/// The value of the docstring `s` as the interpreter has it. RustPython
/// turns each `\r\n` of the source into `\n` before it parses, and the value
/// that the parser gives keeps the line endings of the text it reads.
fn docstring_value<'a>(s: &'a ExprStringLiteral, source: &str) -> Cow<'a, str> {
    let Some(text) = source.get(std::ops::Range::<usize>::from(s.range())) else {
        return Cow::Borrowed(s.value.to_str());
    };
    if !text.contains("\r\n") {
        return Cow::Borrowed(s.value.to_str());
    }
    match parse_expression(&text.replace("\r\n", "\n")).map(|parsed| parsed.into_expr()) {
        Ok(Expr::StringLiteral(normalized)) => Cow::Owned(normalized.value.to_str().to_owned()),
        // The text parsed once as a string, and has only lost some `\r`.
        _ => Cow::Owned(s.value.to_str().replace("\r\n", "\n")),
    }
}

/// The range in `source` of each line of the docstring `s`, without its line
/// ending, or `None` when a line of the source is not the line of the value
/// that [`clean_doc`] starts from: a docstring of several strings, one with
/// an escape, or one with a tab, which `clean_doc` expands.
fn raw_lines(s: &ExprStringLiteral, source: &str) -> Option<Vec<TextRange>> {
    if s.value.is_implicit_concatenated() {
        return None;
    }
    let content = s.value.iter().next()?.content_range();
    let start: usize = content.start().into();
    let raw = source.get(start..usize::from(content.end()))?;
    if raw.contains('\t') {
        return None;
    }
    // A `\r\n` ends a line as `\n` does, as [`docstring_value`] reads it.
    let mut ranges = Vec::new();
    let mut texts = Vec::new();
    let mut line_start = 0;
    let bytes = raw.as_bytes();
    let mut i = 0;
    while i <= bytes.len() {
        let end_of_line = match bytes.get(i) {
            None => Some(0),
            Some(b'\n') => Some(1),
            Some(b'\r') if bytes.get(i + 1) == Some(&b'\n') => Some(2),
            Some(_) => None,
        };
        match end_of_line {
            Some(width) => {
                texts.push(&raw[line_start..i]);
                ranges.push(TextRange::new(
                    text_size(start + line_start),
                    text_size(start + i),
                ));
                i += width.max(1);
                line_start = i;
            }
            None => i += 1,
        }
    }
    let value = docstring_value(s, source);
    let value: Vec<&str> = value.split('\n').collect();
    (texts == value).then_some(ranges)
}

/// The docstring the compiler stores for `doc`, with the tabs expanded and
/// the margin of the lines after the first removed.
// From `clean_doc` in the codegen of RustPython, which runs the doctests.
fn clean_doc(doc: &str) -> String {
    let doc = expand_tabs(doc, 8);
    let margin = doc
        .split('\n')
        .skip(1)
        .filter(|line| line.chars().any(|c| c != ' '))
        .map(|line| line.chars().take_while(|c| *c == ' ').count())
        .min()
        .unwrap_or(0);

    let mut cleaned = String::with_capacity(doc.len());
    if let Some(first_line) = doc.split('\n').next() {
        let trimmed = first_line.trim_start();
        if trimmed.len() == first_line.len() && margin == 0 {
            return doc;
        }
        cleaned.push_str(trimmed);
    }
    for line in doc.split('\n').skip(1) {
        cleaned.push('\n');
        let skip = line.chars().take(margin).take_while(|c| *c == ' ').count();
        cleaned.push_str(&line[skip..]);
    }
    cleaned
}

// From `expandtabs` in the codegen of RustPython.
fn expand_tabs(input: &str, tab_stop: usize) -> String {
    let mut expanded = String::with_capacity(input.len());
    let mut tab_size = tab_stop;
    let mut col_count = 0usize;
    for ch in input.chars() {
        match ch {
            '\t' => {
                let num_spaces = tab_size - col_count;
                col_count += num_spaces;
                expanded.push_str(&" ".repeat(num_spaces));
            }
            '\r' | '\n' => {
                expanded.push(ch);
                col_count = 0;
                tab_size = 0;
            }
            _ => {
                expanded.push(ch);
                col_count += 1;
            }
        }
        if col_count >= tab_size {
            tab_size += tab_stop;
        }
    }
    expanded
}

/// The examples of a docstring, each a list of (line, code) pairs, and the
/// lines that make the docstring malformed, both by the index of the line.
#[derive(Debug, Default)]
struct ParsedDoc {
    examples: Vec<Vec<(usize, String)>>,
    malformed: Vec<(usize, MalformedKind)>,
}

/// Read the examples of `doc`, a docstring as [`clean_doc`] leaves it, with
/// the grammar of `_parse` in `doctest_runner.py`, which follows
/// `DocTestParser` of the stdlib `doctest`.
///
/// The runner stops at the first line it refuses, and this goes on, so the
/// student sees every such line at once. A `>>>` with nothing after it, which
/// the runner skips, is malformed here, because it is almost always a test
/// the student forgot to write.
fn parse_doc(doc: &str) -> ParsedDoc {
    let all: Vec<&str> = doc.split('\n').collect();
    let margin = min_indent(&all);
    let lines: Vec<&str> = all.iter().map(|line| skip_chars(line, margin)).collect();
    let mut parsed = ParsedDoc::default();
    let mut i = 0;
    while let Some(line) = lines.get(i) {
        let Some(indent) = ps1_indent(line) else {
            i += 1;
            continue;
        };
        let start = i;
        i += 1;
        while lines
            .get(i)
            .is_some_and(|line| line.trim_start_matches(' ').starts_with("..."))
        {
            i += 1;
        }
        let source_end = i;
        while lines
            .get(i)
            .is_some_and(|line| !line.trim_matches(' ').is_empty() && ps1_indent(line).is_none())
        {
            i += 1;
        }

        let source_lines = lines.get(start..source_end).unwrap_or_default();
        let prefix = " ".repeat(indent);
        let malformed_before = parsed.malformed.len();
        for (k, line) in source_lines.iter().enumerate() {
            // The runner checks the space first, at the column of the space
            // after the `>>>`, so a `...` out of line fails that check too,
            // with a message that does not say what is wrong.
            if k > 0 && !line.is_empty() && !line.starts_with(&format!("{prefix}.")) {
                parsed
                    .malformed
                    .push((start + k, MalformedKind::ContinuationIndent));
            } else if line.chars().nth(indent + 3).is_some_and(|c| c != ' ') {
                let kind = if k == 0 {
                    MalformedKind::Primary
                } else {
                    MalformedKind::Continuation
                };
                parsed.malformed.push((start + k, kind));
            }
        }
        // The code of a line with a malformed prompt is not what the student
        // meant, and checking it would only add errors about the wrong text.
        let source_is_malformed = parsed.malformed.len() > malformed_before;
        for (k, line) in lines
            .get(source_end..i)
            .unwrap_or_default()
            .iter()
            .enumerate()
        {
            if !line.is_empty() && !line.starts_with(&prefix) {
                parsed
                    .malformed
                    .push((source_end + k, MalformedKind::OutputIndent));
            }
        }

        let code: Vec<(usize, String)> = source_lines
            .iter()
            .enumerate()
            .map(|(k, line)| (start + k, skip_chars(line, indent + 4).to_owned()))
            .collect();
        let source = code
            .iter()
            .map(|(_, code)| code.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        if source_is_malformed {
            continue;
        }
        if is_blank_or_comment(&source) {
            if source.chars().all(is_py_space) {
                parsed.malformed.push((start, MalformedKind::EmptyPrompt));
            }
        } else {
            parsed.examples.push(code);
        }
    }
    parsed
}

/// The line of the docstring where a second statement of `example` starts,
/// if one starts on a line after the first statement does. The runner
/// compiles an example in single mode, and RustPython refuses that, as
/// `single_mode_body_error` in its compiler says. A source that does not
/// parse is left to the type check.
fn second_statement_line(example: &[(usize, String)]) -> Option<usize> {
    let source = example
        .iter()
        .map(|(_, code)| code.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    let parsed = parse_module(&source).ok()?;
    let body = &parsed.syntax().body;
    let first = usize::from(body.first()?.start());
    body.iter().skip(1).find_map(|stmt| {
        let start = usize::from(stmt.start());
        let between = source.get(first..start)?;
        if !between.contains(['\n', '\r']) {
            return None;
        }
        let line = source.get(..start)?.matches('\n').count();
        example.get(line).map(|(line, _)| *line)
    })
}

/// Returns `true` if `code` holds a `# doctest:` directive, `false`
/// otherwise. The test is the one of the stdlib, which takes a `#` with a
/// quote after it on the line for one inside a string.
fn has_directive(code: &str) -> bool {
    code.match_indices('#').any(|(at, _)| {
        let rest = code.get(at + 1..).unwrap_or_default();
        rest.trim_start_matches(is_py_space)
            .strip_prefix("doctest:")
            .is_some_and(|options| !options.contains(['\'', '"']))
    })
}

/// The indentation of `line` if it starts with `>>>`, `None` otherwise.
fn ps1_indent(line: &str) -> Option<usize> {
    let rest = line.trim_start_matches(' ');
    rest.starts_with(">>>").then(|| line.len() - rest.len())
}

/// The smallest indentation of a line that is not blank.
fn min_indent(lines: &[&str]) -> usize {
    lines
        .iter()
        .filter_map(|line| {
            let rest = line.trim_start_matches(' ');
            let first = rest.chars().next()?;
            (!is_py_space(first)).then(|| line.len() - rest.len())
        })
        .min()
        .unwrap_or(0)
}

/// Returns `true` if `source` is one line that is blank or only a comment,
/// `false` otherwise.
fn is_blank_or_comment(source: &str) -> bool {
    let source = source.strip_suffix('\n').unwrap_or(source);
    if source.contains('\n') {
        return false;
    }
    let rest = source.trim_start_matches(' ');
    rest.is_empty() || rest.starts_with('#')
}

/// `line` without its first `n` characters, as `line[n:]` in Python.
fn skip_chars(line: &str, n: usize) -> &str {
    line.char_indices()
        .nth(n)
        .map_or("", |(offset, _)| line.get(offset..).unwrap_or_default())
}

/// Returns `true` if Python's `str.isspace` holds for `c`, `false` otherwise.
/// Python also counts the separators U+001C to U+001F as space.
fn is_py_space(c: char) -> bool {
    c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c)
}

/// Build a diagnostic for each docstring whose examples never run.
pub fn never_run_diagnostics(file: File, never_run: &[TextRange]) -> Vec<Diagnostic> {
    never_run
        .iter()
        .map(|range| {
            make_lint_diagnostic(
                &DOCTEST_NEVER_RUNS,
                file,
                *range,
                "Doctest inside a function never runs: move the definition to the top of the module or of a class".to_string(),
            )
        })
        .collect()
}

/// Build a diagnostic for each docstring under a check of `__name__`.
pub fn under_name_check_diagnostics(file: File, ranges: &[TextRange]) -> Vec<Diagnostic> {
    ranges
        .iter()
        .map(|range| {
            make_lint_diagnostic(
                &DOCTEST_UNDER_NAME_CHECK,
                file,
                *range,
                "Doctest under a check of `__name__` never runs: move the definition out of the check".to_string(),
            )
        })
        .collect()
}

/// Build diagnostics for each malformed line.
pub fn malformed_diagnostics(file: File, malformed: &[Malformed]) -> Vec<Diagnostic> {
    malformed
        .iter()
        .map(|m| {
            let (lint, msg) = match m.kind {
                MalformedKind::Primary | MalformedKind::EmptyPrompt => (
                    &DOCTEST_MALFORMED_PROMPT,
                    "Doctest prompt must be followed by a single space: change `>>>x` to `>>> x`",
                ),
                MalformedKind::Continuation => (
                    &DOCTEST_MALFORMED_PROMPT,
                    "Doctest continuation must be followed by a single space: change `...x` to `... x`",
                ),
                MalformedKind::ContinuationIndent => (
                    &DOCTEST_INCONSISTENT_INDENTATION,
                    "Doctest continuation `...` must start in the column of its `>>>`",
                ),
                MalformedKind::OutputIndent => (
                    &DOCTEST_INCONSISTENT_INDENTATION,
                    "Doctest output must not start to the left of its `>>>`",
                ),
                MalformedKind::Directive => (
                    &DOCTEST_DIRECTIVE,
                    "spython runs doctests with no options: remove the `# doctest:` comment",
                ),
                MalformedKind::SecondStatement => (
                    &DOCTEST_MULTIPLE_STATEMENTS,
                    "A doctest example holds one statement: start this one with its own `>>>`",
                ),
            };
            make_lint_diagnostic(lint, file, m.range, msg.to_string())
        })
        .collect()
}

/// Indent (in bytes) prepended to each snippet body line by
/// `build_synthetic_source`. The body bytes that follow are byte-identical to
/// the original docstring content, so byte offsets within the body translate
/// 1:1 between the synthetic source and the original line range.
const SYNTHETIC_BODY_INDENT: u32 = 4;

/// Remap a synthetic `prim_range` to a sub-range of `line_range` in the
/// original source, preserving column information when both endpoints lie on
/// the same synthetic body line. If the diagnostic spans multiple synthetic
/// lines, the end is clamped to the end of `line_range`.
fn remap_range_within_line(
    prim_range: TextRange,
    line_idx: usize,
    line_starts: &[u32],
    line_range: TextRange,
    byte_to_line: impl Fn(u32) -> usize,
) -> TextRange {
    let line_start = line_starts.get(line_idx).copied().unwrap_or(0);
    let body_len: u32 = line_range.len().into();
    let prim_start: u32 = prim_range.start().into();
    let prim_end: u32 = prim_range.end().into();

    let col = |byte: u32| {
        byte.saturating_sub(line_start)
            .saturating_sub(SYNTHETIC_BODY_INDENT)
            .min(body_len)
    };

    let new_start = line_range.start() + TextSize::new(col(prim_start));
    // Only refine the end when it shares the same synthetic line; otherwise
    // we'd cross a real-source line boundary that has no meaningful mapping.
    // Use `prim_end - 1` to look up the inclusive last byte's line.
    let same_line = prim_end <= prim_start
        || byte_to_line(prim_end.saturating_sub(1).max(prim_start)) == line_idx;
    let new_end = if same_line {
        line_range.start() + TextSize::new(col(prim_end))
    } else {
        line_range.end()
    };

    if new_end >= new_start {
        TextRange::new(new_start, new_end)
    } else {
        line_range
    }
}

// --- Synthetic source + diagnostic remapping --------------------------------

/// Origin of a single line in a synthetic source built by `build_synthetic_source`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyntheticLine {
    /// A line copied verbatim from the original source (or a trailing newline
    /// we added). Diagnostics here are dropped — they are already reported
    /// against the real file.
    Original,
    /// A scaffold line for snippet `usize`: blank separator, `def` header, or
    /// the `pass` body of an empty snippet. Diagnostics here are remapped to
    /// the snippet's anchor range so they don't get silently lost.
    SyntheticHeader(usize),
    /// A real doctest body line: `(snippet_idx, line_idx_in_snippet)`.
    SyntheticBody(usize, usize),
}

/// Maps lines of the synthetic source (produced by `build_synthetic_source`)
/// back to the original snippets.
#[derive(Debug)]
pub struct SyntheticMap {
    /// One entry per 0-based line index in the synthetic source.
    pub lines: Vec<SyntheticLine>,
    /// Byte offset in the synthetic source where the verbatim copy of the
    /// original ends (i.e. where the appended doctest functions begin).
    pub original_len: u32,
}

/// Build a synthetic Python source: the original verbatim, then one
/// `def __spython_doctest_<N>__() -> None:` stub per snippet, with the
/// snippet's code indented 4 spaces inside the function body.
pub fn build_synthetic_source(
    original: &str,
    snippets: &[DoctestSnippet],
) -> (String, SyntheticMap) {
    let mut syn = String::with_capacity(original.len() + 256);
    syn.push_str(original);
    // Ensure a trailing newline before we append; otherwise the first `def`
    // would attach to the last original line.
    if !syn.ends_with('\n') {
        syn.push('\n');
    }
    let original_len = text_size(syn.len()).to_u32();

    let original_line_count = syn.lines().count();
    let mut line_map: Vec<SyntheticLine> = vec![SyntheticLine::Original; original_line_count];

    for (idx, snippet) in snippets.iter().enumerate() {
        // Blank separator line.
        syn.push('\n');
        line_map.push(SyntheticLine::SyntheticHeader(idx));
        // `def __spython_doctest_N__() -> None:`
        syn.push_str("def ");
        syn.push_str(SYNTHETIC_FN_PREFIX);
        syn.push_str(&idx.to_string());
        syn.push_str(SYNTHETIC_FN_SUFFIX);
        syn.push_str("() -> None:\n");
        line_map.push(SyntheticLine::SyntheticHeader(idx));

        // `String::split('\n')` always yields at least one element, so an empty
        // snippet appears as `[""]` here and falls into the all-empty branch.
        let body_lines: Vec<&str> = snippet.code.split('\n').collect();
        if body_lines.iter().all(|l| l.is_empty()) {
            syn.push_str("    pass\n");
            line_map.push(SyntheticLine::SyntheticHeader(idx));
        } else {
            for (line_idx, body_line) in body_lines.iter().enumerate() {
                syn.push_str("    ");
                syn.push_str(body_line);
                syn.push('\n');
                line_map.push(SyntheticLine::SyntheticBody(idx, line_idx));
            }
        }
    }

    (
        syn,
        SyntheticMap {
            lines: line_map,
            original_len,
        },
    )
}

/// Filter and rewrite diagnostics from a synthetic source:
/// * drop any whose primary span falls within the verbatim-original portion
///   (already reported against the real file);
/// * rewrite the primary span to point at the original docstring location and
///   prepend `"in doctest of <owner>: "` to the message. Diagnostics that
///   land on a scaffold line (`def` header, blank, or synthetic `pass`) fall
///   back to the snippet's anchor range so they aren't silently dropped.
pub fn remap_diagnostics(
    diags: Vec<Diagnostic>,
    synthetic_file: File,
    original_file: File,
    snippets: &[DoctestSnippet],
    map: &SyntheticMap,
    synthetic_source: &str,
) -> Vec<Diagnostic> {
    // Precompute line-start byte offsets in the synthetic source.
    let mut line_starts: Vec<u32> = Vec::with_capacity(synthetic_source.len() / 40 + 1);
    line_starts.push(0);
    for (i, b) in synthetic_source.bytes().enumerate() {
        if b == b'\n' {
            line_starts.push(text_size(i + 1).to_u32());
        }
    }

    let byte_to_line = |byte: u32| -> usize {
        // binary search: last line_start <= byte
        match line_starts.binary_search(&byte) {
            Ok(i) => i,
            Err(i) => i.saturating_sub(1),
        }
    };

    let mut out: Vec<Diagnostic> = Vec::new();
    for diag in diags {
        // Only process diagnostics whose primary annotation points at the synthetic file.
        let Some(prim) = diag.primary_annotation() else {
            continue;
        };
        let prim_span = prim.get_span();
        // Only handle spans that point at a ty `File`. In this pipeline the
        // synthetic source is registered with ty, so its primary span carries
        // a `UnifiedFile::Ty(File)`.
        let span_file = match prim_span.file() {
            ruff_db::diagnostic::UnifiedFile::Ty(f) => *f,
            ruff_db::diagnostic::UnifiedFile::Ruff(_) => continue,
        };
        if span_file != synthetic_file {
            // Not ours; keep as-is (shouldn't happen in practice).
            out.push(diag);
            continue;
        }
        let Some(prim_range) = prim_span.range() else {
            continue;
        };
        let prim_start: u32 = prim_range.start().into();
        if prim_start < map.original_len {
            // Diagnostic in the verbatim-original region; already reported.
            continue;
        }
        // ty's parse errors can point at EOF (a byte position one past the
        // last newline). `byte_to_line` then returns an index past
        // `map.lines`, so clamp to the last real line — which is always one
        // of the appended doctest body / scaffold lines for `prim_start >=
        // original_len`.
        let line_idx = byte_to_line(prim_start).min(map.lines.len().saturating_sub(1));
        let (snippet, original_range) = match map.lines.get(line_idx).copied() {
            Some(SyntheticLine::SyntheticBody(idx, line)) => {
                let Some(snippet) = snippets.get(idx) else {
                    continue;
                };
                let line_range = snippet
                    .line_ranges
                    .get(line)
                    .copied()
                    .unwrap_or(snippet.anchor_range);
                // A column of the code is a column of the file only when the
                // line is the text of its range.
                let range = if snippet.exact {
                    remap_range_within_line(
                        prim_range,
                        line_idx,
                        &line_starts,
                        line_range,
                        byte_to_line,
                    )
                } else {
                    line_range
                };
                (snippet, range)
            }
            Some(SyntheticLine::SyntheticHeader(idx)) => match snippets.get(idx) {
                Some(snippet) => (snippet, snippet.anchor_range),
                None => continue,
            },
            Some(SyntheticLine::Original) | None => continue,
        };

        let id = diag.id();
        let severity = diag.severity();
        let old_msg = diag.headline_message().to_string();
        let new_msg = format!("in doctest of {}: {old_msg}", snippet.owner);
        let mut rebuilt = Diagnostic::new(id, severity, new_msg);
        rebuilt.annotate(Annotation::primary(
            Span::from(original_file).with_range(original_range),
        ));
        out.push(rebuilt);
    }
    out
}

// A test writes the source it means to read back, and asserts how many
// snippets came out before it names one.
#[cfg(test)]
#[allow(clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn extract(src: &str) -> DoctestExtraction {
        let parsed = parse_module(src).expect("the test source parses");
        let module = parsed.into_syntax();
        extract_doctests(&module, src)
    }

    #[test]
    fn no_docstring_no_output() {
        let src = "def f(x: int) -> int:\n    return x\n";
        let ex = extract(src);
        assert!(ex.snippets.is_empty());
        assert!(ex.malformed.is_empty());
    }

    #[test]
    fn docstring_without_prompts_yields_nothing() {
        let src = "def f(x: int) -> int:\n    \"\"\"Sum two ints.\"\"\"\n    return x\n";
        let ex = extract(src);
        assert!(ex.snippets.is_empty());
        assert!(ex.malformed.is_empty());
    }

    #[test]
    fn single_example() {
        let src = "def add(x: int, y: int) -> int:\n    \"\"\"\n    >>> add(1, 2)\n    3\n    \"\"\"\n    return x + y\n";
        let ex = extract(src);
        assert_eq!(ex.snippets.len(), 1);
        assert!(ex.malformed.is_empty());
        let s = &ex.snippets[0];
        assert_eq!(s.owner, "add");
        assert_eq!(s.code, "add(1, 2)");
        assert_eq!(s.line_ranges.len(), 1);
        // The line_range should point at the "add(1, 2)" inside the source.
        let slice: usize = s.line_ranges[0].start().into();
        let end: usize = s.line_ranges[0].end().into();
        assert_eq!(&src[slice..end], "add(1, 2)");
    }

    #[test]
    fn continuation_lines() {
        let src = "def f(x: int) -> int:\n    \"\"\"\n    >>> y = (\n    ...     x\n    ... )\n    \"\"\"\n    return x\n";
        let ex = extract(src);
        assert_eq!(ex.snippets.len(), 1);
        assert!(ex.malformed.is_empty());
        let s = &ex.snippets[0];
        assert_eq!(s.code, "y = (\n    x\n)");
        assert_eq!(s.line_ranges.len(), 3);
    }

    #[test]
    fn malformed_primary() {
        let src = "def f(x: int) -> int:\n    \"\"\"\n    >>>x\n    \"\"\"\n    return x\n";
        let ex = extract(src);
        assert_eq!(ex.snippets.len(), 0);
        assert_eq!(ex.malformed.len(), 1);
        assert_eq!(ex.malformed[0].kind, MalformedKind::Primary);
    }

    #[test]
    fn malformed_bare_primary_prompt() {
        // `>>>` alone (with no example after it) is almost certainly a typo;
        // surface it instead of silently swallowing the line.
        let src = "def f(x: int) -> int:\n    \"\"\"\n    >>>\n    \"\"\"\n    return x\n";
        let ex = extract(src);
        assert_eq!(ex.snippets.len(), 0);
        assert_eq!(ex.malformed.len(), 1);
        assert_eq!(ex.malformed[0].kind, MalformedKind::EmptyPrompt);
    }

    #[test]
    fn remap_within_line_preserves_column() {
        // synthetic line content: "    add(\"a\", 2)\n"
        //                        ^^^^               ← 4-byte indent
        //                            ^^^^^^^^^^^   ← body, 11 bytes
        // simulate ty's range pointing at the `"a"` argument (cols 8..=10
        // inside the synthetic line, which is body cols 4..=6).
        let line_starts = vec![0u32, 16];
        let line_range = TextRange::new(TextSize::new(100), TextSize::new(111)); // 11-byte original body
        let prim = TextRange::new(TextSize::new(8), TextSize::new(11));
        let remapped = remap_range_within_line(prim, 0, &line_starts, line_range, |b| {
            line_starts.partition_point(|&s| s <= b).saturating_sub(1)
        });
        assert_eq!(u32::from(remapped.start()), 100 + 4);
        assert_eq!(u32::from(remapped.end()), 100 + 7);
    }

    #[test]
    fn remap_clamps_end_when_diag_crosses_lines() {
        // Diagnostic starts on line 0 and ends on line 1: end clamps to the
        // first line's end so we never produce a range outside `line_range`.
        let line_starts = vec![0u32, 16, 32];
        let line_range = TextRange::new(TextSize::new(100), TextSize::new(111));
        let prim = TextRange::new(TextSize::new(8), TextSize::new(20));
        let remapped = remap_range_within_line(prim, 0, &line_starts, line_range, |b| {
            line_starts.partition_point(|&s| s <= b).saturating_sub(1)
        });
        assert_eq!(u32::from(remapped.start()), 100 + 4);
        assert_eq!(u32::from(remapped.end()), 111);
    }

    #[test]
    fn remap_clamps_when_diag_starts_in_indent() {
        // Diagnostic that includes the synthetic 4-space indent collapses to
        // the start of the body content (col 0), not a negative offset.
        let line_starts = vec![0u32, 16];
        let line_range = TextRange::new(TextSize::new(100), TextSize::new(111));
        let prim = TextRange::new(TextSize::new(2), TextSize::new(6));
        let remapped = remap_range_within_line(prim, 0, &line_starts, line_range, |b| {
            line_starts.partition_point(|&s| s <= b).saturating_sub(1)
        });
        assert_eq!(u32::from(remapped.start()), 100);
        assert_eq!(u32::from(remapped.end()), 100 + 2);
    }

    #[test]
    fn synthetic_header_lines_marked_for_fallback() {
        // The blank separator and `def` header preceding each snippet must
        // be marked as `SyntheticHeader`, so a diagnostic landing on either
        // can be remapped to the snippet's anchor range instead of dropped.
        let src = "def add(x: int, y: int) -> int:\n    \"\"\"\n    >>> add(1, 2)\n    \"\"\"\n    return x + y\n";
        let ex = extract(src);
        let (_, map) = build_synthetic_source(src, &ex.snippets);
        assert!(
            map.lines
                .iter()
                .any(|l| matches!(l, SyntheticLine::SyntheticHeader(0)))
        );
        assert!(
            map.lines
                .iter()
                .any(|l| matches!(l, SyntheticLine::SyntheticBody(0, 0)))
        );
    }

    #[test]
    fn malformed_continuation() {
        let src = "def f(x: int) -> int:\n    \"\"\"\n    >>> y = (\n    ...x\n    ... )\n    \"\"\"\n    return x\n";
        let ex = extract(src);
        // The `>>>` example is started but the malformed continuation stops it.
        assert!(
            ex.malformed
                .iter()
                .any(|m| m.kind == MalformedKind::Continuation)
        );
    }

    #[test]
    fn continuation_outside_example_is_prose() {
        let src = "def f(x: int) -> int:\n    \"\"\"Summary ... more prose.\"\"\"\n    return x\n";
        let ex = extract(src);
        assert!(ex.snippets.is_empty());
        assert!(ex.malformed.is_empty());
    }

    #[test]
    fn method_owner_uses_dotted_name() {
        let src = "class Point:\n    def move(self, dx: int) -> int:\n        \"\"\"\n        >>> p.move(1)\n        \"\"\"\n        return dx\n";
        let ex = extract(src);
        assert_eq!(ex.snippets.len(), 1);
        assert_eq!(ex.snippets[0].owner, "Point.move");
    }

    #[test]
    fn examples_in_one_docstring_merge_into_one_snippet() {
        // Two `>>>` examples in the same docstring share scope at runtime
        // (both run in `vars(module)`), so we merge them into a single
        // snippet with both code lines.
        let src = "def add(x: int, y: int) -> int:\n    \"\"\"\n    >>> add(1, 2)\n    3\n    >>> add(-1, 1)\n    0\n    \"\"\"\n    return x + y\n";
        let ex = extract(src);
        assert_eq!(ex.snippets.len(), 1);
        assert_eq!(ex.snippets[0].code, "add(1, 2)\nadd(-1, 1)");
        let (syn, map) = build_synthetic_source(src, &ex.snippets);
        assert!(syn.contains("def __spython_doctest_0__() -> None:"));
        assert!(map.lines.contains(&SyntheticLine::SyntheticBody(0, 0)));
        assert!(map.lines.contains(&SyntheticLine::SyntheticBody(0, 1)));
    }

    /// The docstring of `f` in `src`, as [`scan_docstring`] reads it: the
    /// examples, each with the newline the runner gives its source, or
    /// `ERR` when the runner would refuse the docstring.
    fn read_as_the_checker(src: &str) -> String {
        let parsed = parse_module(src).expect("the test source parses");
        let module = parsed.into_syntax();
        let Some(Stmt::FunctionDef(f)) = module.body.first() else {
            panic!("the test source starts with a def");
        };
        let Some(Stmt::Expr(stmt)) = f.body.first() else {
            panic!("the def starts with its docstring");
        };
        let Expr::StringLiteral(doc) = &*stmt.value else {
            panic!("the docstring is a string");
        };
        let parsed = parse_doc(&clean_doc(&docstring_value(doc, src)));
        if parsed
            .malformed
            .iter()
            .any(|(_, kind)| *kind != MalformedKind::EmptyPrompt)
        {
            return "ERR".to_owned();
        }
        parsed
            .examples
            .iter()
            .map(|example| {
                let mut source = example
                    .iter()
                    .map(|(_, code)| code.as_str())
                    .collect::<Vec<_>>()
                    .join("\n");
                if !source.ends_with('\n') {
                    source.push('\n');
                }
                source
            })
            .collect::<Vec<_>>()
            .join("\u{1}")
    }

    /// `text` as a Python string literal, every character that is not plain
    /// ASCII written as an escape.
    fn py_literal(text: &str) -> String {
        let mut out = String::from("\"");
        for c in text.chars() {
            if (' '..='~').contains(&c) && c != '\\' && c != '"' {
                out.push(c);
            } else {
                out.push_str(&format!("\\U{:08x}", u32::from(c)));
            }
        }
        out.push('"');
        out
    }

    /// Docstrings made of lines that start with what the grammar of a
    /// doctest cares about, from a fixed seed.
    fn docstrings(count: usize) -> Vec<String> {
        const INDENTS: &[&str] = &["", "", " ", "  ", "    ", "     ", "\t", "\u{a0}", "\u{c}"];
        const HEADS: &[&str] = &[
            ">>> ",
            ">>> ",
            ">>> ",
            ">>>",
            "... ",
            "... ",
            "...",
            "",
            "",
            "#",
            "Traceback (most recent call last):",
            "<BLANKLINE>",
        ];
        const TAILS: &[&str] = &[
            "x",
            "1",
            " ",
            "  ",
            "# c",
            "\t",
            "\u{1c}",
            "é",
            ":",
            "(",
            ")",
            ".",
            ">>>",
            "...",
            "ValueError: m",
            "\r",
        ];
        let mut state: u64 = 0x2545_f491_4f6c_dd1d;
        let mut next = |n: usize| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            usize::try_from(state % n as u64).expect("below n")
        };
        (0..count)
            .map(|_| {
                let lines = 1 + next(8);
                (0..lines)
                    .map(|_| {
                        let mut line = INDENTS[next(INDENTS.len())].to_owned();
                        line.push_str(HEADS[next(HEADS.len())]);
                        for _ in 0..next(3) {
                            line.push_str(TAILS[next(TAILS.len())]);
                        }
                        line
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .collect()
    }

    /// The checker reads the same examples as the runner, from the same
    /// source, written with escapes and, where it can be, as is.
    #[test]
    fn the_checker_reads_a_docstring_as_the_runner_does() {
        let mut cases = String::new();
        let mut exact_sources = Vec::new();
        for doc in docstrings(3000) {
            let mut sources = vec![format!("def f():\n    {}\n", py_literal(&doc))];
            if !doc.contains(['\\', '"']) {
                let src = format!("def f():\n    \"\"\"{doc}\"\"\"\n");
                let crlf = src.replace('\n', "\r\n");
                if !doc.contains('\t') {
                    exact_sources.push(src.clone());
                    exact_sources.push(crlf.clone());
                }
                sources.push(src);
                sources.push(crlf);
            }
            for src in sources {
                cases.push_str(&format!(
                    "    ({}, {}),\n",
                    py_literal(&src),
                    py_literal(&read_as_the_checker(&src))
                ));
            }
        }
        let script = format!(
            "{}\n_CASES = [\n{cases}]\n{}",
            crate::run::DOCTEST_RUNNER,
            "for _src, _checker in _CASES:\n\
             \x20   _ns = {}\n\
             \x20   exec(_src, _ns)\n\
             \x20   _doc = dict(_docstrings(_src)).get('f')\n\
             \x20   assert _doc == _ns['f'].__doc__, (_src, _doc)\n\
             \x20   try:\n\
             \x20       _runner = '\\x01'.join(s for s, _, _ in _parse(_doc, 'f'))\n\
             \x20   except ValueError:\n\
             \x20       _runner = 'ERR'\n\
             \x20   assert _runner == _checker, (_src, _runner, _checker)\n"
        );
        assert!(crate::run::run_source(&script, "parse_test.py", ".").is_ok());

        // Where the docstring is the text of the file, each line of code is
        // the text of its range.
        for src in exact_sources {
            for snippet in extract(&src).snippets {
                assert!(snippet.exact, "{src:?}");
                for (line, range) in snippet.code.split('\n').zip(&snippet.line_ranges) {
                    assert_eq!(&src[*range], line, "{src:?}");
                }
            }
        }
    }

    #[test]
    fn a_bare_continuation_is_an_empty_line_of_code() {
        let src = "def f() -> None:\n    \"\"\"\n    >>> if True:\n    ...     f()\n    ...\n    \"\"\"\n";
        let ex = extract(src);
        assert!(ex.malformed.is_empty());
        assert_eq!(ex.snippets[0].code, "if True:\n    f()\n");
    }

    #[test]
    fn a_line_out_of_line_with_its_prompt_is_malformed() {
        let src = "def f() -> None:\n    \"\"\"\n    >>> f(\n      ... )\n    >>> f()\n   x\n    \"\"\"\n";
        let kinds: Vec<MalformedKind> = extract(src).malformed.iter().map(|m| m.kind).collect();
        assert_eq!(
            kinds,
            [
                MalformedKind::ContinuationIndent,
                MalformedKind::OutputIndent
            ]
        );
    }

    #[test]
    fn a_docstring_with_crlf_lines_maps_each_line() {
        let src = "def f() -> None:\r\n    \"\"\"\r\n    >>> f()\r\n    >>> g()\r\n    \"\"\"\r\n";
        let snippet = &extract(src).snippets[0];
        assert!(snippet.exact);
        assert_eq!(&src[snippet.line_ranges[0]], "f()");
        assert_eq!(&src[snippet.line_ranges[1]], "g()");
    }

    #[test]
    fn a_docstring_with_an_escape_maps_to_the_whole_docstring() {
        let src = "def f() -> None:\n    \"\"\"\n    >>> f('\\n')\n    \"\"\"\n";
        let snippet = &extract(src).snippets[0];
        assert!(!snippet.exact);
        assert_eq!(snippet.code, "f('");
        assert_eq!(&src[snippet.line_ranges[0]], &src[21..src.len() - 1]);
    }

    #[test]
    fn a_docstring_of_two_strings_is_read() {
        let src = "def f() -> None:\n    \">>> f()\\n\" \">>> g()\"\n";
        let snippet = &extract(src).snippets[0];
        assert!(!snippet.exact);
        assert_eq!(snippet.code, "f()\ng()");
    }

    #[test]
    fn a_definition_under_an_if_or_a_try_is_read() {
        let src = "if True:\n    def f() -> None:\n        \">>> f()\"\nelse:\n    pass\ntry:\n    class C:\n        def m(self) -> None:\n            \">>> 1\"\nexcept ValueError:\n    pass\n";
        let ex = extract(src);
        let owners: Vec<&str> = ex.snippets.iter().map(|s| s.owner.as_str()).collect();
        assert_eq!(owners, ["f", "C.m"]);
        assert!(ex.never_run.is_empty());
    }

    #[test]
    fn the_docstring_of_the_module_is_read_under_its_runner_name() {
        let ex = extract("\">>> 1\"\n");
        assert_eq!(ex.snippets[0].owner, "<module>");
    }

    #[test]
    fn a_string_at_the_top_of_an_if_is_not_a_docstring() {
        let ex = extract("if True:\n    \">>> 1\"\n");
        assert!(ex.snippets.is_empty());
        assert!(ex.never_run.is_empty());
    }

    #[test]
    fn a_doctest_inside_a_function_never_runs() {
        let src = "def outer() -> None:\n    def inner() -> None:\n        \">>> inner()\"\n    class C:\n        def m(self) -> None:\n            \">>> 1\"\n    if True:\n        def deeper() -> None:\n            \"no tests\"\n";
        let ex = extract(src);
        assert!(ex.snippets.is_empty());
        let docs: Vec<&str> = ex.never_run.iter().map(|r| &src[*r]).collect();
        assert_eq!(docs, ["\">>> inner()\"", "\">>> 1\""]);
    }

    #[test]
    fn a_doctest_under_a_check_of_name_never_runs() {
        let src = "if __name__ == \"__main__\":\n    def a() -> None:\n        \">>> a()\"\nelif True:\n    class B:\n        def m(self) -> None:\n            \">>> 1\"\nwhile not __name__:\n    def c() -> None:\n        \">>> c()\"\nmatch x:\n    case 1 if __name__ == \"m\":\n        def d() -> None:\n            \">>> d()\"\nif x:\n    def e() -> None:\n        \">>> e()\"\n";
        let ex = extract(src);
        let docs: Vec<&str> = ex.under_name_check.iter().map(|r| &src[*r]).collect();
        assert_eq!(
            docs,
            ["\">>> a()\"", "\">>> 1\"", "\">>> c()\"", "\">>> d()\""]
        );
        let owners: Vec<&str> = ex
            .docstrings
            .iter()
            .map(|(owner, _)| owner.as_str())
            .collect();
        assert_eq!(owners, ["e"]);
        assert!(ex.never_run.is_empty());
    }

    #[test]
    fn a_doctest_directive_is_malformed() {
        assert!(has_directive("f()  # doctest: +ELLIPSIS"));
        assert!(has_directive("f()  #doctest:+SKIP"));
        assert!(!has_directive("print('# doctest: +SKIP')"));
        assert!(!has_directive("f()  # doctests: none"));
        let src = "def f() -> None:\n    \"\"\"\n    >>> f()  # doctest: +SKIP\n    \"\"\"\n";
        let ex = extract(src);
        assert_eq!(ex.malformed[0].kind, MalformedKind::Directive);
        assert_eq!(&src[ex.malformed[0].range], "    >>> f()  # doctest: +SKIP");
    }

    #[test]
    fn a_second_statement_on_a_later_line_is_malformed() {
        let line = |code: &str| {
            let example: Vec<(usize, String)> = code
                .split('\n')
                .enumerate()
                .map(|(i, code)| (i + 10, code.to_owned()))
                .collect();
            second_statement_line(&example)
        };
        assert_eq!(line("x = 1; y = 2"), None);
        assert_eq!(line("if True:\n    pass\n"), None);
        assert_eq!(line("x = 1\n# c"), None);
        assert_eq!(line("x = 1\ny = 2"), Some(11));
        assert_eq!(line("x = (1,\n2); y = 3"), Some(11));
        assert_eq!(line("if True:\n    pass\nx = 1"), Some(12));
    }

    /// The checker refuses the same examples as the single mode of the
    /// compiler of RustPython, on examples made of statements from a fixed
    /// seed.
    #[test]
    fn the_checker_refuses_a_second_statement_as_the_compiler_does() {
        const STATEMENTS: &[&str] = &[
            "x = 1",
            "y",
            "x = (1,\n2)",
            "x = 1 \\\n+ 2",
            "if x:\n    pass",
            "for i in x:\n    y",
            "@d\ndef f():\n    pass",
            "# c",
            "",
        ];
        const SEPARATORS: &[&str] = &["\n", "; ", ";", "\n\n", " \\\n", "  # c\n"];
        let mut state: u64 = 0x9e37_79b9_7f4a_7c15;
        let mut next = |n: usize| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            usize::try_from(state % n as u64).expect("below n")
        };
        let mut cases = String::new();
        for _ in 0..3000 {
            let mut source = String::from(STATEMENTS[next(STATEMENTS.len())]);
            for _ in 0..next(4) {
                source.push_str(SEPARATORS[next(SEPARATORS.len())]);
                source.push_str(STATEMENTS[next(STATEMENTS.len())]);
            }
            let example: Vec<(usize, String)> = source
                .split('\n')
                .enumerate()
                .map(|(i, code)| (i, code.to_owned()))
                .collect();
            if parse_module(&source).is_err() {
                continue;
            }
            let refused = second_statement_line(&example).is_some();
            cases.push_str(&format!(
                "    ({}, {}),\n",
                py_literal(&format!("{source}\n")),
                if refused { "True" } else { "False" }
            ));
        }
        let script = format!(
            "_CASES = [\n{cases}]\n{}",
            "for _src, _checker in _CASES:\n\
             \x20   try:\n\
             \x20       compile(_src, '<doctest>', 'single')\n\
             \x20       _runner = False\n\
             \x20   except SyntaxError as _e:\n\
             \x20       _runner = _e.msg == 'multiple statements found while compiling a single statement'\n\
             \x20   assert _runner == _checker, (_src, _runner, _checker)\n"
        );
        assert!(crate::run::run_source(&script, "single_test.py", ".").is_ok());
    }

    #[test]
    fn is_synthetic_fn_name_works() {
        assert!(is_synthetic_fn_name("__spython_doctest_0__"));
        assert!(is_synthetic_fn_name("__spython_doctest_42__"));
        assert!(!is_synthetic_fn_name("__spython_doctest____"));
        assert!(!is_synthetic_fn_name("add"));
        assert!(!is_synthetic_fn_name("__init__"));
    }
}
