//! Doctest extraction, format validation, and type-check scaffolding.
//!
//! Walks the AST to find docstrings attached to the module, functions,
//! classes, and methods, extracts doctest snippets from them, and detects
//! malformed `>>>` / `...` prompts (e.g. `>>>foo` — a common student mistake
//! that causes the doctest runner to silently skip the line).
//!
//! A synthetic Python source can be assembled from a file's original text
//! plus one `def __spython_doctest_<N>__() -> None:` stub per snippet; that
//! synthetic source is then type-checked with the usual pipeline. Diagnostics
//! coming out of the synthetic functions are remapped back to the original
//! docstring locations via `SyntheticMap` / `remap_diagnostics`.

use ruff_db::diagnostic::{Annotation, Diagnostic, Span};
use ruff_db::files::File;
use ruff_python_ast::{Expr, ModModule, Stmt, StmtClassDef, StmtFunctionDef};
use ruff_text_size::{TextRange, TextSize};

use crate::checker::make_lint_diagnostic;
use crate::lints::DOCTEST_MALFORMED_PROMPT;

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

/// A single doctest example extracted from a docstring: the executable source
/// (concatenated `>>>`/`...` lines with the prompt stripped) plus per-line
/// provenance back to the original file.
#[derive(Debug, Clone)]
pub struct DoctestSnippet {
    /// Human-friendly name of the doctest's container: `"module"`, a function
    /// name, or `"Class.method"`. Used in diagnostic messages.
    pub owner: String,
    /// Executable code with prompts (`>>> ` / `... `) removed, joined by `\n`.
    pub code: String,
    /// The range in the original file of the first `>>>` line's content
    /// (after the prompt). Used as a fallback when a diagnostic can't be
    /// mapped to a specific line.
    pub anchor_range: TextRange,
    /// For each line of `code`, the range in the original file of that
    /// line's content (after the prompt). `line_ranges.len() == code.lines().count()`.
    pub line_ranges: Vec<TextRange>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptKind {
    Primary,
    Continuation,
}

/// A docstring line that starts with `>>>` or `...` but is not followed by
/// a single space — the doctest runner silently ignores these, hiding the
/// fact that the test never ran.
#[derive(Debug, Clone)]
pub struct MalformedPrompt {
    pub range: TextRange,
    pub kind: PromptKind,
}

/// Result of walking a module for doctests.
#[derive(Debug, Default)]
pub struct DoctestExtraction {
    pub snippets: Vec<DoctestSnippet>,
    pub malformed: Vec<MalformedPrompt>,
}

/// Walk `module` looking for docstrings and extract doctest snippets from them.
/// `source` must be the exact source text the module was parsed from.
pub fn extract_doctests(module: &ModModule, source: &str) -> DoctestExtraction {
    let mut out = DoctestExtraction::default();
    walk_stmts(&module.body, source, "module", &mut out);
    out
}

fn walk_stmts(stmts: &[Stmt], source: &str, scope: &str, out: &mut DoctestExtraction) {
    // The docstring is the very first statement (if any) when it is a plain
    // string-literal expression statement.
    if let Some(first) = stmts.first()
        && let Stmt::Expr(expr_stmt) = first
        && let Expr::StringLiteral(s) = &*expr_stmt.value
    {
        // Skip implicitly concatenated docstrings: we can't cleanly map
        // offsets back to the original source when parts are joined.
        if !s.value.is_implicit_concatenated() {
            let part = s
                .value
                .iter()
                .next()
                .expect("non-concatenated string has exactly one part");
            scan_docstring(part.content_range(), source, scope, out);
        }
    }
    for stmt in stmts {
        match stmt {
            Stmt::FunctionDef(func) => walk_function(func, source, scope, out),
            Stmt::ClassDef(cls) => walk_class(cls, source, scope, out),
            _ => {}
        }
    }
}

fn walk_function(func: &StmtFunctionDef, source: &str, scope: &str, out: &mut DoctestExtraction) {
    let name = if scope == "module" {
        func.name.to_string()
    } else {
        format!("{scope}.{}", func.name)
    };
    walk_stmts(&func.body, source, &name, out);
}

fn walk_class(cls: &StmtClassDef, source: &str, scope: &str, out: &mut DoctestExtraction) {
    let name = if scope == "module" {
        cls.name.to_string()
    } else {
        format!("{scope}.{}", cls.name)
    };
    // Collect a class docstring under the class name.
    if let Some(first) = cls.body.first()
        && let Stmt::Expr(expr_stmt) = first
        && let Expr::StringLiteral(s) = &*expr_stmt.value
        && !s.value.is_implicit_concatenated()
    {
        let part = s
            .value
            .iter()
            .next()
            .expect("non-concatenated string has exactly one part");
        scan_docstring(part.content_range(), source, &name, out);
    }
    for stmt in &cls.body {
        match stmt {
            Stmt::FunctionDef(func) => walk_function(func, source, &name, out),
            Stmt::ClassDef(inner) => walk_class(inner, source, &name, out),
            _ => {}
        }
    }
}

/// Scan the content of one docstring (range = its content_range in `source`)
/// for doctest examples and malformed prompts.
///
/// All examples found in this docstring are merged into a single
/// `DoctestSnippet` — this mirrors the runtime semantics where every
/// `>>>` example within a docstring shares the same `vars(module)` scope,
/// so names defined by earlier examples are visible to later ones.
fn scan_docstring(
    content_range: TextRange,
    source: &str,
    owner: &str,
    out: &mut DoctestExtraction,
) {
    let content_start: usize = content_range.start().into();
    let content_end: usize = content_range.end().into();
    let content = &source[content_start..content_end];

    // Precompute line ranges (byte offset inside `content`).
    let mut lines: Vec<(usize, usize, &str)> = Vec::new(); // (start, end_excl_newline, text)
    let mut offset = 0usize;
    for line in content.split_inclusive('\n') {
        let trimmed_end = if line.ends_with("\r\n") {
            line.len() - 2
        } else if line.ends_with('\n') {
            line.len() - 1
        } else {
            line.len()
        };
        lines.push((offset, offset + trimmed_end, &line[..trimmed_end]));
        offset += line.len();
    }

    let mut code_lines: Vec<String> = Vec::new();
    let mut line_ranges: Vec<TextRange> = Vec::new();
    let mut anchor_range: Option<TextRange> = None;

    let mut i = 0;
    while i < lines.len() {
        let (line_start, line_end, line_text) = lines[i];
        let stripped = line_text.trim_start();
        let leading = line_text.len() - stripped.len();

        let primary = starts_with_prompt(stripped, ">>>");
        if let Some(kind) = primary {
            let full_line_range = TextRange::new(
                TextSize::try_from(content_start + line_start).unwrap(),
                TextSize::try_from(content_start + line_end).unwrap(),
            );
            match kind {
                PromptClass::Valid => {
                    let content_after_prompt =
                        TextSize::try_from(content_start + line_start + leading + 4).unwrap();
                    let code_line_range = TextRange::new(
                        content_after_prompt,
                        TextSize::try_from(content_start + line_end).unwrap(),
                    );
                    code_lines.push(stripped[4..].to_string());
                    line_ranges.push(code_line_range);
                    if anchor_range.is_none() {
                        anchor_range = Some(code_line_range);
                    }
                    i += 1;
                    // Continuation `... ` lines.
                    while i < lines.len() {
                        let (cs, ce, ct) = lines[i];
                        let cstrip = ct.trim_start();
                        let cleading = ct.len() - cstrip.len();
                        match starts_with_prompt(cstrip, "...") {
                            Some(PromptClass::Valid) => {
                                let after =
                                    TextSize::try_from(content_start + cs + cleading + 4).unwrap();
                                code_lines.push(cstrip[4..].to_string());
                                line_ranges.push(TextRange::new(
                                    after,
                                    TextSize::try_from(content_start + ce).unwrap(),
                                ));
                                i += 1;
                            }
                            Some(PromptClass::MalformedEmpty) => {
                                let after =
                                    TextSize::try_from(content_start + cs + cleading + 3).unwrap();
                                code_lines.push(String::new());
                                line_ranges.push(TextRange::new(after, after));
                                i += 1;
                            }
                            Some(PromptClass::Malformed) => {
                                out.malformed.push(MalformedPrompt {
                                    range: TextRange::new(
                                        TextSize::try_from(content_start + cs).unwrap(),
                                        TextSize::try_from(content_start + ce).unwrap(),
                                    ),
                                    kind: PromptKind::Continuation,
                                });
                                i += 1;
                                break;
                            }
                            None => break,
                        }
                    }
                }
                PromptClass::MalformedEmpty => {
                    // bare `>>>` on its own — treat as an empty line in the
                    // accumulated snippet so the line count stays stable.
                    i += 1;
                }
                PromptClass::Malformed => {
                    out.malformed.push(MalformedPrompt {
                        range: full_line_range,
                        kind: PromptKind::Primary,
                    });
                    i += 1;
                }
            }
        } else {
            i += 1;
        }
    }

    if let Some(anchor) = anchor_range {
        out.snippets.push(DoctestSnippet {
            owner: owner.to_string(),
            code: code_lines.join("\n"),
            anchor_range: anchor,
            line_ranges,
        });
    }
}

enum PromptClass {
    /// `>>> foo` or `... foo` — prompt followed by exactly one space and content.
    Valid,
    /// `>>>` / `...` alone (possibly with trailing whitespace-only text).
    MalformedEmpty,
    /// `>>>foo` / `...foo` — no space after prompt (student mistake).
    Malformed,
}

/// Classify the start of `stripped` with respect to `prompt` (`">>>"` or `"..."`).
/// Returns `None` if the line doesn't start with the prompt at all.
fn starts_with_prompt(stripped: &str, prompt: &str) -> Option<PromptClass> {
    if !stripped.starts_with(prompt) {
        return None;
    }
    let rest = &stripped[prompt.len()..];
    if rest.is_empty() {
        return Some(PromptClass::MalformedEmpty);
    }
    // Exactly one space, followed by content (or end).
    if let Some(stripped_rest) = rest.strip_prefix(' ') {
        // Reject `>>>  foo` (two or more spaces) as well? We accept it: the
        // runner strips `>>> ` (4 chars) and treats the extra space as
        // indentation. Be lenient to match stdlib `doctest`.
        let _ = stripped_rest;
        return Some(PromptClass::Valid);
    }
    Some(PromptClass::Malformed)
}

/// Build diagnostics for each malformed-prompt finding.
pub fn malformed_prompt_diagnostics(file: File, malformed: &[MalformedPrompt]) -> Vec<Diagnostic> {
    malformed
        .iter()
        .map(|m| {
            let msg = match m.kind {
                PromptKind::Primary => {
                    "Doctest prompt must be followed by a single space: change `>>>x` to `>>> x`"
                }
                PromptKind::Continuation => {
                    "Doctest continuation must be followed by a single space: change `...x` to `... x`"
                }
            };
            make_lint_diagnostic(&DOCTEST_MALFORMED_PROMPT, file, m.range, msg.to_string())
        })
        .collect()
}

// --- Synthetic source + diagnostic remapping --------------------------------

/// Maps lines of the synthetic source (produced by `build_synthetic_source`)
/// back to the original snippets.
#[derive(Debug)]
pub struct SyntheticMap {
    /// For each 0-based line index in the synthetic source:
    /// * `Some((snippet_idx, line_idx_in_snippet))` if the line came from a doctest snippet;
    /// * `None` for verbatim-original lines, blank separators, and `def` headers.
    pub lines: Vec<Option<(usize, usize)>>,
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
    let original_len = u32::try_from(syn.len()).unwrap();

    // One `None` per line of the original (including the newline we may have added).
    let original_line_count = syn.lines().count();
    let mut line_map: Vec<Option<(usize, usize)>> = vec![None; original_line_count];

    for (idx, snippet) in snippets.iter().enumerate() {
        // Blank separator line.
        syn.push('\n');
        line_map.push(None);
        // `def __spython_doctest_N__() -> None:`
        syn.push_str("def ");
        syn.push_str(SYNTHETIC_FN_PREFIX);
        syn.push_str(&idx.to_string());
        syn.push_str(SYNTHETIC_FN_SUFFIX);
        syn.push_str("() -> None:\n");
        line_map.push(None); // the `def` header line

        let body_lines: Vec<&str> = snippet.code.split('\n').collect();
        if body_lines.is_empty() || body_lines.iter().all(|l| l.is_empty()) {
            syn.push_str("    pass\n");
            line_map.push(None);
        } else {
            for (line_idx, body_line) in body_lines.iter().enumerate() {
                syn.push_str("    ");
                syn.push_str(body_line);
                syn.push('\n');
                line_map.push(Some((idx, line_idx)));
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
/// * drop any whose primary line didn't come from a doctest snippet (e.g. the
///   `def` header);
/// * rewrite the primary span to point at the original docstring location and
///   prepend `"in doctest of <owner>: "` to the message.
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
            line_starts.push(u32::try_from(i + 1).unwrap());
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
        let line_idx = byte_to_line(prim_start);
        let Some(Some((snippet_idx, line_in_snippet))) = map.lines.get(line_idx).copied() else {
            continue;
        };
        let snippet = &snippets[snippet_idx];
        let original_range = snippet
            .line_ranges
            .get(line_in_snippet)
            .copied()
            .unwrap_or(snippet.anchor_range);

        let id = diag.id();
        let severity = diag.severity();
        let old_msg = diag.primary_message().to_string();
        let new_msg = format!("in doctest of {}: {old_msg}", snippet.owner);
        let mut rebuilt = Diagnostic::new(id, severity, new_msg);
        rebuilt.annotate(Annotation::primary(
            Span::from(original_file).with_range(original_range),
        ));
        out.push(rebuilt);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use ruff_python_parser::parse_module;

    fn extract(src: &str) -> DoctestExtraction {
        let parsed = parse_module(src).unwrap();
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
        assert_eq!(ex.malformed[0].kind, PromptKind::Primary);
    }

    #[test]
    fn malformed_continuation() {
        let src = "def f(x: int) -> int:\n    \"\"\"\n    >>> y = (\n    ...x\n    ... )\n    \"\"\"\n    return x\n";
        let ex = extract(src);
        // The `>>>` example is started but the malformed continuation stops it.
        assert!(
            ex.malformed
                .iter()
                .any(|m| m.kind == PromptKind::Continuation)
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
        assert!(map.lines.contains(&Some((0, 0))));
        assert!(map.lines.contains(&Some((0, 1))));
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
