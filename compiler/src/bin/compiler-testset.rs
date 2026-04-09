use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

use compiler::test_set::{
    CURRICULUM, collect_curriculum, summarize, summarize_by_directory, write_generated_scripts,
};

fn main() -> ExitCode {
    let mut args = env::args_os().skip(1);
    let root = args
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("../na-programacao"));
    let output_dir = args.next().map(PathBuf::from);

    let modules = match collect_curriculum(&root) {
        Ok(modules) => modules,
        Err(err) => {
            eprintln!("compiler-testset: {err}");
            return ExitCode::FAILURE;
        }
    };

    let summary = summarize(&modules);
    println!("root: {}", root.display());
    println!("units: {}", CURRICULUM.len());
    println!("modules: {}", summary.modules);
    println!("cases: {}", summary.cases);

    for (directory, summary) in summarize_by_directory(&modules) {
        println!(
            "{directory}: {} modules, {} cases",
            summary.modules, summary.cases
        );
    }

    if let Some(output_dir) = output_dir {
        match write_generated_scripts(&modules, &output_dir) {
            Ok(written) => {
                println!("written: {written} scripts to {}", output_dir.display());
            }
            Err(err) => {
                eprintln!("compiler-testset: {err}");
                return ExitCode::FAILURE;
            }
        }
    }

    ExitCode::SUCCESS
}
