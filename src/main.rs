use clap::Parser;
use colored::*;
use indicatif::{ProgressBar, ProgressStyle};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::thread;
use walkdir::WalkDir;

#[derive(Parser, Debug)]
#[command(name = "flutter-wipe", author, version, about, long_about = None, alias = "fw")]
struct Cli {
    /// The directory to scan for Flutter projects
    #[arg(short, long, value_name = "PATH", default_value = ".")]
    directory: PathBuf,
}

#[derive(Debug, Deserialize)]
struct Pubspec {
    dependencies: HashMap<String, serde_yaml::Value>,
}

fn main() {
    let cli = Cli::parse();
    let search_path = Arc::new(cli.directory);

    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"])
            .template("{spinner:.cyan} {msg}")
            .unwrap(),
    );
    spinner.set_message("Scanning for Flutter projects...".cyan().to_string());
    spinner.enable_steady_tick(std::time::Duration::from_millis(100));

    let search_path_clone = Arc::clone(&search_path);
    let find_projects_thread = thread::spawn(move || find_flutter_projects(&search_path_clone));

    let projects = find_projects_thread.join().unwrap();
    spinner.finish_with_message("Scan complete.".green().to_string());

    if projects.is_empty() {
        println!("{}", "No Flutter projects found.".yellow());
        return;
    }

    println!(
        "{}",
        format!("Found {} Flutter projects. Cleaning...", projects.len())
            .bold()
            .blue()
    );

    let mut total_reclaimed: u64 = 0;
    let mut cleaned_count = 0;

    for project_path in projects {
        println!(
            "
{}",
            project_path.display().to_string().bold().yellow()
        );

        let build_dir = project_path.join("build");
        let pre_clean_size = get_dir_size(&build_dir).unwrap_or(0);

        match Command::new("flutter")
            .arg("clean")
            .current_dir(&project_path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
        {
            Ok(output) => {
                if output.status.success() {
                    let freed_space_str = human_bytes::human_bytes(pre_clean_size as f64);
                    println!(
                        "  {} {} {}",
                        "✓ Cleaned".green(),
                        "Reclaimed:".cyan(),
                        freed_space_str.bright_blue()
                    );
                    total_reclaimed += pre_clean_size;
                    cleaned_count += 1;
                } else {
                    let error_msg = String::from_utf8_lossy(&output.stderr);
                    println!("  {} {}", "✗ Failed:".red(), error_msg.trim());
                }
            }
            Err(e) => {
                println!("  {} {}", "✗ Failed to execute command:".red(), e);
            }
        }
    }

    print_summary(cleaned_count, total_reclaimed);
}

fn find_flutter_projects(path: &Path) -> Vec<PathBuf> {
    WalkDir::new(path)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_dir() && e.file_name() != ".git" && e.file_name() != "build")
        .filter(|e| is_flutter_project(e.path()))
        .map(|e| e.path().to_path_buf())
        .collect()
}

fn is_flutter_project(path: &Path) -> bool {
    let pubspec_path = path.join("pubspec.yaml");
    if !pubspec_path.exists() {
        return false;
    }

    let mut file_content = String::new();
    if File::open(pubspec_path)
        .and_then(|mut f| f.read_to_string(&mut file_content))
        .is_err()
    {
        return false;
    }

    if let Ok(pubspec) = serde_yaml::from_str::<Pubspec>(&file_content) {
        return pubspec.dependencies.contains_key("flutter");
    }

    false
}

fn get_dir_size(path: &Path) -> Result<u64, fs_extra::error::Error> {
    if !path.exists() {
        return Ok(0);
    }
    fs_extra::dir::get_size(path)
}

fn print_summary(cleaned_count: u32, total_reclaimed: u64) {
    let total_reclaimed_str = human_bytes::human_bytes(total_reclaimed as f64);
    let summary_text = format!(
        "Processed {} projects. Total space reclaimed: {}",
        cleaned_count, total_reclaimed_str
    );

    println!(
        "
{}",
        "=".repeat(summary_text.len() + 4).green()
    );
    println!(
        "{} {} {}",
        "=".green(),
        summary_text.bold().green(),
        "=".green()
    );
    println!("{}", "=".repeat(summary_text.len() + 4).green());
}
