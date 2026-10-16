//! Implementation of the `convert` subcommand.

use anyhow::{bail, Context, Result};
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use std::fs;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;

use omni3mf_core::{
    convert_stl_to_3mf, PipelineOptions, RasterizerOptions, SanitizerOptions, ScaleAnomaly,
};

use crate::args::ConvertArgs;
use crate::table::{print_batch_summary, BatchItemRecord};

/// Executes the `convert` subcommand for either a single file or a batch directory.
pub fn execute_convert(args: ConvertArgs) -> Result<()> {
    if !args.input.exists() {
        bail!("Input path does not exist: {}", args.input.display());
    }

    if args.input.is_dir() {
        execute_batch_convert(args)
    } else {
        execute_single_convert(args)
    }
}

/// Converts a single STL file into a 3MF package.
fn execute_single_convert(args: ConvertArgs) -> Result<()> {
    let input_path = &args.input;
    let out_path = match &args.output {
        Some(out) => {
            if out.is_dir() {
                let stem = input_path.file_stem().unwrap_or_default();
                out.join(format!("{}.3mf", stem.to_string_lossy()))
            } else {
                out.clone()
            }
        }
        None => input_path.with_extension("3mf"),
    };

    if out_path == *input_path
        || (input_path.exists()
            && out_path.exists()
            && input_path.canonicalize().ok() == out_path.canonicalize().ok())
    {
        bail!("Output path cannot be identical to input path; specify -o with a distinct filename.");
    }

    if let Some(parent) = out_path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create output directory: {}", parent.display()))?;
        }
    }

    let is_tty = std::io::stdout().is_terminal();
    let spinner = if !args.quiet && is_tty {
        let sp = ProgressBar::new_spinner();
        sp.set_style(
            ProgressStyle::default_spinner()
                .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏ ")
                .template("{spinner:.cyan} {msg}")
                .unwrap(),
        );
        sp.set_message(format!("Converting {}...", input_path.display()));
        sp.enable_steady_tick(std::time::Duration::from_millis(80));
        Some(sp)
    } else {
        None
    };

    let start = Instant::now();
    let stl_bytes = fs::read(input_path)
        .with_context(|| format!("Failed to read input STL file: {}", input_path.display()))?;

    let pipeline_opts = PipelineOptions {
        tolerance: args.merge_tolerance,
        patch_holes: !args.no_repair,
        max_hole_edges: args.hole_max_edges,
        split_shells: !args.no_split,
        scale_override: args.scale,
        include_thumbnail: true,
        thumbnail_options: RasterizerOptions::default(),
        sanitize_output: args.strip_slicer_config,
        sanitizer_options: SanitizerOptions {
            strip_vendor_profiles: args.strip_slicer_config,
            preserve_supports: args.preserve_supports,
            preserve_colors: true,
            preserve_thumbnails: true,
        },
        check_scale: true,
    };

    let result = convert_stl_to_3mf(&stl_bytes, &pipeline_opts)
        .map_err(|e| anyhow::anyhow!("Conversion failed for '{}': {}", input_path.display(), e))?;

    fs::write(&out_path, &result.package_bytes)
        .with_context(|| format!("Failed to write output 3MF: {}", out_path.display()))?;

    let elapsed = start.elapsed();
    if let Some(sp) = spinner {
        sp.finish_and_clear();
    }

    let mut total_verts = 0;
    let mut total_faces = 0;
    for s in &result.shells {
        total_verts += s.mesh.vertices.len();
        total_faces += s.mesh.faces.len();
    }

    if !args.quiet {
        println!(
            "✓ Converted '{}' -> '{}' in {:.2}ms",
            input_path.display(),
            out_path.display(),
            elapsed.as_secs_f64() * 1000.0
        );
        println!(
            "  Shells: {} | Vertices: {} | Faces: {} | Size: {} bytes",
            result.shells.len(),
            total_verts,
            total_faces,
            result.package_bytes.len()
        );

        if let Some(scale_rep) = &result.scale_report {
            if scale_rep.anomaly != ScaleAnomaly::Normal && !scale_rep.user_prompt.is_empty() {
                eprintln!("  ⚠ Scale Warning: {}", scale_rep.user_prompt);
            }
        }
    }

    Ok(())
}

/// Recursively scans a directory for `.stl` and `.STL` files.
fn collect_stl_files(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    let mut stack = vec![dir.to_path_buf()];

    while let Some(current_dir) = stack.pop() {
        for entry in fs::read_dir(&current_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if let Some(ext) = path.extension() {
                if ext.eq_ignore_ascii_case("stl") {
                    files.push(path);
                }
            }
        }
    }

    files.sort();
    Ok(files)
}

/// Converts a directory of STL files in parallel across a Rayon thread pool.
fn execute_batch_convert(args: ConvertArgs) -> Result<()> {
    let stl_files = collect_stl_files(&args.input)?;
    if stl_files.is_empty() {
        bail!("No STL files found in directory '{}'", args.input.display());
    }

    let out_dir = match &args.output {
        Some(out) => out.clone(),
        None => args.input.clone(),
    };
    fs::create_dir_all(&out_dir)
        .with_context(|| format!("Failed to create batch output directory: {}", out_dir.display()))?;

    // Configure Rayon thread pool
    let pool = if args.threads > 0 {
        rayon::ThreadPoolBuilder::new()
            .num_threads(args.threads)
            .build()
            .context("Failed to initialize Rayon thread pool")?
    } else {
        rayon::ThreadPoolBuilder::new()
            .build()
            .context("Failed to initialize Rayon thread pool")?
    };

    let total_files = stl_files.len();
    let is_tty = std::io::stdout().is_terminal();
    let pb = if !args.quiet && is_tty {
        let p = ProgressBar::new(total_files as u64);
        p.set_style(
            ProgressStyle::default_bar()
                .template("[{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta}) {msg}")
                .unwrap()
                .progress_chars("=>-"),
        );
        p
    } else {
        ProgressBar::hidden()
    };

    let pb_shared = Arc::new(pb);
    let failed_counter = Arc::new(AtomicUsize::new(0));

    let batch_start = Instant::now();

    let records: Vec<BatchItemRecord> = pool.install(|| {
        stl_files
            .par_iter()
            .map(|input_path| {
                let file_name = input_path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();

                // Compute output relative path
                let rel = input_path.strip_prefix(&args.input).unwrap_or(input_path);
                let mut dest = out_dir.join(rel);
                dest.set_extension("3mf");

                if let Some(parent) = dest.parent() {
                    fs::create_dir_all(parent).ok();
                }

                pb_shared.set_message(format!("Processing {}", file_name));

                let item_start = Instant::now();
                let convert_res = (|| -> Result<(usize, usize, usize)> {
                    let stl_bytes = fs::read(input_path)
                        .with_context(|| format!("Failed to read {}", input_path.display()))?;

                    let pipeline_opts = PipelineOptions {
                        tolerance: args.merge_tolerance,
                        patch_holes: !args.no_repair,
                        max_hole_edges: args.hole_max_edges,
                        split_shells: !args.no_split,
                        scale_override: args.scale,
                        include_thumbnail: true,
                        thumbnail_options: RasterizerOptions::default(),
                        sanitize_output: args.strip_slicer_config,
                        sanitizer_options: SanitizerOptions {
                            strip_vendor_profiles: args.strip_slicer_config,
                            preserve_supports: args.preserve_supports,
                            preserve_colors: true,
                            preserve_thumbnails: true,
                        },
                        check_scale: true,
                    };

                    let res = convert_stl_to_3mf(&stl_bytes, &pipeline_opts)?;
                    fs::write(&dest, &res.package_bytes)?;

                    let mut verts = 0;
                    let mut faces = 0;
                    for s in &res.shells {
                        verts += s.mesh.vertices.len();
                        faces += s.mesh.faces.len();
                    }
                    Ok((res.shells.len(), verts, faces))
                })();

                let duration = item_start.elapsed();
                pb_shared.inc(1);

                match convert_res {
                    Ok((shells, vertices, faces)) => BatchItemRecord {
                        file_name,
                        shells,
                        vertices,
                        faces,
                        duration,
                        status: Ok(()),
                    },
                    Err(e) => {
                        failed_counter.fetch_add(1, Ordering::SeqCst);
                        BatchItemRecord {
                            file_name,
                            shells: 0,
                            vertices: 0,
                            faces: 0,
                            duration,
                            status: Err(e.to_string()),
                        }
                    }
                }
            })
            .collect()
    });

    pb_shared.finish_and_clear();
    let total_elapsed = batch_start.elapsed();

    if !args.quiet {
        print_batch_summary(&records, total_elapsed);
    }

    let failed_count = failed_counter.load(Ordering::SeqCst);
    if failed_count > 0 {
        bail!("{}/{} files failed to convert", failed_count, total_files);
    }

    Ok(())
}
