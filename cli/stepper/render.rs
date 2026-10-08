use std::cmp::{max, min};
use std::io::{self, Write};

use crossterm::cursor::MoveTo;
use crossterm::queue;
use crossterm::style::{
    Attribute, Color, Print, ResetColor, SetAttribute, SetBackgroundColor, SetForegroundColor,
};
use crossterm::terminal::{self, Clear, ClearType};
use engine::completion::{PYTHON_BOOLEANS, PYTHON_CONSTANTS, PYTHON_KEYWORDS};

use super::StepperApp;

#[derive(Clone, Copy)]
struct Rect {
    x: u16,
    y: u16,
    width: u16,
    height: u16,
}

struct RenderLine {
    text: String,
    highlighted: bool,
}

struct CodeRenderLine {
    prefix: String,
    code: String,
    highlighted: bool,
}

struct StyledSegment<'a> {
    text: &'a str,
    style: TokenStyle,
}

#[derive(Clone, Copy)]
enum TokenStyle {
    Plain,
    Comment,
    String,
    Number,
    Keyword,
    Builtin,
    Boolean,
    Constant,
    Decorator,
}

struct CodePalette {
    text: Color,
    line_number: Color,
    active_text: Color,
    active_line_number: Color,
    comment: Color,
    string: Color,
    number: Color,
    keyword: Color,
    builtin: Color,
    boolean: Color,
    constant: Color,
    decorator: Color,
    active_bg: Color,
}

const DARK_PALETTE: CodePalette = CodePalette {
    text: Color::Rgb {
        r: 171,
        g: 178,
        b: 191,
    },
    line_number: Color::Rgb {
        r: 93,
        g: 99,
        b: 111,
    },
    active_text: Color::Rgb {
        r: 236,
        g: 240,
        b: 241,
    },
    active_line_number: Color::Rgb {
        r: 97,
        g: 175,
        b: 239,
    },
    comment: Color::Rgb {
        r: 93,
        g: 99,
        b: 111,
    },
    string: Color::Rgb {
        r: 161,
        g: 193,
        b: 129,
    },
    number: Color::Rgb {
        r: 191,
        g: 149,
        b: 106,
    },
    keyword: Color::Rgb {
        r: 180,
        g: 119,
        b: 207,
    },
    builtin: Color::Rgb {
        r: 115,
        g: 173,
        b: 233,
    },
    boolean: Color::Rgb {
        r: 191,
        g: 149,
        b: 106,
    },
    constant: Color::Rgb {
        r: 223,
        g: 193,
        b: 132,
    },
    decorator: Color::Rgb {
        r: 116,
        g: 174,
        b: 232,
    },
    active_bg: Color::Rgb {
        r: 40,
        g: 44,
        b: 52,
    },
};

const LIGHT_PALETTE: CodePalette = CodePalette {
    text: Color::Rgb {
        r: 56,
        g: 58,
        b: 66,
    },
    line_number: Color::Rgb {
        r: 162,
        g: 163,
        b: 167,
    },
    active_text: Color::Rgb {
        r: 40,
        g: 44,
        b: 52,
    },
    active_line_number: Color::Rgb {
        r: 92,
        g: 120,
        b: 226,
    },
    comment: Color::Rgb {
        r: 162,
        g: 163,
        b: 167,
    },
    string: Color::Rgb {
        r: 100,
        g: 159,
        b: 87,
    },
    number: Color::Rgb {
        r: 173,
        g: 110,
        b: 37,
    },
    keyword: Color::Rgb {
        r: 164,
        g: 73,
        b: 171,
    },
    builtin: Color::Rgb {
        r: 91,
        g: 121,
        b: 227,
    },
    boolean: Color::Rgb {
        r: 173,
        g: 110,
        b: 37,
    },
    constant: Color::Rgb {
        r: 193,
        g: 132,
        b: 1,
    },
    decorator: Color::Rgb {
        r: 92,
        g: 120,
        b: 226,
    },
    active_bg: Color::Rgb {
        r: 235,
        g: 237,
        b: 243,
    },
};

pub(super) fn draw(stdout: &mut impl Write, app: &StepperApp) -> io::Result<()> {
    let (width, height) = terminal::size()?;
    queue!(stdout, MoveTo(0, 0), Clear(ClearType::All))?;

    if width < 60 || height < 12 {
        queue!(
            stdout,
            MoveTo(0, 0),
            Print("Terminal too small for the stepper."),
            MoveTo(0, 1),
            Print("Resize the window and try again."),
        )?;
        stdout.flush()?;
        return Ok(());
    }

    let status_height = 1;
    let output_height = min(max(height / 3, 6), height.saturating_sub(5));
    let top_height = height.saturating_sub(output_height + status_height);

    let code_width = min(max(width * 45 / 100, 28), width.saturating_sub(24));
    let stack_width = width.saturating_sub(code_width);

    let code_rect = Rect {
        x: 0,
        y: 0,
        width: code_width,
        height: top_height,
    };
    let stack_rect = Rect {
        x: code_width,
        y: 0,
        width: stack_width,
        height: top_height,
    };
    let output_rect = Rect {
        x: 0,
        y: top_height,
        width,
        height: output_height,
    };

    let code_lines = build_code_lines(app, code_rect);
    let stack_lines = build_stack_lines(app, stack_rect);
    let output_lines = build_output_lines(app, output_rect);

    draw_code_box(
        stdout,
        code_rect,
        &format!("Code  {}", app.title),
        &code_lines,
    )?;
    draw_box(stdout, stack_rect, "Stack and Variables", &stack_lines)?;
    draw_box(stdout, output_rect, "Output", &output_lines)?;

    queue!(
        stdout,
        MoveTo(0, height.saturating_sub(1)),
        Print(fit_to_width(&app.status_line(), width as usize)),
    )?;
    stdout.flush()
}

fn build_code_lines(app: &StepperApp, rect: Rect) -> Vec<CodeRenderLine> {
    let visible = rect.height.saturating_sub(2) as usize;
    if visible == 0 {
        return Vec::new();
    }

    let current_line = app.current_display_line().filter(|line| *line > 0);
    let focus = current_line.unwrap_or_else(|| app.focus_line_for_viewport());
    let total = app.source_lines.len();
    let start = focus
        .saturating_sub(visible / 2 + 1)
        .min(total.saturating_sub(visible));
    let end = min(total, start + visible);

    (start..end)
        .map(|index| {
            let line_no = index + 1;
            let marker = if Some(line_no) == current_line {
                ">"
            } else {
                " "
            };
            CodeRenderLine {
                prefix: format!(
                    "{marker}{line_no:>width$} ",
                    width = app.line_number_width()
                ),
                code: app.source_lines[index].clone(),
                highlighted: Some(line_no) == current_line,
            }
        })
        .collect()
}

fn build_stack_lines(app: &StepperApp, rect: Rect) -> Vec<RenderLine> {
    let visible = rect.height.saturating_sub(2) as usize;
    if visible == 0 {
        return Vec::new();
    }

    let mut lines = Vec::new();
    if let Some(step) = app.current_step() {
        if step.stack.is_empty() {
            lines.push(RenderLine {
                text: "No user frames at this step.".to_string(),
                highlighted: false,
            });
        } else {
            for (index, frame) in step.stack.iter().rev().enumerate() {
                if index > 0 {
                    lines.push(RenderLine {
                        text: String::new(),
                        highlighted: false,
                    });
                }
                lines.push(RenderLine {
                    text: format!(
                        "{} @ line {}",
                        frame.name,
                        display_frame_line(app, step, frame.line as usize, index == 0)
                    ),
                    highlighted: index == 0,
                });
                if frame.locals.is_empty() {
                    lines.push(RenderLine {
                        text: "  (no local variables)".to_string(),
                        highlighted: false,
                    });
                } else {
                    for binding in &frame.locals {
                        lines.push(RenderLine {
                            text: format!(
                                "  {} = {}",
                                binding.name,
                                app.format_value(step, &binding.value)
                            ),
                            highlighted: false,
                        });
                    }
                }
            }
        }
    } else if app.is_initial_position() || app.is_final_position() {
    } else if let Some(error) = &app.trace.error {
        lines.push(RenderLine {
            text: format!("Error: {}: {}", error.exception_type, error.message),
            highlighted: true,
        });
    } else {
        lines.push(RenderLine {
            text: "No captured steps.".to_string(),
            highlighted: false,
        });
    }

    if lines.len() > visible {
        lines.truncate(visible);
    }
    lines
}

fn build_output_lines(app: &StepperApp, rect: Rect) -> Vec<RenderLine> {
    let visible = rect.height.saturating_sub(2) as usize;
    if visible == 0 {
        return Vec::new();
    }

    let output = app.current_output();
    if output.is_empty() {
        return vec![RenderLine {
            text: "(no output yet)".to_string(),
            highlighted: false,
        }];
    }

    let all_lines: Vec<_> = output.lines().map(str::to_owned).collect();
    let start = all_lines.len().saturating_sub(visible);
    all_lines[start..]
        .iter()
        .map(|line| RenderLine {
            text: line.clone(),
            highlighted: false,
        })
        .collect()
}

fn draw_box(
    stdout: &mut impl Write,
    rect: Rect,
    title: &str,
    lines: &[RenderLine],
) -> io::Result<()> {
    if rect.width < 2 || rect.height < 2 {
        return Ok(());
    }

    let inner_width = rect.width.saturating_sub(2) as usize;
    let content_height = rect.height.saturating_sub(2) as usize;

    queue!(
        stdout,
        MoveTo(rect.x, rect.y),
        Print(top_border(title, rect.width as usize)),
    )?;

    for offset in 0..content_height {
        let y = rect.y + offset as u16 + 1;
        queue!(stdout, MoveTo(rect.x, y), Print("│"))?;
        let line = lines
            .get(offset)
            .map(|line| line.text.as_str())
            .unwrap_or("");
        let highlighted = lines.get(offset).is_some_and(|line| line.highlighted);
        if highlighted {
            queue!(stdout, SetAttribute(Attribute::Reverse))?;
        }
        queue!(stdout, Print(fit_to_width(line, inner_width)))?;
        if highlighted {
            queue!(stdout, SetAttribute(Attribute::Reset))?;
        }
        queue!(stdout, Print("│"))?;
    }

    queue!(
        stdout,
        MoveTo(rect.x, rect.y + rect.height.saturating_sub(1)),
        Print(bottom_border(rect.width as usize)),
    )?;
    Ok(())
}

fn draw_code_box(
    stdout: &mut impl Write,
    rect: Rect,
    title: &str,
    lines: &[CodeRenderLine],
) -> io::Result<()> {
    if rect.width < 2 || rect.height < 2 {
        return Ok(());
    }

    let palette = palette();
    let inner_width = rect.width.saturating_sub(2) as usize;
    let content_height = rect.height.saturating_sub(2) as usize;

    queue!(
        stdout,
        MoveTo(rect.x, rect.y),
        Print(top_border(title, rect.width as usize)),
    )?;

    for offset in 0..content_height {
        let y = rect.y + offset as u16 + 1;
        queue!(stdout, MoveTo(rect.x, y), Print("│"))?;

        if let Some(line) = lines.get(offset) {
            draw_code_line(stdout, line, inner_width, palette)?;
        } else {
            queue!(stdout, Print(" ".repeat(inner_width)))?;
        }

        queue!(
            stdout,
            ResetColor,
            SetAttribute(Attribute::Reset),
            Print("│")
        )?;
    }

    queue!(
        stdout,
        MoveTo(rect.x, rect.y + rect.height.saturating_sub(1)),
        Print(bottom_border(rect.width as usize)),
    )?;
    Ok(())
}

fn draw_code_line(
    stdout: &mut impl Write,
    line: &CodeRenderLine,
    width: usize,
    palette: &CodePalette,
) -> io::Result<()> {
    if line.highlighted {
        queue!(stdout, SetBackgroundColor(palette.active_bg))?;
        queue!(stdout, SetAttribute(Attribute::Bold))?;
    }

    let prefix = fit_plain(&line.prefix, width);
    let prefix_width = prefix.chars().count();
    let code_width = width.saturating_sub(prefix_width);
    let clipped_code = fit_plain(&line.code, code_width);

    queue!(
        stdout,
        SetForegroundColor(if line.highlighted {
            palette.active_line_number
        } else {
            palette.line_number
        }),
        Print(&prefix),
    )?;

    let mut used = prefix_width;
    for segment in styled_segments(&clipped_code) {
        queue!(
            stdout,
            SetForegroundColor(if line.highlighted {
                highlighted_token_color(palette, segment.style)
            } else {
                token_color(palette, segment.style)
            })
        )?;
        queue!(stdout, Print(segment.text))?;
        used += segment.text.chars().count();
    }

    if used < width {
        queue!(
            stdout,
            SetForegroundColor(if line.highlighted {
                palette.active_text
            } else {
                palette.text
            }),
            Print(" ".repeat(width - used)),
        )?;
    }

    queue!(stdout, ResetColor)?;
    Ok(())
}

fn display_frame_line(
    app: &StepperApp,
    step: &engine::trace::TraceStep,
    line: usize,
    is_top: bool,
) -> usize {
    if is_top && step.event == "call" && line == step.line as usize {
        app.current_display_line().unwrap_or(line)
    } else {
        line
    }
}

fn top_border(title: &str, width: usize) -> String {
    if width <= 2 {
        return "┌┐".to_string();
    }

    let max_title = width.saturating_sub(4);
    let title = fit_plain(title, max_title);
    let mut out = String::from("┌");
    if !title.is_empty() {
        out.push('─');
        out.push(' ');
        out.push_str(&title);
        out.push(' ');
    }
    let remaining = width.saturating_sub(out.chars().count() + 1);
    out.push_str(&"─".repeat(remaining));
    out.push('┐');
    out
}

fn bottom_border(width: usize) -> String {
    if width <= 2 {
        return "└┘".to_string();
    }
    format!("└{}┘", "─".repeat(width - 2))
}

fn fit_to_width(text: &str, width: usize) -> String {
    let plain = fit_plain(text, width);
    let used = plain.chars().count();
    if used >= width {
        plain
    } else {
        format!("{plain}{}", " ".repeat(width - used))
    }
}

fn fit_plain(text: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }

    let text = text.replace('\t', "    ");
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= width {
        return text;
    }
    if width <= 3 {
        return ".".repeat(width);
    }
    let prefix: String = chars[..width - 3].iter().collect();
    format!("{prefix}...")
}

fn palette() -> &'static CodePalette {
    if crate::repl::is_light_theme() {
        &LIGHT_PALETTE
    } else {
        &DARK_PALETTE
    }
}

fn styled_segments(line: &str) -> Vec<StyledSegment<'_>> {
    let bytes = line.as_bytes();
    let len = bytes.len();
    let mut segments = Vec::new();
    let mut i = 0;

    while i < len {
        match bytes[i] {
            b'#' => {
                segments.push(StyledSegment {
                    text: &line[i..],
                    style: TokenStyle::Comment,
                });
                return segments;
            }
            b'\'' | b'"' => {
                let start = i;
                i = skip_string(bytes, i);
                segments.push(StyledSegment {
                    text: &line[start..i],
                    style: TokenStyle::String,
                });
            }
            b'0'..=b'9' => {
                let start = i;
                i = skip_number(bytes, i);
                segments.push(StyledSegment {
                    text: &line[start..i],
                    style: TokenStyle::Number,
                });
            }
            b'.' if i + 1 < len && bytes[i + 1].is_ascii_digit() => {
                let start = i;
                i = skip_number(bytes, i);
                segments.push(StyledSegment {
                    text: &line[start..i],
                    style: TokenStyle::Number,
                });
            }
            b'a'..=b'z' | b'A'..=b'Z' | b'_' => {
                let start = i;
                while i < len && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                    i += 1;
                }
                let word = &line[start..i];
                let style =
                    if i < len && (bytes[i] == b'\'' || bytes[i] == b'"') && is_string_prefix(word)
                    {
                        let str_end = skip_string(bytes, i);
                        let text = &line[start..str_end];
                        i = str_end;
                        segments.push(StyledSegment {
                            text,
                            style: TokenStyle::String,
                        });
                        continue;
                    } else if PYTHON_BOOLEANS.contains(&word) {
                        TokenStyle::Boolean
                    } else if PYTHON_CONSTANTS.contains(&word) {
                        TokenStyle::Constant
                    } else if PYTHON_KEYWORDS.contains(&word) {
                        TokenStyle::Keyword
                    } else if is_python_builtin(word) {
                        TokenStyle::Builtin
                    } else {
                        TokenStyle::Plain
                    };
                segments.push(StyledSegment { text: word, style });
            }
            b'@' => {
                let start = i;
                i += 1;
                while i < len
                    && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_' || bytes[i] == b'.')
                {
                    i += 1;
                }
                segments.push(StyledSegment {
                    text: &line[start..i],
                    style: TokenStyle::Decorator,
                });
            }
            _ => {
                let start = i;
                let ch = line[i..].chars().next().unwrap();
                i += ch.len_utf8();
                segments.push(StyledSegment {
                    text: &line[start..i],
                    style: TokenStyle::Plain,
                });
            }
        }
    }

    segments
}

fn token_color(palette: &CodePalette, style: TokenStyle) -> Color {
    match style {
        TokenStyle::Plain => palette.text,
        TokenStyle::Comment => palette.comment,
        TokenStyle::String => palette.string,
        TokenStyle::Number => palette.number,
        TokenStyle::Keyword => palette.keyword,
        TokenStyle::Builtin => palette.builtin,
        TokenStyle::Boolean => palette.boolean,
        TokenStyle::Constant => palette.constant,
        TokenStyle::Decorator => palette.decorator,
    }
}

fn highlighted_token_color(palette: &CodePalette, style: TokenStyle) -> Color {
    match style {
        TokenStyle::Plain => palette.active_text,
        TokenStyle::Comment => palette.line_number,
        _ => token_color(palette, style),
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_plain_truncates_long_text() {
        assert_eq!(fit_plain("abcdef", 5), "ab...");
    }

    #[test]
    fn top_border_fills_requested_width() {
        let border = top_border("Titulo", 20);
        assert_eq!(border.chars().count(), 20);
    }
}
