use clap::Parser;
use colored::*;
use indicatif::{ProgressBar, ProgressStyle};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::env;
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
    #[arg(short, long, value_name = "PATH", default_value = ".")]
    directory: PathBuf,

    #[arg(short, long = "exclude", value_name = "PATTERN")]
    exclude_patterns: Vec<String>,

    #[arg(long)]
    no_default_excludes: bool,

    #[arg(short, long, value_name = "CONFIG_FILE")]
    config: Option<PathBuf>,
}

#[derive(Debug, Deserialize)]
struct Config {
    exclude_patterns: Option<Vec<String>>,
    default_excludes: Option<bool>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            exclude_patterns: None,
            default_excludes: Some(true),
        }
    }
}

#[derive(Debug, Deserialize)]
struct Pubspec {
    dependencies: HashMap<String, serde_yaml::Value>,
}

fn main() {
    let cli = Cli::parse();
    let config = load_config(&cli);
    let exclude_patterns = Arc::new(get_exclude_patterns(&cli, &config));
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
    let exclude_patterns_clone = Arc::clone(&exclude_patterns);
    let find_projects_thread =
        thread::spawn(move || find_flutter_projects(&search_path_clone, &exclude_patterns_clone));

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
        println!("{}", project_path.display().to_string().bold().yellow());

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

fn find_flutter_projects(path: &Path, exclude_patterns: &HashSet<String>) -> Vec<PathBuf> {
    WalkDir::new(path)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_dir())
        .filter(|e| !should_exclude_directory(e.path(), exclude_patterns))
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
