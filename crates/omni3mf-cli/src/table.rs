//! Formatting utilities for terminal summary tables.

use std::time::Duration;

/// Result of an individual item in a batch conversion run.
#[derive(Debug, Clone)]
pub struct BatchItemRecord {
    pub file_name: String,
    pub shells: usize,
    pub vertices: usize,
    pub faces: usize,
    pub duration: Duration,
    pub status: Result<(), String>,
}

/// Renders a structured ASCII table summarizing a batch conversion execution.
pub fn print_batch_summary(records: &[BatchItemRecord], total_elapsed: Duration) {
    if records.is_empty() {
        println!("No files processed.");
        return;
    }

    let mut max_name_len = "File".len();
    for r in records {
        if r.file_name.len() > max_name_len {
            max_name_len = r.file_name.len();
        }
    }
    max_name_len = max_name_len.min(45); // prevent excessive width

    println!();
    println!("Omni3MF Batch Conversion Summary");
    println!("{}", "=".repeat(max_name_len + 48));
    println!(
        "{:<width$} | {:>6} | {:>8} | {:>8} | {:>8} | {:<7}",
        "File", "Shells", "Vertices", "Faces", "Time(ms)", "Status",
        width = max_name_len
    );
    println!("{}", "-".repeat(max_name_len + 48));

    let mut total_success = 0;
    let mut total_failed = 0;
    let mut total_shells = 0;

    for r in records {
        let display_name = if r.file_name.len() > max_name_len {
            format!("...{}", &r.file_name[r.file_name.len() - (max_name_len - 3)..])
        } else {
            r.file_name.clone()
        };

        match &r.status {
            Ok(()) => {
                total_success += 1;
                total_shells += r.shells;
                println!(
                    "{:<width$} | {:>6} | {:>8} | {:>8} | {:>8.1} | OK",
                    display_name,
                    r.shells,
                    r.vertices,
                    r.faces,
                    r.duration.as_secs_f64() * 1000.0,
                    width = max_name_len
                );
            }
            Err(e) => {
                total_failed += 1;
                let short_err = if e.len() > 15 { &e[..15] } else { e.as_str() };
                println!(
                    "{:<width$} | {:>6} | {:>8} | {:>8} | {:>8.1} | ERR: {}",
                    display_name,
                    "-",
                    "-",
                    "-",
                    r.duration.as_secs_f64() * 1000.0,
                    short_err,
                    width = max_name_len
                );
            }
        }
    }

    println!("{}", "=".repeat(max_name_len + 48));
    println!(
        "Total: {} files | Succeeded: {} | Failed: {} | Shells: {} | Time: {:.2}s",
        records.len(),
        total_success,
        total_failed,
        total_shells,
        total_elapsed.as_secs_f64()
    );
    println!();
}
