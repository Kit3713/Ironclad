mod ast;
mod errors;
mod parser;
mod validate;

use clap::Parser as ClapParser;
use std::fs;
use std::process;

use crate::errors::{Severity, format_diagnostic};

#[derive(ClapParser)]
#[command(name = "ironclad-storage")]
#[command(about = "Ironclad storage syntax parser and validator (Phase 1 prototype)")]
struct Cli {
    /// Path to an Ironclad storage file (.icl)
    file: String,

    /// Output the AST as JSON
    #[arg(long)]
    ast: bool,

    /// Output the AST as pretty-printed debug format
    #[arg(long)]
    debug: bool,
}

fn main() {
    let cli = Cli::parse();

    let source = match fs::read_to_string(&cli.file) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: cannot read `{}`: {e}", cli.file);
            process::exit(1);
        }
    };

    // Parse
    let file_ast = match parser::parse_storage(&source) {
        Ok(ast) => ast,
        Err(e) => {
            eprintln!("{e}");
            process::exit(1);
        }
    };

    // Validate
    let warnings = match validate::validate(&file_ast) {
        Ok(warnings) => warnings,
        Err(errors::IroncladError::ValidationError { errors }) => {
            let mut warning_count = 0;
            let mut error_count = 0;
            for diag in &errors {
                eprint!("{}", format_diagnostic(diag, &source));
                match diag.severity {
                    Severity::Error => error_count += 1,
                    Severity::Warning => warning_count += 1,
                }
            }
            eprintln!("\nfailed: {error_count} error(s), {warning_count} warning(s)");
            process::exit(1);
        }
        Err(e) => {
            eprintln!("{e}");
            process::exit(1);
        }
    };

    // Print warnings
    for diag in &warnings {
        eprint!("{}", format_diagnostic(diag, &source));
    }

    // Success output
    if cli.ast {
        let json = serde_json::to_string_pretty(&file_ast).unwrap();
        println!("{json}");
    } else if cli.debug {
        println!("{file_ast:#?}");
    } else {
        let decl_count = file_ast.declarations.len();
        let mut counts: Vec<(&str, usize)> = Vec::new();
        type DeclFilter = (&'static str, fn(&&ast::StorageDecl) -> bool);
        let types: &[DeclFilter] = &[
            ("disk", |d| matches!(d, ast::StorageDecl::Disk(_))),
            ("mdraid", |d| matches!(d, ast::StorageDecl::MdRaid(_))),
            ("zpool", |d| matches!(d, ast::StorageDecl::Zpool(_))),
            ("stratis", |d| matches!(d, ast::StorageDecl::Stratis(_))),
            ("multipath", |d| matches!(d, ast::StorageDecl::Multipath(_))),
            ("iscsi", |d| matches!(d, ast::StorageDecl::Iscsi(_))),
            ("nfs", |d| matches!(d, ast::StorageDecl::Nfs(_))),
            ("tmpfs", |d| matches!(d, ast::StorageDecl::Tmpfs(_))),
        ];
        for (name, pred) in types {
            let c = file_ast.declarations.iter().filter(pred).count();
            if c > 0 {
                counts.push((name, c));
            }
        }
        let detail = counts
            .iter()
            .map(|(n, c)| format!("{c} {n}"))
            .collect::<Vec<_>>()
            .join(", ");
        let selinux_note = if file_ast.selinux.is_some() {
            " + selinux"
        } else {
            ""
        };
        println!("ok: parsed {decl_count} declaration(s) ({detail}{selinux_note})");
        if !warnings.is_empty() {
            println!("   {} warning(s)", warnings.len());
        }
    }
}

#[cfg(test)]
mod tests;
