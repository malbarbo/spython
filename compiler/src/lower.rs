//! From the syntax tree and the types of the checker to the IR.

use ruff_db::files::File;
use ruff_db::parsed::parsed_module;
use ruff_db::source::source_text;
use ruff_python_ast as ast;
use ruff_text_size::Ranged;
use ty_module_resolver::{KnownModule, file_to_module};
use ty_python_semantic::types::lowering::{Shape, expr_shape};
use ty_python_semantic::{Db, SemanticModel};

use crate::Error;
use crate::ir::{Expr, Program, Stmt};

pub(crate) fn lower(db: &dyn Db, file: File) -> Result<Program, Error> {
    let program_file = db.program_file(file);
    let parsed = parsed_module(db, program_file.python_file(db));
    let module = parsed.load(db);
    let model = SemanticModel::new(db, program_file);
    let source = source_text(db, file);
    let lowering = Lowering {
        model: &model,
        source: source.as_str(),
    };
    let body = module
        .suite()
        .iter()
        .map(|stmt| lowering.stmt(stmt))
        .collect::<Result<_, _>>()?;
    Ok(Program { body })
}

struct Lowering<'a, 'db> {
    model: &'a SemanticModel<'db>,
    source: &'a str,
}

impl Lowering<'_, '_> {
    fn stmt(&self, stmt: &ast::Stmt) -> Result<Stmt, Error> {
        match stmt {
            ast::Stmt::Expr(e) => Ok(Stmt::Expr(self.expr(&e.value)?)),
            _ => Err(unsupported(stmt, "this statement")),
        }
    }

    fn expr(&self, expr: &ast::Expr) -> Result<Expr, Error> {
        match expr {
            ast::Expr::StringLiteral(s) => {
                let value = s.value.to_str();
                // The parser reads the escape of a lone surrogate, as in
                // "\ud800", as U+FFFD. Python keeps the surrogate, and printing
                // it raises UnicodeEncodeError.
                let source = self
                    .source
                    .get(std::ops::Range::<usize>::from(expr.range()));
                if value.contains('\u{fffd}') && source.is_none_or(|text| text.contains('\\')) {
                    return Err(unsupported(
                        expr,
                        "a str with an escape that may be a surrogate",
                    ));
                }
                Ok(Expr::Str(value.to_owned()))
            }
            ast::Expr::Call(call) if self.is_builtin_print(&call.func) => self.print(call),
            _ => Err(unsupported(expr, "this expression")),
        }
    }

    fn print(&self, call: &ast::ExprCall) -> Result<Expr, Error> {
        if !call.arguments.keywords.is_empty() {
            return Err(unsupported(call, "print with keyword arguments"));
        }
        let args = call
            .arguments
            .args
            .iter()
            .map(|arg| match self.expr(arg)? {
                Expr::Str(s) => Ok(Expr::Str(s)),
                _ => Err(unsupported(arg, "print of a value that is not a str")),
            })
            .collect::<Result<_, _>>()?;
        Ok(Expr::Print(args))
    }

    /// Returns `true` if `func` is the `print` of the builtins, `false`
    /// otherwise.
    fn is_builtin_print(&self, func: &ast::Expr) -> bool {
        matches!(
            expr_shape(self.model, func),
            Some(Shape::Function(def)) if def.name == "print" && is_builtins(self.model.db(), def.file)
        )
    }
}

/// Returns `true` if `file` is the `builtins` module, `false` otherwise.
fn is_builtins(db: &dyn Db, file: File) -> bool {
    let resolver_file = db.program_file(file).resolver_file(db);
    file_to_module(db, resolver_file).is_some_and(|m| m.is_known(db, KnownModule::Builtins))
}

fn unsupported(node: &impl Ranged, what: &str) -> Error {
    Error::Unsupported {
        range: node.range(),
        what: what.to_owned(),
    }
}
