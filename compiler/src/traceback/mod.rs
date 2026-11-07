//! The source lines of a frame of a traceback, with the carets under them,
//! as the `traceback` module of CPython 3.13 writes them. This is a port of
//! `StackSummary.format_frame_summary` and of the functions it calls, and
//! each step keeps the quirks of the original, so the output is the same
//! byte for byte.

mod wide;

use ruff_python_ast::{Expr, Stmt};
use ruff_python_parser::parse_module;
use ruff_text_size::{Ranged, TextRange, TextSize};

/// The traceback writes the first frames of a run of frames on the same
/// line, and then how many more there are.
pub(crate) const RECURSIVE_CUTOFF: i32 = 3;

/// The text CPython writes for a frame of the function `name`, in the file
/// at `path`, when the instruction that failed covers `range` of `source`.
pub(crate) fn frame(path: &str, name: &str, source: &str, range: TextRange) -> String {
    let (lineno, _) = FileLines::new(source).position(range.start());
    format!(
        "  File \"{path}\", line {lineno}, in {name}\n{}",
        source_lines(source, range)
    )
}

/// The number of the line where `range` of `source` starts, from 1.
pub(crate) fn line(source: &str, range: TextRange) -> usize {
    FileLines::new(source).position(range.start()).0
}

/// The lines CPython writes under the `File` line of a frame. Each line
/// ends in a newline, and an instruction on blank lines gives no line.
fn source_lines(source: &str, range: TextRange) -> String {
    let file = FileLines::new(source);
    let (lineno, colno) = file.position(range.start());
    let (end_lineno, end_colno) = file.position(range.end());
    let original: String = (lineno..=end_lineno)
        .map(|n| format!("{}\n", rstrip(file.line(n))))
        .collect();
    let dedented = dedent(&original);
    if strip(&dedented).is_empty() {
        return String::new();
    }
    frame_lines(&original, &dedented, end_lineno - lineno, colno, end_colno)
        .expect("the lines of a frame")
}

/// The lines of a file as `linecache` reads them, with the universal
/// newlines of the text mode.
struct FileLines<'a> {
    source: &'a str,
    /// The start and the end of each line, without its newline.
    lines: Vec<(usize, usize)>,
    /// The bytes of the byte order mark, which Python drops.
    bom: usize,
}

impl<'a> FileLines<'a> {
    fn new(source: &'a str) -> Self {
        let bom = if source.starts_with('\u{feff}') { 3 } else { 0 };
        let bytes = source.as_bytes();
        let mut lines = Vec::new();
        let mut start = bom;
        let mut i = bom;
        while let Some(&b) = bytes.get(i) {
            match b {
                b'\n' => {
                    lines.push((start, i));
                    start = i + 1;
                }
                b'\r' => {
                    lines.push((start, i));
                    if bytes.get(i + 1) == Some(&b'\n') {
                        i += 1;
                    }
                    start = i + 1;
                }
                _ => {}
            }
            i += 1;
        }
        lines.push((start, bytes.len()));
        FileLines { source, lines, bom }
    }

    /// The line, counted from 1, and the byte offset in the line, of
    /// `offset`.
    fn position(&self, offset: TextSize) -> (usize, usize) {
        let offset = usize::from(offset).max(self.bom);
        let index = self
            .lines
            .partition_point(|&(start, _)| start <= offset)
            .saturating_sub(1);
        let (start, _) = self.lines.get(index).copied().unwrap_or((0, 0));
        (index + 1, offset - start)
    }

    /// The line `n`, counted from 1, or `""` past the end as in `linecache`.
    fn line(&self, n: usize) -> &'a str {
        n.checked_sub(1)
            .and_then(|i| self.lines.get(i))
            .and_then(|&(start, end)| self.source.get(start..end))
            .unwrap_or("")
    }
}

/// The caret anchors of `_extract_caret_anchors_from_line_segment`.
struct Anchors {
    left_end_lineno: usize,
    left_end_offset: usize,
    right_start_lineno: usize,
    right_start_offset: usize,
}

const PRIMARY_CHAR: char = '~';
const SECONDARY_CHAR: char = '^';

/// The part of `format_frame_summary` after the `File` line. `None` is an
/// `IndexError` of Python, which the original would raise too.
fn frame_lines(
    original: &str,
    dedented: &str,
    last: usize,
    colno: usize,
    end_colno: usize,
) -> Option<String> {
    let all_lines_original = splitlines(original);
    let first_line: Vec<char> = all_lines_original.first()?.chars().collect();
    let last_line: Vec<char> = all_lines_original.get(last)?.chars().collect();
    let start_offset = byte_offset_to_character_offset(&first_line, colno);
    let end_offset = byte_offset_to_character_offset(&last_line, end_colno);

    let all_lines: Vec<Vec<char>> = splitlines(dedented)
        .into_iter()
        .take(last + 1)
        .map(|line| line.chars().collect())
        .collect();
    let first = all_lines.first()?;
    let last_of_all = all_lines.last()?;

    let dedent_characters = first_line.len() as isize - first.len() as isize;
    let start_offset = (start_offset as isize - dedent_characters).max(0) as usize;
    let end_offset = (end_offset as isize - dedent_characters).max(0) as usize;

    let dp_start_offset = display_width(first, start_offset);
    let dp_end_offset = display_width(last_of_all, end_offset);

    let joined: Vec<char> = join(&all_lines);
    let stop = (joined.len() + end_offset).saturating_sub(last_of_all.len());
    let segment: String = slice(&joined, start_offset, stop).iter().collect();

    let anchors = extract_caret_anchors(&segment);
    let show_carets = should_show_carets(start_offset, end_offset, &all_lines, anchors.is_some());

    let mut significant_lines = vec![0, all_lines.len() - 1];
    let mut anchor_offsets = None;
    if let Some(anchors) = &anchors {
        let mut left_end_offset = anchors.left_end_offset;
        let mut right_start_offset = anchors.right_start_offset;
        if anchors.left_end_lineno == 0 {
            left_end_offset += start_offset;
        }
        if anchors.right_start_lineno == 0 {
            right_start_offset += start_offset;
        }
        let left_end_offset =
            display_width(all_lines.get(anchors.left_end_lineno)?, left_end_offset);
        let right_start_offset = display_width(
            all_lines.get(anchors.right_start_lineno)?,
            right_start_offset,
        );
        anchor_offsets = Some((left_end_offset, right_start_offset));
        for lineno in [anchors.left_end_lineno, anchors.right_start_lineno] {
            significant_lines.extend(lineno.saturating_sub(1)..lineno + 2);
        }
    }
    significant_lines.retain(|&lineno| lineno < all_lines.len());
    significant_lines.sort_unstable();
    significant_lines.dedup();

    let output_line = |result: &mut String, lineno: usize| -> Option<()> {
        let line = all_lines.get(lineno)?;
        result.extend(line);
        result.push('\n');
        if !show_carets {
            return Some(());
        }
        let num_spaces = line.iter().take_while(|&&c| is_space(c)).count();
        let num_carets = if lineno == all_lines.len() - 1 {
            dp_end_offset
        } else {
            display_width(line, line.len())
        };
        for col in 0..num_carets {
            let within_anchors = match (&anchors, anchor_offsets) {
                (Some(a), Some((left, right))) => {
                    (lineno > a.left_end_lineno || (lineno == a.left_end_lineno && col >= left))
                        && (lineno < a.right_start_lineno
                            || (lineno == a.right_start_lineno && col < right))
                }
                _ => false,
            };
            result.push(
                if col < num_spaces || (lineno == 0 && col < dp_start_offset) {
                    ' '
                } else if within_anchors {
                    SECONDARY_CHAR
                } else if anchors.is_some() {
                    PRIMARY_CHAR
                } else {
                    // Without anchors, both characters are `^`.
                    SECONDARY_CHAR
                },
            );
        }
        result.push('\n');
        Some(())
    };

    let mut result = String::new();
    let mut previous = None;
    for &lineno in &significant_lines {
        if let Some(previous) = previous {
            let linediff = lineno - previous;
            if linediff == 2 {
                output_line(&mut result, lineno - 1)?;
            } else if linediff > 2 {
                result.push_str(&format!("...<{} lines>...\n", linediff - 1));
            }
        }
        output_line(&mut result, lineno)?;
        previous = Some(lineno);
    }
    Some(indent(&dedent(&result), "    "))
}

/// `_should_show_carets`.
fn should_show_carets(
    start_offset: usize,
    end_offset: usize,
    all_lines: &[Vec<char>],
    anchors: bool,
) -> bool {
    let text: String = join(all_lines).into_iter().collect();
    if let Ok(parsed) = parse_module(&text) {
        let Some(statement) = parsed.suite().first() else {
            return false;
        };
        let value = match statement {
            Stmt::Return(ret) => match ret.value.as_deref() {
                Some(value @ Expr::Call(call)) if call.func.is_name_expr() => Some(value),
                _ => None,
            },
            Stmt::Assign(assign) => match (&*assign.value, &*assign.targets) {
                (value @ Expr::Call(_), [Expr::Name(_)]) => Some(value),
                _ => None,
            },
            _ => None,
        };
        if let Some(value) = value {
            let lines = Positions::new(&text);
            let (lineno, col_offset) = lines.position(value.range().start());
            let (end_lineno, end_col_offset) = lines.position(value.range().end());
            if lineno == 1
                && end_lineno == all_lines.len()
                && col_offset == start_offset
                && end_col_offset == end_offset
            {
                return false;
            }
        }
    }
    if anchors {
        return true;
    }
    let before = all_lines
        .first()
        .map(|line| slice(line, 0, start_offset))
        .unwrap_or_default();
    let after = all_lines
        .last()
        .map(|line| slice(line, end_offset, line.len()))
        .unwrap_or_default();
    before.iter().any(|&c| !is_space(c)) || after.iter().any(|&c| !is_space(c))
}

/// `_extract_caret_anchors_from_line_segment`. `None` is a `SyntaxError`, an
/// `IndexError` or an `AssertionError`, which the caller ignores.
fn extract_caret_anchors(segment: &str) -> Option<Anchors> {
    let text = format!("(\n{segment}\n)");
    let parsed = parse_module(&text).ok()?;
    let [Stmt::Expr(statement)] = parsed.suite().as_slice() else {
        return None;
    };
    let lines: Vec<Vec<char>> = splitlines(segment)
        .into_iter()
        .map(|line| line.chars().collect())
        .collect();
    let positions = Positions::new(&text);
    let walker = Walker {
        lines: &lines,
        positions: &positions,
    };
    match &*statement.value {
        Expr::BinOp(binop) => {
            let (lineno, col) = walker.setup_positions(&binop.left, true)?;
            let (lineno, col) =
                walker.increment_until(lineno, col, |c| !is_space(c) && c != ')')?;
            let line = lines.get(lineno)?;
            let mut right_col = col + 1;
            let (right_lineno, right_col_offset) = positions.position(binop.right.range().start());
            let right_lineno = right_lineno.checked_sub(2)?;
            if right_col < line.len()
                && (right_lineno > lineno
                    || right_col < walker.normalize(right_lineno, right_col_offset)?)
                && line
                    .get(right_col)
                    .is_some_and(|&c| !is_space(c) && c != '\\' && c != '#')
            {
                right_col += 1;
            }
            Some(Anchors {
                left_end_lineno: lineno,
                left_end_offset: col,
                right_start_lineno: lineno,
                right_start_offset: right_col,
            })
        }
        Expr::Subscript(subscript) => walker.brackets(&subscript.value, &statement.value, '['),
        Expr::Call(call) => walker.brackets(&call.func, &statement.value, '('),
        _ => None,
    }
}

/// The helpers inside `_extract_caret_anchors_from_line_segment`.
struct Walker<'a> {
    lines: &'a [Vec<char>],
    positions: &'a Positions,
}

impl Walker<'_> {
    fn normalize(&self, lineno: usize, offset: usize) -> Option<usize> {
        Some(byte_offset_to_character_offset(
            self.lines.get(lineno)?,
            offset,
        ))
    }

    fn next_valid_char(&self, mut lineno: usize, mut col: usize) -> Option<(usize, usize)> {
        while let Some(line) = self.lines.get(lineno)
            && col >= line.len()
        {
            col = 0;
            lineno += 1;
        }
        let line = self.lines.get(lineno)?;
        (col < line.len()).then_some((lineno, col))
    }

    fn increment_until(
        &self,
        mut lineno: usize,
        mut col: usize,
        stop: impl Fn(char) -> bool,
    ) -> Option<(usize, usize)> {
        loop {
            let ch = *self.lines.get(lineno)?.get(col)?;
            if ch == '\\' || ch == '#' {
                (lineno, col) = self.next_valid_char(lineno + 1, 0)?;
            } else if !stop(ch) {
                (lineno, col) = self.next_valid_char(lineno, col + 1)?;
            } else {
                return Some((lineno, col));
            }
        }
    }

    /// The line and the column of the end of `expr`, moved to the next valid
    /// character when `force_valid` is `true`.
    fn setup_positions(&self, expr: &Expr, force_valid: bool) -> Option<(usize, usize)> {
        let (end_lineno, end_col_offset) = self.positions.position(expr.range().end());
        let lineno = end_lineno.checked_sub(2)?;
        let col = self.normalize(lineno, end_col_offset)?;
        if force_valid {
            self.next_valid_char(lineno, col)
        } else {
            Some((lineno, col))
        }
    }

    /// The anchors of a subscript or a call, from the `open` bracket after
    /// `inner` to the end of `outer`.
    fn brackets(&self, inner: &Expr, outer: &Expr, open: char) -> Option<Anchors> {
        let (left_lineno, left_col) = self.setup_positions(inner, true)?;
        let (left_lineno, left_col) = self.increment_until(left_lineno, left_col, |c| c == open)?;
        let (right_lineno, right_col) = self.setup_positions(outer, false)?;
        Some(Anchors {
            left_end_lineno: left_lineno,
            left_end_offset: left_col,
            right_start_lineno: right_lineno,
            right_start_offset: right_col,
        })
    }
}

/// The line, counted from 1, and the byte offset in the line, of an offset
/// in a text that the parser has read, as the `ast` module gives them.
struct Positions {
    starts: Vec<usize>,
}

impl Positions {
    fn new(text: &str) -> Self {
        let mut starts = vec![0];
        starts.extend(text.match_indices('\n').map(|(i, _)| i + 1));
        Positions { starts }
    }

    fn position(&self, offset: TextSize) -> (usize, usize) {
        let offset = usize::from(offset);
        let index = self
            .starts
            .partition_point(|&start| start <= offset)
            .saturating_sub(1);
        let start = self.starts.get(index).copied().unwrap_or(0);
        (index + 1, offset - start)
    }
}

/// `_byte_offset_to_character_offset`, which decodes the first `offset`
/// bytes and replaces a character cut in half.
fn byte_offset_to_character_offset(line: &[char], offset: usize) -> usize {
    let bytes = line.iter().collect::<String>().into_bytes();
    String::from_utf8_lossy(bytes.get(..offset).unwrap_or(&bytes))
        .chars()
        .count()
}

/// `_display_width`, the columns of the first `offset` characters of `line`.
fn display_width(line: &[char], offset: usize) -> usize {
    slice(line, 0, offset)
        .iter()
        .map(|&c| if is_wide(c) { 2 } else { 1 })
        .sum()
}

fn is_wide(c: char) -> bool {
    let c = u32::from(c);
    let i = wide::WIDE.partition_point(|&(_, last)| last < c);
    wide::WIDE.get(i).is_some_and(|&(first, _)| first <= c)
}

/// Returns `true` if `str.isspace` of Python is true for `c`, `false`
/// otherwise. Python counts the separators U+001C to U+001F, and Rust
/// does not.
fn is_space(c: char) -> bool {
    c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c)
}

fn rstrip(s: &str) -> &str {
    s.trim_end_matches(is_space)
}

fn strip(s: &str) -> &str {
    s.trim_matches(is_space)
}

/// `str.splitlines` of Python, which also breaks lines at the characters
/// that Python counts as line boundaries.
fn splitlines(s: &str) -> Vec<&str> {
    splitlines_keepends(s)
        .into_iter()
        .map(|line| line.trim_end_matches(is_line_boundary))
        .collect()
}

/// `str.splitlines(True)`, where each line keeps its line boundary.
fn splitlines_keepends(s: &str) -> Vec<&str> {
    let mut lines = Vec::new();
    let mut start = 0;
    let mut chars = s.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        if is_line_boundary(c) {
            let mut end = i + c.len_utf8();
            if c == '\r'
                && let Some(&(_, '\n')) = chars.peek()
            {
                chars.next();
                end += 1;
            }
            lines.push(s.get(start..end).unwrap_or(""));
            start = end;
        }
    }
    if start < s.len() {
        lines.push(s.get(start..).unwrap_or(""));
    }
    lines
}

fn is_line_boundary(c: char) -> bool {
    matches!(
        c,
        '\n' | '\r'
            | '\u{0b}'
            | '\u{0c}'
            | '\u{1c}'
            | '\u{1d}'
            | '\u{1e}'
            | '\u{85}'
            | '\u{2028}'
            | '\u{2029}'
    )
}

/// `textwrap.dedent`, which removes the longest prefix of spaces and tabs
/// common to every line that is not blank, and empties the blank lines.
fn dedent(text: &str) -> String {
    let lines: Vec<&str> = text
        .split('\n')
        .map(|line| {
            if !line.is_empty() && line.chars().all(|c| c == ' ' || c == '\t') {
                ""
            } else {
                line
            }
        })
        .collect();
    let mut margin: Option<&str> = None;
    for line in &lines {
        let indent_len = line.len() - line.trim_start_matches([' ', '\t']).len();
        if indent_len == line.len() {
            continue;
        }
        let indent = line.get(..indent_len).unwrap_or("");
        margin = Some(match margin {
            None => indent,
            Some(margin) => {
                let common = margin
                    .bytes()
                    .zip(indent.bytes())
                    .take_while(|(a, b)| a == b)
                    .count();
                margin.get(..common).unwrap_or("")
            }
        });
    }
    let margin = margin.unwrap_or("");
    lines
        .iter()
        .map(|line| line.strip_prefix(margin).unwrap_or(line))
        .collect::<Vec<_>>()
        .join("\n")
}

/// `textwrap.indent` with a predicate that is always true.
fn indent(text: &str, prefix: &str) -> String {
    splitlines_keepends(text)
        .into_iter()
        .map(|line| format!("{prefix}{line}"))
        .collect()
}

fn join(lines: &[Vec<char>]) -> Vec<char> {
    let mut joined = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        if i > 0 {
            joined.push('\n');
        }
        joined.extend(line);
    }
    joined
}

/// `s[start:stop]` of Python, for a `start` and a `stop` that are not
/// negative.
fn slice(s: &[char], start: usize, stop: usize) -> Vec<char> {
    let stop = stop.min(s.len());
    s.get(start.min(stop)..stop).unwrap_or_default().to_vec()
}

#[cfg(test)]
mod tests;
