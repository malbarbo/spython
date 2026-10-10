//! Whether a REPL input is finished, and how far in the next line starts.

use ruff_python_ast::helpers::is_compound_statement;
use ruff_python_ast::token::{TokenKind, Tokens};
use ruff_python_ast::{ModModule, PySourceType};
use ruff_python_parser::{
    InterpolatedStringErrorType, LexicalErrorType, ParseErrorType, Parsed, parse_unchecked_source,
};
use ruff_text_size::{Ranged, TextSize};

/// What one level of indentation is worth, in spaces. Enter, Tab and
/// Backspace all move by it, so the three agree on where a block is written.
pub const INDENT: usize = 4;

/// What the prompt does with the Python as it stands: `-1` runs it, and
/// anything else is how far in the line the cursor opens starts, in spaces.
/// A `:` command is `shell::ready_state`'s to read, and what it carries
/// arrives here on its own.
///
/// `cursor` is a byte offset into `src`, as `repl_complete` already takes
/// one. A host that counts in another unit can send one that falls inside a
/// character, and that one moves back to the boundary before it. Slicing at
/// it would panic, and a panic is the end of the session.
///
/// Input that no line could finish is run, so that the user is given the
/// error instead of a prompt that will not let go of them.
pub fn ready_state(src: &str, cursor: usize) -> i32 {
    let cursor = char_boundary(src, cursor);
    // Nothing but whitespace is left after the cursor: the line it opens is
    // the line after the input, which the input alone already answers.
    let at_end = src[cursor..].trim().is_empty();
    // One line runs from anywhere in it, the way a prompt always has. Several
    // run from the end only: with the cursor back in the text, Enter is
    // opening a line inside a block still being written, and running the
    // block because it happens to be finished is not what the key was
    // pressed for.
    if (at_end || !src.contains('\n')) && !incomplete(src) {
        return -1;
    }
    let indent = indent_at(src, if at_end { src.len() } else { cursor });
    i32::try_from(indent).unwrap_or(0)
}

/// How far in the line opened at `cursor` starts, in spaces: as far in as the
/// block open above it puts the line, and at least as far as the code under
/// it is written.
///
/// The two halves answer different things. Above the cursor the tokens say
/// which block is open, which is a floor: a line out of it is not in the
/// block at all. Below it only the indentation as written counts, and it is
/// a floor too. A line shallower than the code under it closes a block that
/// code still needs open. So the answer is the deeper of the two, and where
/// they disagree it is the one that closes nothing.
///
/// Also what Tab indents by, through `completion`.
pub(crate) fn indent_at(src: &str, cursor: usize) -> usize {
    let cursor = char_boundary(src, cursor);
    let above = &src[..cursor];
    let parsed = parse_unchecked_source(above, PySourceType::Python);
    indent_after(parsed.tokens(), above).max(indent_below(src, cursor))
}

/// Returns `true` if the reader has to go on reading, `false` otherwise.
/// It serves a caller with no parse in hand.
fn incomplete(src: &str) -> bool {
    is_incomplete(&parse_unchecked_source(src, PySourceType::Python), src)
}

/// `cursor` brought inside `src` and onto a character boundary.
pub(crate) fn char_boundary(src: &str, cursor: usize) -> usize {
    let mut cursor = cursor.min(src.len());
    while !src.is_char_boundary(cursor) {
        cursor -= 1;
    }
    cursor
}

/// How far in the code under the cursor is written, in spaces: the first line
/// below the cursor's own that says something and can be measured. A blank
/// line and a comment say nothing about where the block is written, and a line
/// indented with anything but spaces cannot be measured in them: all three are
/// passed over.
fn indent_below(src: &str, cursor: usize) -> usize {
    let Some((_, below)) = src[cursor..].split_once('\n') else {
        return 0;
    };
    below
        .lines()
        .find_map(|line| {
            let text = line.trim_start_matches(' ');
            (!text.is_empty() && !text.starts_with('#') && !text.starts_with(char::is_whitespace))
                .then(|| indent_of(line))
        })
        .unwrap_or(0)
}

/// Returns `true` if the reader has to go on reading before this input can
/// run, `false` otherwise.
///
/// Where the input ends is not where it stops, and the text alone does not
/// say. A bracket inside a comment closes nothing.
///
/// The kind of error does not say it either. `x = ` and `x = )` are the same
/// `ExpectedExpression`, as the parser reports where it wanted a value. What
/// tells them apart is whether any of the input is left after that: if none
/// is, there was nothing to reject.
///
/// A wrong `true` is the costly one. It hangs the prompt on a stray `)`, and
/// only Ctrl-C gets out, so an error that says neither is taken as finished.
fn is_incomplete(parsed: &Parsed<ModModule>, src: &str) -> bool {
    // The parser recovers from a bad token and goes on, so the errors after
    // the first are about what it made of the rest. Only the first one says
    // where the input stopped being read.
    let Some(err) = parsed.errors().first() else {
        // It parses, and a block goes on until a blank line ends it.
        return parsed
            .syntax()
            .body
            .last()
            .is_some_and(is_compound_statement)
            && !ends_with_blank_line(src);
    };
    match &err.error {
        // The lexer reached the end inside a bracket still open, where a
        // blank line is content and not the end of the input.
        ParseErrorType::Lexical(LexicalErrorType::Eof) => true,
        // Only a string of three quotes runs into the next line. One of a
        // single quote is over at the end of the line, wherever it stops.
        ParseErrorType::Lexical(LexicalErrorType::UnclosedStringError) => {
            is_triple_quoted(&src[err.location])
        }
        ParseErrorType::Lexical(
            LexicalErrorType::FStringError(
                InterpolatedStringErrorType::UnterminatedTripleQuotedString,
            )
            | LexicalErrorType::TStringError(
                InterpolatedStringErrorType::UnterminatedTripleQuotedString,
            ),
        ) => true,
        // A backslash at the end joins the next line onto this one.
        ParseErrorType::Lexical(LexicalErrorType::LineContinuationError) => {
            err.location.end() >= end_of_text(src)
        }
        // What the input stops in the middle of: an indented block after a
        // header, a `def` or `class` after a decorator, a name after a dot.
        // A blank line says the user has nothing more to give.
        ParseErrorType::OtherError(_) => {
            ends_there(parsed.tokens(), err.location.start()) && !ends_with_blank_line(src)
        }
        // A token was left to reject: the input is finished and wrong.
        _ => false,
    }
}

/// How far in the next line starts, in spaces: one level deeper than the line
/// that opens a block or leaves a bracket open, one out from the line that
/// ends a block, and level with the line it follows otherwise.
///
/// Read from the tokens, so a colon or a bracket inside a comment or a string
/// is text. A bracket is worth one level however many are opened at once, so
/// `print(f(` is a line to indent, not two.
fn indent_after(tokens: &Tokens, src: &str) -> usize {
    let line_start = src.rfind('\n').map_or(0, |i| i + 1);
    let line = &src[line_start..];
    let layout = layout(tokens, src, line_start);
    if layout.depth > layout.depth_at_last_line {
        // The line left a bracket open, and what goes in it hangs off where
        // that line starts.
        return indent_of(line) + INDENT;
    }
    if layout.depth > 0 {
        // Inside brackets, where the lines above have laid out the input and
        // a colon is a dict's or a slice's.
        return indent_of(line);
    }
    // Out of brackets it is the statement that is indented, and a statement
    // starts where its first line does, not where its last one does.
    let logical = line_at(src, layout.starts.last().copied().unwrap_or(0));
    let indent = indent_of(logical);
    if layout.last_code == Some(TokenKind::Colon) {
        indent + INDENT
    } else if indent > 0 && leaves_block(logical) {
        enclosing_indent(src, &layout.starts, indent)
    } else {
        indent
    }
}

/// What the tokens say about the shape of the input.
struct Layout {
    /// Where each statement begins, in order. A line break inside brackets
    /// does not begin one, and neither does one after a backslash, which the
    /// lexer answers by not ending the line. A comment is not a statement
    /// either: the line it is written on is still the one above it.
    starts: Vec<usize>,
    /// How many brackets are open where the last line starts.
    depth_at_last_line: i32,
    /// How many are open where the input ends.
    depth: i32,
    /// The last token that is not layout.
    last_code: Option<TokenKind>,
}

/// Read the shape of the input off the tokens, in one walk over them.
fn layout(tokens: &Tokens, src: &str, line_start: usize) -> Layout {
    let mut layout = Layout {
        starts: vec![0],
        depth_at_last_line: 0,
        depth: 0,
        last_code: None,
    };
    let mut next = None;
    for token in tokens.iter() {
        let kind = token.kind();
        match kind {
            TokenKind::Lpar | TokenKind::Lsqb | TokenKind::Lbrace => layout.depth += 1,
            TokenKind::Rpar | TokenKind::Rsqb | TokenKind::Rbrace => layout.depth -= 1,
            // The break the lexer ends the input with is not a line break:
            // there is no line after it to start.
            TokenKind::Newline | TokenKind::NonLogicalNewline if layout.depth <= 0 => {
                let end = usize::from(token.range().end());
                if end < src.len() {
                    next = Some(end);
                }
            }
            _ => {}
        }
        if is_code(kind) {
            layout.last_code = Some(kind);
            // The line only begins a statement once something is written on it.
            if let Some(start) = next.take() {
                layout.starts.push(start);
            }
        }
        if usize::from(token.range().start()) < line_start {
            layout.depth_at_last_line = layout.depth;
        }
    }
    layout
}

/// The spaces the line starts with.
fn indent_of(line: &str) -> usize {
    line.len() - line.trim_start_matches(' ').len()
}

fn line_at(src: &str, start: usize) -> &str {
    let rest = src.get(start..).unwrap_or("");
    rest.split_once('\n').map_or(rest, |(line, _)| line)
}

/// Where the block holding the one that ends at `indent` is written, in
/// spaces: the innermost statement above it that starts further out. Read
/// from the input and not by taking a level off, which asks the user to go
/// back to a column that may hold no block at all.
fn enclosing_indent(src: &str, starts: &[usize], indent: usize) -> usize {
    starts
        .iter()
        .map(|&start| line_at(src, start))
        .filter(|line| !line.trim().is_empty())
        .map(indent_of)
        .filter(|&i| i < indent)
        .max()
        .unwrap_or(0)
}

/// Returns `true` if the string under the error is written with three
/// quotes, `false` otherwise. The parser reports the error from where the
/// string opens, `rb'''` and all.
fn is_triple_quoted(text: &str) -> bool {
    let quotes = text.trim_start_matches(char::is_alphabetic);
    quotes.starts_with("\"\"\"") || quotes.starts_with("'''")
}

/// Returns `true` if the line is the last one its block can have, `false`
/// otherwise. Nothing follows a `return` or a `pass` at the same indentation.
fn leaves_block(line: &str) -> bool {
    matches!(
        line.split_whitespace().next(),
        Some("return" | "pass" | "break" | "continue" | "raise")
    )
}

/// Returns `true` if nothing of the input comes after `at`, `false`
/// otherwise. A comment and a blank line do not count. They are what the
/// user typed before going on.
fn ends_there(tokens: &Tokens, at: TextSize) -> bool {
    tokens
        .iter()
        .filter(|t| is_code(t.kind()))
        .all(|t| t.range().start() < at)
}

/// Returns `true` if the token is input and not layout, `false` otherwise.
fn is_code(kind: TokenKind) -> bool {
    !matches!(
        kind,
        TokenKind::Comment
            | TokenKind::Newline
            | TokenKind::NonLogicalNewline
            | TokenKind::Indent
            | TokenKind::Dedent
            | TokenKind::EndOfFile
    )
}

/// Returns `true` if the user ended the input with a line that says nothing,
/// `false` otherwise. Such a line closes a block, and it gives up a prompt
/// that waits for one.
fn ends_with_blank_line(src: &str) -> bool {
    src.rsplit_once('\n')
        .is_some_and(|(_, last)| last.trim().is_empty())
}

/// Offset past the last character that is not trailing whitespace.
fn end_of_text(src: &str) -> TextSize {
    TextSize::try_from(src.trim_end().len()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Returns `true` if `src` is unfinished, `false` otherwise. The tests ask
    /// about a string, and only `ready_state` has a parse in hand.
    fn is_incomplete(src: &str) -> bool {
        super::is_incomplete(&parse_unchecked_source(src, PySourceType::Python), src)
    }

    /// The cursor at the end of the input, which is where the tests written
    /// before there was a cursor put it.
    fn ready_state(src: &str) -> i32 {
        super::ready_state(src, src.len())
    }

    /// Likewise: how far in the line after the whole input starts.
    fn continuation_indent(src: &str) -> usize {
        indent_at(src, src.len())
    }

    /// The `|` in `marked` is where the cursor is.
    fn at_cursor(marked: &str) -> i32 {
        let (above, below) = marked.split_once('|').expect("the cursor is a `|`");
        super::ready_state(&format!("{above}{below}"), above.len())
    }

    #[test]
    fn an_input_that_stops_where_it_asked_for_more() {
        for src in [
            // Something is still open.
            "print(",
            "x = [1,",
            "x = [1,\n    2,",
            "x = {",
            "x = (1 +",
            "x = '''abc",
            "x = rb'''abc",
            "x = f\"\"\"abc",
            "s = '''a\nb",
            "x = 1 + \\\n",
            "x = 1 + \\",
            // A block, and the decorator above one.
            "def f():",
            "def f(x: int) -> int:",
            "if True:",
            "class A:",
            "match x:",
            "for i in range(3):",
            "while True:",
            "with open(\"a\") as f:",
            "if True:\n    pass\nelse:",
            "try:\n    pass\nexcept:",
            "@dataclass",
            "@dataclass\nclass A:",
            // What the user typed before going on is not what comes after.
            "def f():  # soma",
            "def f():\n    # soma",
            // It parses, and the block goes on.
            "def f() -> int:\n    return 1",
            "if True:\n    pass",
            "class A:\n    x: int = 1",
            "if True: pass",
        ] {
            assert!(is_incomplete(src), "{src:?}");
        }
    }

    #[test]
    fn an_input_that_is_finished() {
        for src in [
            // A token was left to reject.
            "x = )",
            "x = ) 1",
            "x = 1 +)",
            ")",
            "}",
            "x = 1 )",
            "x = 1 1",
            "def f() 1",
            "1 +\n2",
            "else:",
            "elif x:",
            "    x = 1",
            "x = 1\n  y = 2",
            // A string of one quote does not run into the next line.
            "x = \"abc",
            "x = r\"abc",
            "x = \"a\" \"b",
            "x = f\"abc",
            "f\"{x",
            // A blank line ends what was waiting for a body.
            "def f():\n",
            "@dataclass\n",
            "x = a.\n",
            // An f-string that no line can close.
            "f\"{x!q}\"",
            "f\"{x:\"",
            "x = 1 + \\\n2",
            // Finished and right.
            "",
            "   ",
            "x = 1",
            "1 + 1",
            "print(\"a\")",
            "x = [1,\n    2]",
            "# comentario",
            "x = 1  # nota:",
            "print(1)  # c:",
            "s = \"\"\"\ntitulo:\n\"\"\"",
            "def f() -> int:\n    return 1\n",
            "if True:\n    pass\n\n",
            "def f() -> int:\n    return 1\n    ",
        ] {
            assert!(!is_incomplete(src), "{src:?}");
        }
    }

    #[test]
    fn the_next_line_is_indented_by_the_block_it_is_in() {
        assert_eq!(continuation_indent("def f() -> int:"), 4);
        assert_eq!(continuation_indent("def f() -> int:\n    x = 1"), 4);
        assert_eq!(continuation_indent("def f() -> int:\n    return 1"), 0);
        assert_eq!(
            continuation_indent("def f() -> int:\n    if x:\n        return 1"),
            4
        );
        assert_eq!(continuation_indent("s = \"\"\"\ntitulo:"), 0);
        assert_eq!(continuation_indent("def f() -> int:\n    if x:"), 8);
        assert_eq!(continuation_indent("x = 1"), 0);
        assert_eq!(continuation_indent("x = 1  # a:"), 0);
        assert_eq!(continuation_indent("s = \"a:\""), 0);
    }

    #[test]
    fn a_bracket_left_open_is_a_level_of_its_own() {
        assert_eq!(continuation_indent("x = ["), 4);
        assert_eq!(continuation_indent("print("), 4);
        // However many are opened at once.
        assert_eq!(continuation_indent("print(f("), 4);
        // A line inside brackets is where the one before it left off.
        assert_eq!(continuation_indent("x = [\n    1,"), 4);
        assert_eq!(continuation_indent("x = [\n    "), 4);
        // A colon inside brackets is a dict's, not a block's.
        assert_eq!(continuation_indent("d = {1:"), 4);
        assert_eq!(continuation_indent("d = {\n    1:"), 4);
        // And one inside a block goes a level deeper still.
        assert_eq!(continuation_indent("def f() -> None:\n    print("), 8);
        // A bracket opened inside another hangs off the line it is on.
        assert_eq!(continuation_indent("x = [\n    f("), 8);
    }

    #[test]
    fn a_comment_is_where_the_line_above_it_is() {
        // The block is opened by the header, and the comment inside it is
        // already there: the next line is level with it, not in from it.
        assert_eq!(continuation_indent("def f() -> int:\n    # soma"), 4);
        assert_eq!(continuation_indent("class A:\n    # doc"), 4);
        assert_eq!(
            continuation_indent("def f() -> int:\n    x = 1\n    # nota"),
            4
        );
        assert_eq!(
            continuation_indent("def f() -> int:\n    return 1\n    # nota"),
            0
        );
    }

    #[test]
    fn a_block_ends_back_where_the_one_around_it_is() {
        // Not a level off the indentation, which would leave the user in a
        // column no block is written at.
        assert_eq!(continuation_indent("if True:\n        pass"), 0);
        assert_eq!(continuation_indent("if True:\n  pass"), 0);
        assert_eq!(
            continuation_indent("def f() -> None:\n  if x:\n      return"),
            2
        );
    }

    #[test]
    fn a_statement_is_indented_where_it_starts() {
        // The line that closes the call is not where the statement began.
        assert_eq!(
            continuation_indent("def f() -> None:\n    print(\n        1\n    )"),
            4
        );
        // A header written over more than one line opens its block all the
        // same, a level in from where the header starts.
        assert_eq!(continuation_indent("if f(\n    1\n):"), 4);
        assert_eq!(
            continuation_indent("def f() -> None:\n    if g(\n        1\n    ):"),
            8
        );
        // And the words that end a block are read there too.
        assert_eq!(
            continuation_indent("def f() -> int:\n    return g(\n        1\n    )"),
            0
        );
    }

    #[test]
    fn the_prompt_is_told_to_run_or_how_far_in_the_next_line_starts() {
        // Nothing is open: the block is waiting on a body.
        assert_eq!(ready_state("def f() -> int:"), 4);
        assert_eq!(ready_state("def f() -> int:\n    return 1"), 0);
        assert_eq!(ready_state("@dataclass"), 0);
        // A bracket is worth a level.
        assert_eq!(ready_state("x = ["), 4);
        assert_eq!(ready_state("def f() -> None:\n    print("), 8);
        // What no line could finish is run, and the error is the answer.
        assert_eq!(ready_state("x = "), -1);
        assert_eq!(ready_state("x = )"), -1);
        // A blank line ends what was waiting for a body.
        assert_eq!(ready_state("def f() -> int:\n"), -1);
    }

    /// Every prefix of a valid program is unfinished, by construction. The
    /// REPL only ever asks about whole lines, so those are the prefixes that
    /// have to read as unfinished. One that does not ends the input early
    /// and answers a syntax error the user has not made yet.
    #[test]
    fn every_line_prefix_of_a_valid_program_reads_as_unfinished() {
        let src = r#"from dataclasses import dataclass
from enum import Enum


class Shape(Enum):
    """Um formato.

    Atencao: os nomes sao maiusculos.
    """

    CIRCLE = 1
    SQUARE = 2


@dataclass
class Figure:
    shape: Shape
    size: float


def area(figure: Figure) -> float:
    match figure.shape:
        case Shape.CIRCLE:
            return 3.14 * figure.size * figure.size
        case Shape.SQUARE:
            return figure.size * figure.size


def main() -> None:
    figures: list[Figure] = [
        Figure(Shape.CIRCLE, 1.0),
        Figure(Shape.SQUARE, 2.0),
    ]
    total = 0.0
    for figure in figures:
        total += area(figure)
    print(f"total: {total}")  # soma
"#;
        let lines: Vec<&str> = src.lines().collect();
        let mut rejected = 0;
        for k in 1..lines.len() {
            let prefix = lines.get(..k).unwrap_or(&lines).join("\n");
            if !parse_unchecked_source(&prefix, PySourceType::Python)
                .errors()
                .is_empty()
            {
                rejected += 1;
                assert!(is_incomplete(&prefix), "{prefix:?}");
            }
        }
        assert!(rejected > 10, "{rejected} prefixos e pouco para dizer algo");
    }

    #[test]
    fn one_line_runs_from_anywhere_in_it_and_several_from_the_end() {
        assert_eq!(at_cursor("x = |1"), -1);
        assert_eq!(at_cursor("x = 1  # nota:|"), -1);
        // Several lines with the cursor back in them: the key opens a line,
        // finished or not.
        assert_eq!(at_cursor("x = 1\ny = |2"), 0);
        // The `:` is inside the comment, so it opens no block to go into.
        assert_eq!(at_cursor("x = 1  # nota:|\ny = 2"), 0);
    }

    #[test]
    fn the_line_goes_as_deep_as_the_block_above_and_the_code_below() {
        // Nothing below: the answer is the one the input alone gives.
        assert_eq!(at_cursor("def f() -> int:|"), 4);
        assert_eq!(at_cursor("x = 1\ny = 2|"), -1);
        // The block goes on below, so the line goes into it.
        assert_eq!(at_cursor("def f() -> int:\n    a = 1|\n    b = 2"), 4);
        // What is written below outweighs the dedent `return` asks for: a
        // line out at 0 would leave `b = 2` indented under nothing.
        assert_eq!(at_cursor("def f() -> int:\n    return a|\n    b = 2"), 4);
        assert_eq!(
            at_cursor("def f() -> int:\n    return a|\ndef g() -> int:"),
            0
        );
        // A blank line and a comment say nothing about where the block is.
        assert_eq!(
            at_cursor("def f() -> int:\n    return a|\n\n    # nota\n    b = 2"),
            4
        );
        // The other way round, the block just opened is the floor: 4 is not
        // a place the body of the `if` could go.
        assert_eq!(at_cursor("def f() -> int:\n    if x:|\n    return 1"), 8);
        assert_eq!(at_cursor("d = {\n    'a': 1,|\n}"), 4);
        // And the line the cursor is on is a floor too: Enter here goes on
        // writing the body of `f`, it does not leave it.
        assert_eq!(at_cursor("def f() -> int:\n    x = 1|\ny = 2"), 4);
        // A line indented with a tab cannot be measured in spaces, so it is
        // passed over instead of counted as a line written at the margin.
        assert_eq!(
            at_cursor("def f() -> int:\n    return a|\n\tb = 2\n    c = 3"),
            4
        );
    }

    #[test]
    fn a_cursor_off_a_character_boundary_moves_back_instead_of_panicking() {
        let input = "x = 'a'\ny = 2";
        assert_eq!(super::ready_state(input, 6), 0);
        // The same input with a character the cursor can land inside of.
        let accented = "x = 'ao'\ny = 2".replace('o', "\u{e3}o");
        assert_eq!(super::ready_state(&accented, 6), 0);
        // And past the end, which is where a stale cursor lands.
        assert_eq!(super::ready_state(input, 999), -1);
    }
}
