mod render;

use std::collections::HashSet;
use std::fs;
use std::io::{self, IsTerminal, Write};
use std::path::Path;
use std::time::Duration;

use crossterm::cursor;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use engine::Level;
use engine::trace::{DictEntry, ExecutionTrace, HeapValue, NamedValue, TraceStep, TraceValue};

const DEFAULT_MAX_STEPS: usize = 10_000;

pub fn run_file(file: &Path, level: Level) -> Result<(), String> {
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return Err("the stepper requires an interactive terminal".to_string());
    }

    let abs_file = dunce::canonicalize(file).map_err(|e| {
        format!(
            "could not resolve '{}' to an absolute path: {e}",
            file.display()
        )
    })?;
    if !abs_file.is_file() {
        return Err(format!("'{}' is not a file", file.display()));
    }

    let source = fs::read_to_string(&abs_file)
        .map_err(|e| format!("could not read '{}': {e}", abs_file.display()))?;
    let trace = build_trace_for_stepper(&source, level, std::io::stderr().is_terminal())?;

    let title = abs_file.display().to_string();
    let mut app = StepperApp::new(title, source, trace);
    app.run().map_err(|e| format!("failed to run stepper: {e}"))
}

fn build_trace_for_stepper(
    source: &str,
    level: Level,
    use_color: bool,
) -> Result<ExecutionTrace, String> {
    match engine::type_check_source(source, level) {
        Ok(Some(te)) => {
            engine::print_type_errors(&te.db, &te.diagnostics, use_color);
        }
        Err(te) => {
            engine::print_type_errors(&te.db, &te.diagnostics, use_color);
            return Err("type checking failed".to_string());
        }
        Ok(None) => {}
    }

    let trace_json = engine::trace::trace_source_prechecked(source, DEFAULT_MAX_STEPS);
    serde_json::from_str(&trace_json).map_err(|e| format!("invalid trace: {e}"))
}

pub(crate) struct StepperApp {
    title: String,
    source_lines: Vec<String>,
    trace: ExecutionTrace,
    visible_steps: Vec<usize>,
    current_position: usize,
    output_at: Vec<String>,
}

impl StepperApp {
    fn new(title: String, source: String, trace: ExecutionTrace) -> Self {
        let source_lines = if source.is_empty() {
            vec![String::new()]
        } else {
            source.lines().map(str::to_owned).collect()
        };

        let output_at = build_output_timeline(&trace.steps);
        let visible_steps = build_visible_steps(&source_lines, &trace.steps);
        Self {
            title,
            source_lines,
            trace,
            visible_steps,
            current_position: 0,
            output_at,
        }
    }

    fn max_position(&self) -> usize {
        self.visible_steps.len() + 1
    }

    pub(crate) fn is_initial_position(&self) -> bool {
        self.current_position == 0
    }

    pub(crate) fn is_final_position(&self) -> bool {
        self.current_position == self.max_position()
    }

    fn current_raw_step_index(&self) -> Option<usize> {
        if self.is_initial_position() || self.is_final_position() {
            return None;
        }
        self.visible_steps.get(self.current_position - 1).copied()
    }

    fn current_output_step_index(&self) -> Option<usize> {
        if self.is_initial_position() {
            None
        } else if self.is_final_position() {
            self.trace.steps.len().checked_sub(1)
        } else {
            self.current_raw_step_index()
        }
    }

    pub(crate) fn current_step(&self) -> Option<&TraceStep> {
        self.current_raw_step_index()
            .and_then(|index| self.trace.steps.get(index))
    }

    pub(crate) fn current_output(&self) -> String {
        if let Some(index) = self.current_output_step_index()
            && let Some(output) = self.output_at.get(index)
        {
            return output.clone();
        }
        String::new()
    }

    pub(crate) fn current_display_line(&self) -> Option<usize> {
        self.current_raw_step_index()
            .and_then(|index| self.display_line_for_step(index))
    }

    pub(crate) fn focus_line_for_viewport(&self) -> usize {
        if let Some(line) = self.current_display_line().filter(|line| *line > 0) {
            return line;
        }
        if self.is_final_position() {
            return self
                .visible_steps
                .last()
                .and_then(|&index| self.display_line_for_step(index))
                .filter(|line| *line > 0)
                .unwrap_or(self.source_lines.len().max(1));
        }
        1
    }

    pub(crate) fn step_label(&self) -> String {
        let total = self.visible_steps.len() + 1;
        if self.is_initial_position() {
            return format!("Step 0/{total}  [start]");
        }
        if self.is_final_position() {
            return format!("Step {total}/{total}  [end]");
        }
        match self.current_step() {
            Some(step) => format!(
                "Step {}/{}  line {}  [{}]",
                self.current_position,
                total,
                self.current_display_line().unwrap_or(step.line as usize),
                step.event
            ),
            None => "No captured steps".to_string(),
        }
    }

    fn current_error(&self) -> Option<&engine::trace::TraceErrorInfo> {
        if self.is_final_position() {
            self.trace.error.as_ref()
        } else {
            None
        }
    }

    pub(crate) fn exit_label(&self) -> &'static str {
        match self.trace.exit_kind {
            engine::trace::ExitKind::Normal => "normal",
            engine::trace::ExitKind::Error => "error",
            engine::trace::ExitKind::StepLimit => "step limit",
        }
    }

    pub(crate) fn status_line(&self) -> String {
        let mut status = format!("{}  exit: {}", self.step_label(), self.exit_label());
        if self.trace.truncated {
            status.push_str("  trace truncated");
        }
        if let Some(error) = self.current_error() {
            status.push_str(&format!("  {}: {}", error.exception_type, error.message));
        }
        status.push_str("  ←/→ move  Home/End  q quit");
        status
    }

    pub(crate) fn line_number_width(&self) -> usize {
        self.source_lines.len().max(1).to_string().len()
    }

    pub(crate) fn format_value(&self, step: &TraceStep, value: &TraceValue) -> String {
        format_value_for_display(step, value)
    }

    fn move_by(&mut self, delta: isize) {
        let next = self.current_position.saturating_add_signed(delta);
        self.current_position = next.min(self.max_position());
    }

    fn move_to_start(&mut self) {
        self.current_position = 0;
    }

    fn move_to_end(&mut self) {
        self.current_position = self.max_position();
    }

    fn handle_key(&mut self, code: KeyCode) -> bool {
        match code {
            KeyCode::Right | KeyCode::Char('n') | KeyCode::Char('l') => self.move_by(1),
            KeyCode::Left | KeyCode::Char('p') | KeyCode::Char('h') => self.move_by(-1),
            KeyCode::Home => self.move_to_start(),
            KeyCode::End => self.move_to_end(),
            KeyCode::Esc | KeyCode::Char('q') => return true,
            _ => {}
        }
        false
    }

    fn is_function_definition_line(&self, line_no: usize) -> bool {
        let Some(line) = self.source_lines.get(line_no.saturating_sub(1)) else {
            return false;
        };
        let trimmed = line.trim_start();
        trimmed.starts_with("def ") || trimmed.starts_with("async def ")
    }

    fn display_line_for_step(&self, step_index: usize) -> Option<usize> {
        let step = self.trace.steps.get(step_index)?;
        let raw_line = step.line as usize;
        if step.event != "call" || raw_line == 0 {
            return Some(raw_line);
        }

        let top_frame = step.stack.last()?;
        if top_frame.name == "<module>" || !self.is_function_definition_line(raw_line) {
            return Some(raw_line);
        }

        Some(self.find_function_body_line(raw_line).unwrap_or(raw_line))
    }

    fn find_function_body_line(&self, def_line: usize) -> Option<usize> {
        let def_index = def_line.checked_sub(1)?;
        let def_line_text = self.source_lines.get(def_index)?;
        let def_indent = leading_indent_width(def_line_text);

        let mut header_end = def_line;
        while let Some(line) = self.source_lines.get(header_end.saturating_sub(1)) {
            if line.trim_end().ends_with(':') {
                break;
            }
            header_end += 1;
        }

        for line_no in (header_end + 1)..=self.source_lines.len() {
            let line = &self.source_lines[line_no - 1];
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            let indent = leading_indent_width(line);
            if indent > def_indent {
                return Some(line_no);
            }
            if indent <= def_indent {
                break;
            }
        }

        None
    }

    fn run(&mut self) -> io::Result<()> {
        let mut terminal = TerminalGuard::enter()?;
        let mut needs_redraw = true;
        loop {
            if needs_redraw {
                render::draw(terminal.stdout(), self)?;
                needs_redraw = false;
            }

            if !event::poll(Duration::from_millis(250))? {
                continue;
            }

            match event::read()? {
                Event::Key(key)
                    if key.kind == KeyEventKind::Press || key.kind == KeyEventKind::Repeat =>
                {
                    if self.handle_key(key.code) {
                        break;
                    }
                    needs_redraw = true;
                }
                Event::Resize(_, _) => {
                    needs_redraw = true;
                }
                _ => {}
            }
        }
        Ok(())
    }
}

fn build_output_timeline(steps: &[TraceStep]) -> Vec<String> {
    let mut current = String::new();
    let mut output = Vec::with_capacity(steps.len());
    for step in steps {
        current.push_str(&step.stdout_delta);
        if !step.stderr_delta.is_empty() {
            append_stderr(&mut current, &step.stderr_delta);
        }
        output.push(current.clone());
    }
    output
}

fn append_stderr(buf: &mut String, stderr: &str) {
    for line in stderr.lines() {
        if !buf.is_empty() && !buf.ends_with('\n') {
            buf.push('\n');
        }
        buf.push_str("stderr> ");
        buf.push_str(line);
        buf.push('\n');
    }
    if stderr.ends_with('\n') && !buf.ends_with('\n') {
        buf.push('\n');
    }
}

struct TerminalGuard {
    stdout: io::Stdout,
}

impl TerminalGuard {
    fn enter() -> io::Result<Self> {
        let mut stdout = io::stdout();
        terminal::enable_raw_mode()?;
        if let Err(err) = execute!(stdout, EnterAlternateScreen, cursor::Hide) {
            let _ = terminal::disable_raw_mode();
            return Err(err);
        }
        Ok(Self { stdout })
    }

    fn stdout(&mut self) -> &mut io::Stdout {
        &mut self.stdout
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = execute!(self.stdout, cursor::Show, LeaveAlternateScreen);
        let _ = terminal::disable_raw_mode();
        let _ = self.stdout.flush();
    }
}

fn leading_indent_width(line: &str) -> usize {
    line.chars()
        .take_while(|c| matches!(c, ' ' | '\t'))
        .map(|c| if c == '\t' { 4 } else { 1 })
        .sum()
}

fn build_visible_steps(source_lines: &[String], steps: &[TraceStep]) -> Vec<usize> {
    let mut visible = Vec::new();

    for index in 0..steps.len() {
        if should_skip_step(source_lines, steps, index) {
            continue;
        }
        visible.push(index);
    }

    visible
}

fn should_skip_step(source_lines: &[String], steps: &[TraceStep], index: usize) -> bool {
    let Some(step) = steps.get(index) else {
        return false;
    };
    if !step.stdout_delta.is_empty() || !step.stderr_delta.is_empty() {
        return false;
    }
    if is_initial_module_call_step(step, index) {
        return true;
    }
    let Some(next) = steps.get(index + 1) else {
        return false;
    };

    if is_redundant_call_step(source_lines, steps, index, step, next) {
        return true;
    }

    false
}

fn is_initial_module_call_step(step: &TraceStep, index: usize) -> bool {
    index == 0 && step.event == "call" && is_module_only_step(step)
}

fn is_redundant_call_step(
    source_lines: &[String],
    steps: &[TraceStep],
    index: usize,
    step: &TraceStep,
    next: &TraceStep,
) -> bool {
    if step.event != "call" || next.event != "line" {
        return false;
    }

    if step.stack.len() != next.stack.len() {
        return false;
    }

    if step.stack.last().map(|f| f.name.as_str()) != next.stack.last().map(|f| f.name.as_str()) {
        return false;
    }

    display_line_for_step(source_lines, steps, index)
        == display_line_for_step(source_lines, steps, index + 1)
}

fn display_line_for_step(
    source_lines: &[String],
    steps: &[TraceStep],
    index: usize,
) -> Option<usize> {
    let step = steps.get(index)?;
    let raw_line = step.line as usize;
    if step.event != "call" || raw_line == 0 {
        return Some(raw_line);
    }

    let top_frame = step.stack.last()?;
    if top_frame.name == "<module>" || !is_function_definition_line(source_lines, raw_line) {
        return Some(raw_line);
    }

    Some(find_function_body_line(source_lines, raw_line).unwrap_or(raw_line))
}

fn is_module_only_step(step: &TraceStep) -> bool {
    step.stack.len() == 1
        && step
            .stack
            .last()
            .is_some_and(|frame| frame.name == "<module>")
}

fn is_function_definition_line(source_lines: &[String], line_no: usize) -> bool {
    let Some(line) = source_lines.get(line_no.saturating_sub(1)) else {
        return false;
    };
    let trimmed = line.trim_start();
    trimmed.starts_with("def ") || trimmed.starts_with("async def ")
}

fn find_function_body_line(source_lines: &[String], def_line: usize) -> Option<usize> {
    let def_index = def_line.checked_sub(1)?;
    let def_line_text = source_lines.get(def_index)?;
    let def_indent = leading_indent_width(def_line_text);

    let mut header_end = def_line;
    while let Some(line) = source_lines.get(header_end.saturating_sub(1)) {
        if line.trim_end().ends_with(':') {
            break;
        }
        header_end += 1;
    }

    for line_no in (header_end + 1)..=source_lines.len() {
        let line = &source_lines[line_no - 1];
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        let indent = leading_indent_width(line);
        if indent > def_indent {
            return Some(line_no);
        }
        if indent <= def_indent {
            break;
        }
    }

    None
}

fn format_value_for_display(step: &TraceStep, value: &TraceValue) -> String {
    let mut active_refs = HashSet::new();
    format_value_with_refs(step, value, &mut active_refs)
}

fn format_value_with_refs(
    step: &TraceStep,
    value: &TraceValue,
    active_refs: &mut HashSet<u64>,
) -> String {
    match value {
        TraceValue::Ref { object_id } => format_heap_ref(step, *object_id, active_refs),
    }
}

fn format_fields(
    step: &TraceStep,
    fields: &[NamedValue],
    active_refs: &mut HashSet<u64>,
) -> String {
    fields
        .iter()
        .map(|field| {
            format!(
                "{}={}",
                field.name,
                format_value_with_refs(step, &field.value, active_refs)
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn format_heap_ref(step: &TraceStep, object_id: u64, active_refs: &mut HashSet<u64>) -> String {
    let Some(heap_object) = step.heap.iter().find(|obj| obj.object_id == object_id) else {
        return format!("<missing ref #{object_id}>");
    };
    match &heap_object.value {
        HeapValue::None => "None".to_string(),
        HeapValue::Bool { value } => {
            if *value {
                "True".to_string()
            } else {
                "False".to_string()
            }
        }
        HeapValue::Int { value } | HeapValue::Float { value } | HeapValue::Bytes { value } => {
            value.clone()
        }
        HeapValue::Str { value } => format!("{value:?}"),
        HeapValue::Function { name } => format!("<function {name}> #{object_id}"),
        HeapValue::Class { name } => format!("<class {name}> #{object_id}"),
        HeapValue::Opaque { class_name } => format!("<{class_name}> #{object_id}"),
        HeapValue::List { items } => {
            if !active_refs.insert(object_id) {
                return format!("<ref #{object_id}>");
            }
            let body = format!(
                "[{}]",
                items
                    .iter()
                    .map(|item| format_value_with_refs(step, item, active_refs))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            active_refs.remove(&object_id);
            format!("{body} #{object_id}")
        }
        HeapValue::Dict { entries } => {
            if !active_refs.insert(object_id) {
                return format!("<ref #{object_id}>");
            }
            let body = format!(
                "{{{}}}",
                entries
                    .iter()
                    .map(|entry| format_dict_entry(step, entry, active_refs))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            active_refs.remove(&object_id);
            format!("{body} #{object_id}")
        }
        HeapValue::Dataclass { class_name, fields } => {
            if !active_refs.insert(object_id) {
                return format!("<ref #{object_id}>");
            }
            let body = format!("{class_name}({})", format_fields(step, fields, active_refs));
            active_refs.remove(&object_id);
            format!("{body} #{object_id}")
        }
        HeapValue::Object { class_name, fields } => {
            if !active_refs.insert(object_id) {
                return format!("<ref #{object_id}>");
            }
            let body = format!("{class_name}({})", format_fields(step, fields, active_refs));
            active_refs.remove(&object_id);
            format!("{body} #{object_id}")
        }
    }
}

fn format_dict_entry(
    step: &TraceStep,
    entry: &DictEntry,
    active_refs: &mut HashSet<u64>,
) -> String {
    format!(
        "{}: {}",
        format_value_with_refs(step, &entry.key, active_refs),
        format_value_with_refs(step, &entry.value, active_refs)
    )
}

#[cfg(test)]
mod tests {
    use engine::trace::{ExitKind, HeapObject, StackFrameView, TraceErrorInfo};

    use super::*;

    fn named(name: &str, object_id: u64) -> NamedValue {
        NamedValue {
            name: name.to_string(),
            value: TraceValue::Ref { object_id },
        }
    }

    fn heap_object(object_id: u64, value: HeapValue) -> HeapObject {
        HeapObject { object_id, value }
    }

    fn step(
        line: u32,
        event: &str,
        stack: Vec<StackFrameView>,
        heap: Vec<HeapObject>,
    ) -> TraceStep {
        TraceStep {
            line,
            event: event.to_string(),
            stdout_delta: String::new(),
            stderr_delta: String::new(),
            stack,
            heap,
        }
    }

    #[test]
    fn accumulates_output_and_stderr() {
        let steps = vec![
            TraceStep {
                line: 1,
                event: "line".to_string(),
                stdout_delta: "1\n".to_string(),
                stderr_delta: String::new(),
                stack: Vec::new(),
                heap: Vec::new(),
            },
            TraceStep {
                line: 2,
                event: "line".to_string(),
                stdout_delta: String::new(),
                stderr_delta: "boom\n".to_string(),
                stack: Vec::new(),
                heap: Vec::new(),
            },
        ];

        let timeline = build_output_timeline(&steps);
        assert_eq!(timeline[0], "1\n");
        assert_eq!(timeline[1], "1\nstderr> boom\n");
    }

    #[test]
    fn status_line_mentions_error_only_at_end() {
        let trace = ExecutionTrace {
            steps: vec![step(
                3,
                "line",
                vec![StackFrameView {
                    name: "<module>".to_string(),
                    line: 3,
                    locals: vec![named("x", 1)],
                }],
                vec![heap_object(
                    1,
                    HeapValue::Int {
                        value: "1".to_string(),
                    },
                )],
            )],
            truncated: false,
            exit_kind: ExitKind::Error,
            error: Some(TraceErrorInfo {
                exception_type: "ZeroDivisionError".to_string(),
                message: "division by zero".to_string(),
                line: None,
            }),
        };

        let mut app = StepperApp::new("test.py".to_string(), "x = 1\n".to_string(), trace);
        app.move_by(1);
        assert!(!app.status_line().contains("ZeroDivisionError"));
        app.move_to_end();
        assert!(app.status_line().contains("ZeroDivisionError"));
    }

    #[test]
    fn call_step_enters_function_body_for_display() {
        let trace = ExecutionTrace {
            steps: vec![step(
                1,
                "call",
                vec![
                    StackFrameView {
                        name: "<module>".to_string(),
                        line: 3,
                        locals: Vec::new(),
                    },
                    StackFrameView {
                        name: "add".to_string(),
                        line: 1,
                        locals: Vec::new(),
                    },
                ],
                Vec::new(),
            )],
            truncated: false,
            exit_kind: ExitKind::Normal,
            error: None,
        };

        let source = "def add(a: int, b: int) -> int:\n    return a + b\nx: int = add(1, 2)\n";
        let mut app = StepperApp::new("test.py".to_string(), source.to_string(), trace);
        app.move_by(1);
        assert_eq!(app.current_display_line(), Some(2));
    }

    #[test]
    fn redundant_call_step_is_hidden_from_navigation() {
        let steps = vec![
            step(
                1,
                "call",
                vec![
                    StackFrameView {
                        name: "<module>".to_string(),
                        line: 3,
                        locals: Vec::new(),
                    },
                    StackFrameView {
                        name: "add".to_string(),
                        line: 1,
                        locals: vec![named("a", 1)],
                    },
                ],
                vec![heap_object(
                    1,
                    HeapValue::Int {
                        value: "1".to_string(),
                    },
                )],
            ),
            step(
                2,
                "line",
                vec![
                    StackFrameView {
                        name: "<module>".to_string(),
                        line: 3,
                        locals: Vec::new(),
                    },
                    StackFrameView {
                        name: "add".to_string(),
                        line: 2,
                        locals: vec![named("a", 1)],
                    },
                ],
                vec![heap_object(
                    1,
                    HeapValue::Int {
                        value: "1".to_string(),
                    },
                )],
            ),
        ];

        let source = vec![
            "def add(a: int, b: int) -> int:".to_string(),
            "    return a + b".to_string(),
            "x: int = add(1, 2)".to_string(),
        ];

        let visible = build_visible_steps(&source, &steps);
        assert_eq!(visible, vec![1]);
    }

    #[test]
    fn module_definition_steps_remain_visible() {
        let steps = vec![
            step(
                1,
                "call",
                vec![StackFrameView {
                    name: "<module>".to_string(),
                    line: 1,
                    locals: Vec::new(),
                }],
                Vec::new(),
            ),
            step(
                1,
                "line",
                vec![StackFrameView {
                    name: "<module>".to_string(),
                    line: 1,
                    locals: Vec::new(),
                }],
                Vec::new(),
            ),
            step(
                3,
                "line",
                vec![StackFrameView {
                    name: "<module>".to_string(),
                    line: 3,
                    locals: vec![named("add", 1)],
                }],
                vec![heap_object(
                    1,
                    HeapValue::Function {
                        name: "add".to_string(),
                    },
                )],
            ),
        ];

        let source = vec![
            "def add(a: int, b: int) -> int:".to_string(),
            "    return a + b".to_string(),
            "x: int = add(1, 2)".to_string(),
        ];

        let visible = build_visible_steps(&source, &steps);
        assert_eq!(visible, vec![1, 2]);
    }

    #[test]
    fn sentinel_positions_have_no_selected_line() {
        let trace = ExecutionTrace {
            steps: vec![TraceStep {
                line: 1,
                event: "line".to_string(),
                stdout_delta: "42\n".to_string(),
                stderr_delta: String::new(),
                stack: vec![StackFrameView {
                    name: "<module>".to_string(),
                    line: 1,
                    locals: vec![named("x", 1)],
                }],
                heap: vec![heap_object(
                    1,
                    HeapValue::Int {
                        value: "42".to_string(),
                    },
                )],
            }],
            truncated: false,
            exit_kind: ExitKind::Normal,
            error: None,
        };

        let mut app = StepperApp::new("test.py".to_string(), "x = 42\n".to_string(), trace);
        assert_eq!(app.current_display_line(), None);
        assert_eq!(app.step_label(), "Step 0/2  [start]");
        assert_eq!(app.current_output(), "");

        app.move_to_end();
        assert_eq!(app.current_display_line(), None);
        assert_eq!(app.step_label(), "Step 2/2  [end]");
        assert_eq!(app.current_output(), "42\n");
    }

    #[test]
    fn formats_heap_references_with_identity() {
        let step = TraceStep {
            line: 1,
            event: "line".to_string(),
            stdout_delta: String::new(),
            stderr_delta: String::new(),
            stack: Vec::new(),
            heap: vec![
                HeapObject {
                    object_id: 1,
                    value: HeapValue::Int {
                        value: "1".to_string(),
                    },
                },
                HeapObject {
                    object_id: 7,
                    value: HeapValue::List {
                        items: vec![TraceValue::Ref { object_id: 1 }],
                    },
                },
            ],
        };

        assert_eq!(
            format_value_for_display(&step, &TraceValue::Ref { object_id: 7 }),
            "[1] #7"
        );
    }

    #[test]
    fn formats_immutable_heap_values_inline() {
        let step = TraceStep {
            line: 1,
            event: "line".to_string(),
            stdout_delta: String::new(),
            stderr_delta: String::new(),
            stack: Vec::new(),
            heap: vec![heap_object(
                1,
                HeapValue::Int {
                    value: "42".to_string(),
                },
            )],
        };

        assert_eq!(
            format_value_for_display(&step, &TraceValue::Ref { object_id: 1 }),
            "42"
        );
    }

    #[test]
    fn type_errors_block_stepper_trace_build() {
        let result = build_trace_for_stepper("x: int = 'hello'\n", Level::Full, false);
        assert_eq!(result.unwrap_err(), "type checking failed");
    }
}
