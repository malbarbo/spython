use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use ruff_python_ast::Stmt;
use ruff_python_ast::helpers::is_docstring_stmt;
use ruff_python_parser::{parse_expression, parse_module};
use ruff_text_size::{Ranged, TextRange};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TeachingRange {
    pub min: u8,
    pub max: u8,
}

impl TeachingRange {
    pub const fn exact(level: u8) -> Self {
        Self {
            min: level,
            max: level,
        }
    }

    pub const fn between(min: u8, max: u8) -> Self {
        Self { min, max }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CurriculumUnit {
    pub directory: &'static str,
    pub levels: TeachingRange,
}

pub const CURRICULUM: &[CurriculumUnit] = &[
    CurriculumUnit {
        directory: "02-conceitos-basicos",
        levels: TeachingRange::exact(0),
    },
    CurriculumUnit {
        directory: "03-projeto-de-programas",
        levels: TeachingRange::exact(0),
    },
    CurriculumUnit {
        directory: "04-selecao",
        levels: TeachingRange::exact(1),
    },
    CurriculumUnit {
        directory: "05-tipos-de-dados",
        levels: TeachingRange::exact(2),
    },
    CurriculumUnit {
        directory: "06-repeticao-e-arranjos",
        levels: TeachingRange::exact(3),
    },
    CurriculumUnit {
        directory: "07-outras-formas-de-repeticao",
        levels: TeachingRange::exact(3),
    },
    CurriculumUnit {
        directory: "08-memoria-e-passagem-de-parametros",
        levels: TeachingRange::exact(3),
    },
    CurriculumUnit {
        directory: "09-recursividade",
        levels: TeachingRange::between(1, 3),
    },
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TestCase {
    pub id: String,
    pub context: String,
    pub setup: Vec<String>,
    pub expression: String,
    pub expected: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TestModule {
    pub relative_path: PathBuf,
    pub levels: TeachingRange,
    pub source: String,
    pub testable_source: String,
    pub cases: Vec<TestCase>,
}

impl TestModule {
    pub fn generated_script(&self) -> String {
        render_assert_script(self)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Summary {
    pub modules: usize,
    pub cases: usize,
}

#[derive(Debug)]
pub enum Error {
    MissingCurriculumRoot(PathBuf),
    Io { path: PathBuf, source: io::Error },
    Parse { path: PathBuf, message: String },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::MissingCurriculumRoot(path) => {
                write!(f, "curriculum root not found: {}", path.display())
            }
            Error::Io { path, source } => {
                write!(f, "I/O error at {}: {source}", path.display())
            }
            Error::Parse { path, message } => {
                write!(f, "failed to parse {}: {message}", path.display())
            }
        }
    }
}

impl std::error::Error for Error {}

pub fn collect_curriculum(root: &Path) -> Result<Vec<TestModule>, Error> {
    if !root.exists() {
        return Err(Error::MissingCurriculumRoot(root.to_path_buf()));
    }

    let mut modules = Vec::new();
    for unit in CURRICULUM {
        let directory = root.join(unit.directory);
        if !directory.exists() {
            continue;
        }
        let mut files = Vec::new();
        collect_python_files(&directory, &mut files)?;
        files.sort();

        for path in files {
            let relative_path = path
                .strip_prefix(root)
                .expect("collected file should be inside curriculum root");
            modules.push(extract_from_file(&path, relative_path, unit.levels)?);
        }
    }

    Ok(modules)
}

pub fn summarize(modules: &[TestModule]) -> Summary {
    Summary {
        modules: modules.len(),
        cases: modules.iter().map(|module| module.cases.len()).sum(),
    }
}

pub fn summarize_by_directory(modules: &[TestModule]) -> BTreeMap<String, Summary> {
    let mut out: BTreeMap<String, Summary> = BTreeMap::new();

    for module in modules {
        let key = module
            .relative_path
            .iter()
            .next()
            .map(|segment| segment.to_string_lossy().into_owned())
            .unwrap_or_else(|| "<root>".to_owned());

        let entry = out.entry(key).or_default();
        entry.modules += 1;
        entry.cases += module.cases.len();
    }

    out
}

pub fn write_generated_scripts(modules: &[TestModule], output_dir: &Path) -> Result<usize, Error> {
    let mut written = 0;

    for module in modules {
        if module.cases.is_empty() {
            continue;
        }

        let target = output_dir.join(&module.relative_path);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|source| Error::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }

        fs::write(&target, module.generated_script()).map_err(|source| Error::Io {
            path: target.clone(),
            source,
        })?;
        written += 1;
    }

    Ok(written)
}

pub fn render_assert_script(module: &TestModule) -> String {
    let mut script = module.testable_source.trim().to_owned();
    if !script.is_empty() {
        script.push_str("\n\n");
    }
    script.push_str("# Generated from doctests.\n");
    for case in &module.cases {
        for statement in &case.setup {
            script.push_str(statement);
            script.push('\n');
        }
        script.push_str("assert ");
        script.push_str(&case.expression);
        script.push_str(" == ");
        script.push_str(&case.expected);
        script.push('\n');
    }
    script
}

fn extract_from_file(
    path: &Path,
    relative_path: &Path,
    levels: TeachingRange,
) -> Result<TestModule, Error> {
    let source = fs::read_to_string(path).map_err(|source| Error::Io {
        path: path.to_path_buf(),
        source,
    })?;
    extract_from_source(relative_path, &source, levels)
}

pub fn extract_from_source(
    relative_path: &Path,
    source: &str,
    levels: TeachingRange,
) -> Result<TestModule, Error> {
    let parsed = parse_module(source).map_err(|err| Error::Parse {
        path: relative_path.to_path_buf(),
        message: err.to_string(),
    })?;

    let mut raw_cases = Vec::new();
    if let Some(docstring) = extract_docstring(parsed.suite()) {
        collect_doctests("<module>", docstring, &mut raw_cases);
    }
    for stmt in parsed.suite() {
        collect_stmt_doctests(stmt, None, &mut raw_cases);
    }

    let cases = raw_cases
        .into_iter()
        .enumerate()
        .map(|(index, case)| TestCase {
            id: format!(
                "{}::{}::{}",
                relative_path.display(),
                case.context,
                index + 1
            ),
            context: case.context,
            setup: case.setup,
            expression: case.expression,
            expected: case.expected,
        })
        .collect();

    let testable_source = extract_testable_source(source, parsed.suite());

    Ok(TestModule {
        relative_path: relative_path.to_path_buf(),
        levels,
        source: source.to_owned(),
        testable_source,
        cases,
    })
}

#[derive(Debug)]
struct RawCase {
    context: String,
    setup: Vec<String>,
    expression: String,
    expected: String,
}

fn collect_stmt_doctests(stmt: &Stmt, parent: Option<&str>, out: &mut Vec<RawCase>) {
    match stmt {
        Stmt::FunctionDef(function) => {
            let context = join_context(parent, function.name.as_str());
            if let Some(docstring) = extract_docstring(&function.body) {
                collect_doctests(&context, docstring, out);
            }
            for stmt in &function.body {
                collect_stmt_doctests(stmt, Some(&context), out);
            }
        }
        Stmt::ClassDef(class_def) => {
            let context = join_context(parent, class_def.name.as_str());
            if let Some(docstring) = extract_docstring(&class_def.body) {
                collect_doctests(&context, docstring, out);
            }
            for stmt in &class_def.body {
                collect_stmt_doctests(stmt, Some(&context), out);
            }
        }
        _ => {}
    }
}

fn join_context(parent: Option<&str>, name: &str) -> String {
    match parent {
        Some(parent) => format!("{parent}.{name}"),
        None => name.to_owned(),
    }
}

fn extract_docstring(body: &[Stmt]) -> Option<&str> {
    let first = body.first()?;
    if !is_docstring_stmt(first) {
        return None;
    }

    match first {
        Stmt::Expr(expr) => expr
            .value
            .as_string_literal_expr()
            .map(|string| string.value.to_str()),
        _ => None,
    }
}

fn collect_doctests(context: &str, docstring: &str, out: &mut Vec<RawCase>) {
    let lines: Vec<&str> = docstring.lines().collect();
    let mut index = 0;
    let mut pending_setup = Vec::new();

    while index < lines.len() {
        let line = lines[index];
        let trimmed = line.trim_start();
        let Some(rest) = trimmed.strip_prefix(">>>") else {
            index += 1;
            continue;
        };

        let mut block_lines = vec![strip_inline_comment(strip_prompt_space(rest)).to_owned()];
        index += 1;

        while index < lines.len() {
            let continuation = lines[index].trim_start();
            let Some(rest) = continuation.strip_prefix("...") else {
                break;
            };
            block_lines.push(strip_inline_comment(strip_prompt_space(rest)).to_owned());
            index += 1;
        }

        if block_lines
            .iter()
            .all(|line| line.trim().is_empty() || line.trim_start().starts_with('#'))
        {
            continue;
        }

        let indent = &line[..line.len() - trimmed.len()];
        let mut expected_lines = Vec::new();
        while index < lines.len() {
            let output_line = lines[index];
            let trimmed = output_line.trim_start();
            if trimmed.starts_with(">>>") || trimmed.is_empty() {
                break;
            }
            expected_lines.push(strip_indent(output_line, indent).to_owned());
            index += 1;
        }

        let block = block_lines.join("\n");

        if expected_lines.is_empty() {
            if parse_module(&block).is_ok() {
                pending_setup.push(block);
            }
            continue;
        }

        let expected = expected_lines.join("\n");

        if parse_expression(&block).is_err() || parse_expression(&expected).is_err() {
            continue;
        }

        out.push(RawCase {
            context: context.to_owned(),
            setup: std::mem::take(&mut pending_setup),
            expression: block,
            expected,
        });
    }
}

fn strip_prompt_space(text: &str) -> &str {
    text.strip_prefix(' ').unwrap_or(text)
}

fn strip_indent<'a>(text: &'a str, indent: &str) -> &'a str {
    text.strip_prefix(indent).unwrap_or(text)
}

fn strip_inline_comment(text: &str) -> String {
    let mut result = String::new();
    let mut quote = None;
    let mut escaped = false;

    for ch in text.chars() {
        match quote {
            Some(current) => {
                result.push(ch);
                if escaped {
                    escaped = false;
                } else if ch == '\\' {
                    escaped = true;
                } else if ch == current {
                    quote = None;
                }
            }
            None => {
                if ch == '#' {
                    break;
                }
                if ch == '\'' || ch == '"' {
                    quote = Some(ch);
                }
                result.push(ch);
            }
        }
    }

    result.trim_end().to_owned()
}

fn extract_testable_source(source: &str, suite: &[Stmt]) -> String {
    let snippets: Vec<&str> = suite
        .iter()
        .filter(|stmt| keep_top_level_statement(stmt))
        .map(|stmt| slice_range(source, stmt.range()))
        .collect();

    snippets.join("\n\n")
}

fn keep_top_level_statement(stmt: &Stmt) -> bool {
    matches!(
        stmt,
        Stmt::Import(_)
            | Stmt::ImportFrom(_)
            | Stmt::FunctionDef(_)
            | Stmt::ClassDef(_)
            | Stmt::Assign(_)
            | Stmt::AnnAssign(_)
            | Stmt::TypeAlias(_)
    )
}

fn slice_range(source: &str, range: TextRange) -> &str {
    &source[range.start().to_usize()..range.end().to_usize()]
}

fn collect_python_files(directory: &Path, out: &mut Vec<PathBuf>) -> Result<(), Error> {
    for entry in fs::read_dir(directory).map_err(|source| Error::Io {
        path: directory.to_path_buf(),
        source,
    })? {
        let entry = entry.map_err(|source| Error::Io {
            path: directory.to_path_buf(),
            source,
        })?;
        let path = entry.path();

        if path.is_dir() {
            collect_python_files(&path, out)?;
        } else if path.extension().is_some_and(|ext| ext == "py") {
            out.push(path);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_cases_and_removes_top_level_execution() {
        let source = r#"
PI: float = 3.14

def dobro(x: int) -> int:
    '''
    >>> # comentario
    >>> dobro(4)
    8
    >>> dobro(0)
    0
    '''
    return x * 2

print(dobro(10))
"#;

        let module = extract_from_source(
            Path::new("02-conceitos-basicos/exemplos/dobro.py"),
            source,
            TeachingRange::exact(0),
        )
        .expect("fixture should parse");

        assert_eq!(module.cases.len(), 2);
        assert!(module.cases[0].setup.is_empty());
        assert_eq!(module.cases[0].expression, "dobro(4)");
        assert_eq!(module.cases[0].expected, "8");
        assert!(module.testable_source.contains("PI: float = 3.14"));
        assert!(module.testable_source.contains("def dobro"));
        assert!(!module.testable_source.contains("print(dobro(10))"));
    }

    #[test]
    fn renders_assert_script() {
        let source = r#"
def nome(x: str) -> str:
    '''
    >>> nome('ana')
    'ANA'
    '''
    return x.upper()
"#;

        let module = extract_from_source(
            Path::new("03-projeto-de-programas/exemplos/nome.py"),
            source,
            TeachingRange::exact(0),
        )
        .expect("fixture should parse");

        let script = module.generated_script();
        assert!(script.contains("def nome"));
        assert!(script.contains("assert nome('ana') == 'ANA'"));
    }

    #[test]
    fn preserves_setup_and_strips_inline_comments() {
        let source = r#"
class Cor:
    VERDE = 'verde'

def identidade(x):
    '''
    >>> alias = Cor # comentario
    >>> identidade(alias.VERDE) # comentario
    'verde'
    >>> valores = [3, 1]
    >>> valores[0]
    3
    '''
    return x
"#;

        let module = extract_from_source(
            Path::new("05-tipos-de-dados/exemplos/identidade.py"),
            source,
            TeachingRange::exact(2),
        )
        .expect("fixture should parse");

        assert_eq!(module.cases.len(), 2);
        assert_eq!(module.cases[0].setup, vec!["alias = Cor"]);
        assert_eq!(module.cases[0].expression, "identidade(alias.VERDE)");
        assert_eq!(module.cases[0].expected, "'verde'");
        assert_eq!(module.cases[1].setup, vec!["valores = [3, 1]"]);

        let script = module.generated_script();
        assert!(script.contains("alias = Cor\nassert identidade(alias.VERDE) == 'verde'"));
        assert!(script.contains("valores = [3, 1]\nassert valores[0] == 3"));
    }

    #[test]
    fn scans_external_curriculum_when_available() {
        let root = Path::new("../na-programacao");
        if !root.exists() {
            return;
        }

        let modules = collect_curriculum(root).expect("external curriculum should scan");
        let summary = summarize(&modules);

        assert!(summary.modules > 0);
        assert!(summary.cases > 0);
    }
}
