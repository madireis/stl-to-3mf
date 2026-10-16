//! CLI arguments and command definitions for StlTo3mf (Omni3MF Engine).

use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

/// High-performance, 100% safe Rust STL to 3MF converter with automated mesh repair, multi-shell separation, and slicer profile sanitization.
#[derive(Parser, Debug)]
#[command(
    name = "stl-to-3mf",
    author,
    version,
    about = "High-performance, 100% safe Rust STL to 3MF converter with automated mesh repair, multi-shell separation, and slicer profile sanitization",
    long_about = "StlTo3mf (powered by Omni3MF Engine) converts binary/ASCII STL files into compliant multi-shell 3MF packages,\n\
                  with spatial vertex welding, hole healing, software thumbnail rendering, and slicer scrubbing."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Convert an STL file or directory of STL files into 3MF
    Convert(ConvertArgs),

    /// Inspect geometric metrics, shells, and scale sanity of an STL or 3MF file
    Inspect(InspectArgs),

    /// Sanitize an existing 3MF package by stripping vendor slicer profiles and printer configs
    Sanitize(SanitizeArgs),
}

#[derive(Args, Debug, Clone)]
pub struct ConvertArgs {
    /// Input STL file or directory containing STL files
    #[arg(value_name = "INPUT")]
    pub input: PathBuf,

    /// Output .3mf path (or output directory if input is a directory). Defaults to replacing extension with .3mf
    #[arg(short, long, value_name = "OUTPUT")]
    pub output: Option<PathBuf>,

    /// Vertex spatial welding tolerance in mm
    #[arg(long, default_value_t = 1e-6)]
    pub merge_tolerance: f64,

    /// Skip hole detection and healing
    #[arg(long)]
    pub no_repair: bool,

    /// Maximum boundary loop edges for hole patching
    #[arg(long, default_value_t = 64)]
    pub hole_max_edges: usize,

    /// Keep as a single <object> instead of separating into discrete connected shells
    #[arg(long)]
    pub no_split: bool,

    /// Purge vendor configuration files and metadata from the generated package
    #[arg(long)]
    pub strip_slicer_config: bool,

    /// Preserve support meshes (defaults to true)
    #[arg(long, default_value_t = true, num_args(0..=1), default_missing_value = "true")]
    pub preserve_supports: bool,

    /// Manual scale multiplier override
    #[arg(long)]
    pub scale: Option<f64>,

    /// Worker thread count for rayon (0 = automatic/all CPU cores)
    #[arg(long, default_value_t = 0)]
    pub threads: usize,

    /// Suppress progress bars and interactive spinners
    #[arg(short, long)]
    pub quiet: bool,

    /// Verbose output logging
    #[arg(short, long)]
    pub verbose: bool,
}

#[derive(Args, Debug, Clone)]
pub struct InspectArgs {
    /// STL or 3MF file to inspect
    #[arg(value_name = "INPUT")]
    pub input: PathBuf,

    /// Output formatted JSON statistics
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug, Clone)]
pub struct SanitizeArgs {
    /// Existing .3mf file to sanitize
    #[arg(value_name = "INPUT")]
    pub input: PathBuf,

    /// Sanitized .3mf output path (defaults to <stem>_sanitized.3mf)
    #[arg(short, long, value_name = "OUTPUT")]
    pub output: Option<PathBuf>,

    /// Strip vendor slicer profiles, ini, cfg, and gcode files (defaults to true)
    #[arg(long, default_value_t = true, num_args(0..=1), default_missing_value = "true")]
    pub strip_config: bool,

    /// Preserve custom support geometry in 3MF (defaults to true)
    #[arg(long, default_value_t = true, num_args(0..=1), default_missing_value = "true")]
    pub preserve_supports: bool,

    /// Preserve color records and palettes in 3MF (defaults to true)
    #[arg(long, default_value_t = true, num_args(0..=1), default_missing_value = "true")]
    pub preserve_colors: bool,
}
