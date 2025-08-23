use clap::Parser;
use colored::*;
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::env;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use walkdir::WalkDir;

#[derive(Parser, Debug)]
#[command(
    name = "flutter-wipe",
    author = "Aman Sikarwar <amansikarwar@gmail.com>",
    version,
    about = "A tool to clean Flutter projects",
    long_about = "This tool scans for Flutter projects in the specified directory and cleans them by removing build artifacts. It can be configured to exclude certain directories based on patterns.",
    alias = "fw"
)]
struct Cli {
    #[arg(short, long, value_name = "PATH", default_value = ".")]
    directory: PathBuf,

    #[arg(short, long = "exclude", value_name = "PATTERN")]
    exclude_patterns: Vec<String>,

    #[arg(long)]
    no_default_excludes: bool,

    #[arg(short, long, value_name = "CONFIG_FILE")]
    config: Option<PathBuf>,

    #[arg(short = 'j', long, value_name = "THREADS")]
    threads: Option<usize>,

    #[arg(long)]
    sequential: bool,
}

#[derive(Debug, Deserialize)]
struct Config {
    exclude_patterns: Option<Vec<String>>,
    default_excludes: Option<bool>,
    threads: Option<usize>,
    sequential: Option<bool>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            exclude_patterns: None,
            default_excludes: Some(true),
            threads: None,
            sequential: Some(false),
        }
    }
}

#[derive(Debug, Deserialize)]
struct Pubspec {
    dependencies: HashMap<String, serde_yaml::Value>,
}

#[derive(Debug, Clone)]
struct ProjectInfo {
    path: PathBuf,
    pre_clean_size: u64,
}

#[derive(Debug)]
struct CleanResult {
    project_path: PathBuf,
    success: bool,
    reclaimed_space: u64,
    error_message: Option<String>,
}

fn main() {
    let cli = Cli::parse();
    let config = load_config(&cli);
    let exclude_patterns = Arc::new(get_exclude_patterns(&cli, &config));
    let search_path = Arc::new(cli.directory.clone());

    let thread_count = determine_thread_count(&cli, &config);
    let use_sequential = should_use_sequential(&cli, &config);

    if !use_sequential {
        rayon::ThreadPoolBuilder::new()
            .num_threads(thread_count)
            .build_global()
            .expect("Failed to initialize thread pool");
    }

    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"])
            .template("{spinner:.cyan} {msg}")
            .unwrap(),
    );

    spinner.set_message("Scanning for Flutter projects...".cyan().to_string());
    spinner.enable_steady_tick(Duration::from_millis(100));

    let projects = if use_sequential {
        find_flutter_projects_sequential(&search_path, &exclude_patterns)
    } else {
        find_flutter_projects_parallel(&search_path, &exclude_patterns)
    };

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

    let project_infos = if use_sequential {
        calculate_sizes_sequential(&projects)
    } else {
        calculate_sizes_parallel(&projects)
    };

    let results = if use_sequential {
        clean_projects_sequential(&project_infos)
    } else {
        clean_projects_parallel(&project_infos)
    };

    let mut total_reclaimed: u64 = 0;
    let mut cleaned_count = 0;

    for result in results {
        println!(
            "{}",
            result.project_path.display().to_string().bold().yellow()
        );

        if result.success {
            let freed_space_str = human_bytes::human_bytes(result.reclaimed_space as f64);
            println!(
                "  {} {} {}",
                "✓ Cleaned".green(),
                "Reclaimed:".cyan(),
                freed_space_str.bright_blue()
            );
            total_reclaimed += result.reclaimed_space;
            cleaned_count += 1;
        } else {
            let error_msg = result
                .error_message
                .unwrap_or_else(|| "Unknown error".to_string());
            println!("  {} {}", "✗ Failed:".red(), error_msg.trim());
        }
    }

    print_summary(cleaned_count, total_reclaimed);
}

fn determine_thread_count(cli: &Cli, config: &Config) -> usize {
    if let Some(threads) = cli.threads {
        threads
    } else if let Some(threads) = config.threads {
        threads
    } else {
        num_cpus::get()
    }
}

fn should_use_sequential(cli: &Cli, config: &Config) -> bool {
    cli.sequential || config.sequential.unwrap_or(false)
}

fn find_flutter_projects_sequential(
    path: &Path,
    exclude_patterns: &HashSet<String>,
) -> Vec<PathBuf> {
    WalkDir::new(path)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_dir())
        .filter(|e| !should_exclude_directory(e.path(), exclude_patterns))
        .filter(|e| is_flutter_project(e.path()))
        .map(|e| e.path().to_path_buf())
        .collect()
}

fn find_flutter_projects_parallel(path: &Path, exclude_patterns: &HashSet<String>) -> Vec<PathBuf> {
    let walker = WalkDir::new(path)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_dir())
        .collect::<Vec<_>>();

    walker
        .into_par_iter()
        .filter(|e| !should_exclude_directory(e.path(), exclude_patterns))
        .filter(|e| is_flutter_project(e.path()))
        .map(|e| e.path().to_path_buf())
        .collect()
}

fn calculate_sizes_sequential(projects: &[PathBuf]) -> Vec<ProjectInfo> {
    let progress = ProgressBar::new(projects.len() as u64);
    progress.set_style(
        ProgressStyle::default_bar()
            .template("{bar:40.green/blue} {pos:>7}/{len:7} {msg}")
            .unwrap()
            .progress_chars("##-"),
    );
    progress.set_message("Calculating project sizes...");

    let results: Vec<ProjectInfo> = projects
        .iter()
        .map(|project_path| {
            let build_dir = project_path.join("build");
            let pre_clean_size = get_dir_size(&build_dir).unwrap_or(0);
            progress.inc(1);
            ProjectInfo {
                path: project_path.clone(),
                pre_clean_size,
            }
        })
        .collect();

    progress.finish_with_message("Size calculation complete.");
    results
}

fn calculate_sizes_parallel(projects: &[PathBuf]) -> Vec<ProjectInfo> {
    let progress = ProgressBar::new(projects.len() as u64);
    progress.set_style(
        ProgressStyle::default_bar()
            .template("{bar:40.green/blue} {pos:>7}/{len:7} {msg}")
            .unwrap()
            .progress_chars("##-"),
    );
    progress.set_message("Calculating project sizes in parallel...");

    let results: Vec<ProjectInfo> = projects
        .par_iter()
        .map(|project_path| {
            let build_dir = project_path.join("build");
            let pre_clean_size = get_dir_size(&build_dir).unwrap_or(0);
            progress.inc(1);
            ProjectInfo {
                path: project_path.clone(),
                pre_clean_size,
            }
        })
        .collect();

    progress.finish_with_message("Size calculation complete.");
    results
}

fn clean_projects_sequential(project_infos: &[ProjectInfo]) -> Vec<CleanResult> {
    let progress = ProgressBar::new(project_infos.len() as u64);
    progress.set_style(
        ProgressStyle::default_bar()
            .template("{bar:40.cyan/blue} {pos:>7}/{len:7} {msg}")
            .unwrap()
            .progress_chars("##-"),
    );
    progress.set_message("Cleaning projects...");

    let results: Vec<CleanResult> = project_infos
        .iter()
        .map(|info| {
            let result = clean_single_project(info);
            progress.inc(1);
            result
        })
        .collect();

    progress.finish_with_message("Cleaning complete.");
    results
}

fn clean_projects_parallel(project_infos: &[ProjectInfo]) -> Vec<CleanResult> {
    let progress = ProgressBar::new(project_infos.len() as u64);
    progress.set_style(
        ProgressStyle::default_bar()
            .template("{bar:40.cyan/blue} {pos:>7}/{len:7} {msg}")
            .unwrap()
            .progress_chars("##-"),
    );
    progress.set_message("Cleaning projects...");

    let results: Vec<CleanResult> = project_infos
        .par_iter()
        .map(|info| {
            let result = clean_single_project(info);
            progress.inc(1);
            result
        })
        .collect();

    progress.finish_with_message("Cleaning complete.");
    results
}

fn clean_single_project(project_info: &ProjectInfo) -> CleanResult {
    use std::process::{Command, Stdio};

    match Command::new("flutter")
        .arg("clean")
        .current_dir(&project_info.path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
    {
        Ok(output) => {
            if output.status.success() {
                CleanResult {
                    project_path: project_info.path.clone(),
                    success: true,
                    reclaimed_space: project_info.pre_clean_size,
                    error_message: None,
                }
            } else {
                let error_msg = String::from_utf8_lossy(&output.stderr).to_string();
                CleanResult {
                    project_path: project_info.path.clone(),
                    success: false,
                    reclaimed_space: 0,
                    error_message: Some(error_msg),
                }
            }
        }
        Err(e) => CleanResult {
            project_path: project_info.path.clone(),
            success: false,
            reclaimed_space: 0,
            error_message: Some(format!("Failed to execute command: {e}")),
        },
    }
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
    let summary_text =
        format!("Processed {cleaned_count} projects. Total space reclaimed: {total_reclaimed_str}");

    println!("{}", "=".repeat(summary_text.len() + 4).green());
    println!(
        "{} {} {}",
        "=".green(),
        summary_text.bold().green(),
        "=".green()
    );
    println!("{}", "=".repeat(summary_text.len() + 4).green());
}

fn get_exclude_patterns(cli: &Cli, config: &Config) -> HashSet<String> {
    let mut patterns = HashSet::new();

    for pattern in &cli.exclude_patterns {
        patterns.insert(pattern.clone());
    }

    if let Some(config_patterns) = &config.exclude_patterns {
        for pattern in config_patterns {
            patterns.insert(pattern.clone());
        }
    }

    let use_defaults = if cli.no_default_excludes {
        false
    } else {
        config.default_excludes.unwrap_or(true)
    };

    if use_defaults {
        patterns.insert(".git".to_string());
        patterns.insert("build".to_string());
        patterns.insert("node_modules".to_string());
        patterns.insert(".dart_tool".to_string());

        patterns.insert(".pub-cache".to_string());
        patterns.insert("pub-cache".to_string());

        patterns.insert("flutter".to_string());
        patterns.insert("flutter-sdk".to_string());
        patterns.insert(".flutter".to_string());

        patterns.insert(".mason_cache".to_string());
        patterns.insert(".mason-cache".to_string());
        patterns.insert("mason-cache".to_string());

        if let Ok(pub_cache) = env::var("PUB_CACHE") {
            if let Some(path) = PathBuf::from(pub_cache).file_name() {
                if let Some(name) = path.to_str() {
                    patterns.insert(name.to_string());
                }
            }
        }

        if let Ok(mason_cache) = env::var("MASON_CACHE") {
            if let Some(path) = PathBuf::from(mason_cache).file_name() {
                if let Some(name) = path.to_str() {
                    patterns.insert(name.to_string());
                }
            }
        }

        if let Ok(flutter_root) = env::var("FLUTTER_ROOT") {
            if let Some(path) = PathBuf::from(flutter_root).file_name() {
                if let Some(name) = path.to_str() {
                    patterns.insert(name.to_string());
                }
            }
        }

        if let Ok(home) = env::var("HOME") {
            let home_path = PathBuf::from(home);

            for flutter_dir in &[
                "flutter",
                ".flutter",
                "development/flutter",
                "Developer/flutter",
            ] {
                let flutter_path = home_path.join(flutter_dir);
                if flutter_path.exists() {
                    if let Some(name) = flutter_path.file_name() {
                        if let Some(name_str) = name.to_str() {
                            patterns.insert(name_str.to_string());
                        }
                    }
                }
            }
        }
    }

    patterns
}

fn load_config(cli: &Cli) -> Config {
    let config_path = if let Some(config_file) = &cli.config {
        config_file.clone()
    } else {
        let mut candidates = vec![
            PathBuf::from("flutter-wipe.toml"),
            PathBuf::from("flutter-wipe.config.toml"),
        ];

        if let Ok(home) = env::var("HOME") {
            let home_path = PathBuf::from(home);
            candidates.push(home_path.join(".flutter-wipe.toml"));
            candidates.push(home_path.join(".config/flutter-wipe.toml"));
        }

        candidates
            .into_iter()
            .find(|p| p.exists())
            .unwrap_or_default()
    };

    if config_path.exists() {
        match std::fs::read_to_string(&config_path) {
            Ok(content) => match toml::from_str::<Config>(&content) {
                Ok(config) => {
                    println!("Loaded config from: {}", config_path.display());
                    return config;
                }
                Err(e) => {
                    eprintln!(
                        "Warning: Failed to parse config file {}: {}",
                        config_path.display(),
                        e
                    );
                }
            },
            Err(e) => {
                eprintln!(
                    "Warning: Failed to read config file {}: {}",
                    config_path.display(),
                    e
                );
            }
        }
    }

    Config::default()
}

fn should_exclude_directory(path: &Path, exclude_patterns: &HashSet<String>) -> bool {
    if let Some(path_str) = path.to_str() {
        if path_str.contains(".mason-cache") || path_str.contains(".mason_cache") {
            return true;
        }
    }

    if let Some(dir_name) = path.file_name() {
        if let Some(name_str) = dir_name.to_str() {
            if exclude_patterns.contains(name_str) {
                return true;
            }

            for pattern in exclude_patterns {
                if name_str.contains(pattern) || pattern.contains(name_str) {
                    return true;
                }
            }
        }
    }

    if let Some(path_str) = path.to_str() {
        for pattern in exclude_patterns {
            if path_str.contains(pattern) {
                return true;
            }
        }
    }

    false
}
