//! Implementation of the `sanitize` subcommand.

use anyhow::{bail, Context, Result};
use std::fs;
use std::path::Path;

use omni3mf_core::{sanitize_3mf_archive, SanitizerOptions};

use crate::args::SanitizeArgs;

/// Executes the `sanitize` subcommand.
pub fn execute_sanitize(args: SanitizeArgs) -> Result<()> {
    if !args.input.exists() {
        bail!("Input path does not exist: {}", args.input.display());
    }

    let input_bytes = fs::read(&args.input)
        .with_context(|| format!("Failed to read input 3MF file: {}", args.input.display()))?;

    let out_path = match &args.output {
        Some(out) => {
            if out.is_dir() {
                let stem = args.input.file_stem().unwrap_or_default();
                out.join(format!("{}_sanitized.3mf", stem.to_string_lossy()))
            } else {
                out.clone()
            }
        }
        None => {
            let stem = args.input.file_stem().unwrap_or_default();
            let parent = args.input.parent().unwrap_or_else(|| Path::new(""));
            parent.join(format!("{}_sanitized.3mf", stem.to_string_lossy()))
        }
    };

    let options = SanitizerOptions {
        strip_vendor_profiles: args.strip_config,
        preserve_supports: args.preserve_supports,
        preserve_colors: args.preserve_colors,
        preserve_thumbnails: true,
    };

    let sanitized_bytes = sanitize_3mf_archive(&input_bytes, &options)
        .with_context(|| format!("Failed to sanitize 3MF archive: {}", args.input.display()))?;

    if let Some(parent) = out_path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).ok();
        }
    }

    fs::write(&out_path, &sanitized_bytes)
        .with_context(|| format!("Failed to write sanitized output: {}", out_path.display()))?;

    let in_len = input_bytes.len();
    let out_len = sanitized_bytes.len();
    let diff = in_len as i64 - out_len as i64;

    println!(
        "✓ Sanitized 3MF archive '{}' -> '{}'",
        args.input.display(),
        out_path.display()
    );
    println!(
        "  Original Size:   {} bytes\n  Sanitized Size:  {} bytes ({:+} bytes)",
        in_len, out_len, -diff
    );
    println!(
        "  Preserved:       Colors: {} | Supports: {}",
        args.preserve_colors, args.preserve_supports
    );

    Ok(())
}
