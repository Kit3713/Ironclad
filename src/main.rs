mod ast;
mod errors;
mod manifest;
mod parser;
mod validate;

use clap::{Parser as ClapParser, Subcommand};
use std::fs;
use std::path::PathBuf;
use std::process;

use crate::errors::{Severity, format_diagnostic};

#[derive(ClapParser)]
#[command(name = "ironclad")]
#[command(about = "Ironclad — declarative Linux system configuration compiler")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Parse and validate an Ironclad source file
    Check {
        /// Path to an Ironclad storage file (.icl)
        file: String,

        /// Output the AST as JSON
        #[arg(long)]
        ast: bool,

        /// Output the AST as pretty-printed debug format
        #[arg(long)]
        debug: bool,
    },

    /// Compile an Ironclad source file to a build toolchain
    Compile {
        /// Path to an Ironclad storage file (.icl)
        file: String,

        /// Build target: iso, chroot, image, bare, delta
        #[arg(long, default_value = "iso")]
        target: String,

        /// Output directory for emitted artifacts
        #[arg(long, default_value = "./build")]
        output_dir: PathBuf,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Check { file, ast, debug } => cmd_check(&file, ast, debug),
        Commands::Compile {
            file,
            target,
            output_dir,
        } => cmd_compile(&file, &target, &output_dir),
    }
}

/// Parse, validate, and optionally display AST information.
fn cmd_check(file: &str, show_ast: bool, show_debug: bool) {
    let source = read_source(file);
    let file_ast = parse_source(&source);
    let warnings = validate_ast(&file_ast, &source);

    for diag in &warnings {
        eprint!("{}", format_diagnostic(diag, &source));
    }

    if show_ast {
        let json = serde_json::to_string_pretty(&file_ast).unwrap();
        println!("{json}");
    } else if show_debug {
        println!("{file_ast:#?}");
    } else {
        print_check_summary(&file_ast, &warnings);
    }
}

/// Parse, validate, convert to manifest, sign, and emit to output directory.
fn cmd_compile(file: &str, target: &str, output_dir: &std::path::Path) {
    let build_target: ironclad_emit::BuildTarget = match target.parse() {
        Ok(t) => t,
        Err(e) => {
            eprintln!("error: {e}");
            process::exit(1);
        }
    };

    let source = read_source(file);
    let file_ast = parse_source(&source);
    let warnings = validate_ast(&file_ast, &source);

    for diag in &warnings {
        eprint!("{}", format_diagnostic(diag, &source));
    }

    // Convert AST to manifest
    let manifest = manifest::storage_file_to_manifest(&file_ast);

    // Serialize to CBOR
    let cbor = match ironclad_manifest::serialize_manifest(&manifest) {
        Ok(bytes) => bytes,
        Err(e) => {
            eprintln!("error: {e}");
            process::exit(1);
        }
    };

    // Sign the manifest
    let signed = match ironclad_manifest::signing::sign_manifest(&cbor) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: {e}");
            process::exit(1);
        }
    };

    // Build the toolchain plan
    let plan = ironclad_emit::ToolchainPlan {
        manifest,
        signed_manifest: signed,
        target: build_target,
        output_dir: output_dir.to_path_buf(),
    };

    // Emit the signed manifest
    let emitter = ironclad_emit::ManifestEmitter;
    match ironclad_emit::Emitter::emit(&emitter, &plan) {
        Ok(path) => {
            println!("compiled: target={build_target}");
            println!("  manifest: {}", path.display());
            println!("  output:   {}", output_dir.display());
            if !warnings.is_empty() {
                println!("  {} warning(s)", warnings.len());
            }
        }
        Err(e) => {
            eprintln!("error: {e}");
            process::exit(1);
        }
    }
}

// ─── Shared Helpers ─────────────────────────────────────────

fn read_source(file: &str) -> String {
    match fs::read_to_string(file) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: cannot read `{file}`: {e}");
            process::exit(1);
        }
    }
}

fn parse_source(source: &str) -> ast::StorageFile {
    match parser::parse_storage(source) {
        Ok(ast) => ast,
        Err(e) => {
            eprintln!("{e}");
            process::exit(1);
        }
    }
}

fn validate_ast(
    file_ast: &ast::StorageFile,
    source: &str,
) -> Vec<ironclad_diagnostics::Diagnostic> {
    match validate::validate(file_ast) {
        Ok(warnings) => warnings,
        Err(errors::IroncladError::ValidationError { errors }) => {
            let mut warning_count = 0;
            let mut error_count = 0;
            for diag in &errors {
                eprint!("{}", format_diagnostic(diag, source));
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
    }
}

fn print_check_summary(file_ast: &ast::StorageFile, warnings: &[ironclad_diagnostics::Diagnostic]) {
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

#[cfg(test)]
mod tests;
