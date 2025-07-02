# flutter-wipe

A simple and fast command-line tool to clean up Flutter projects and reclaim disk space.

[![CI](https://github.com/AmanSikarwar/flutter-wipe/actions/workflows/ci.yml/badge.svg)](https://github.com/AmanSikarwar/flutter-wipe/actions/workflows/ci.yml)
[![Release](https://github.com/AmanSikarwar/flutter-wipe/actions/workflows/release.yml/badge.svg)](https://github.com/AmanSikarwar/flutter-wipe/actions/workflows/release.yml)

## What it does

`flutter-wipe` recursively scans a directory for Flutter projects and executes `flutter clean` in each of them. This removes the `build` directory, which can often grow very large, freeing up a significant amount of disk space.

## Installation

### Using the installer script (Linux/macOS)

You can install `flutter-wipe` using the following command:

```sh
curl -fsSL https://raw.githubusercontent.com/AmanSikarwar/flutter-wipe/main/install.sh | sh
```

This will download the latest release, and install it to `/usr/local/bin`.

### From source

You can also install `flutter-wipe` from source using Cargo:

```sh
cargo install --git https://github.com/AmanSikarwar/flutter-wipe
```

## Usage

Simply run `flutter-wipe` in the directory you want to clean:

```sh
flutter-wipe
```

Or, you can specify a directory to scan:

```sh
flutter-wipe --directory /path/to/your/projects
```

The tool also has a shorter alias, `fw`:

```sh
fw
```

### Additional Options

#### Exclude patterns

You can exclude specific directories from being scanned:

```sh
flutter-wipe --exclude "test-projects" --exclude "archived"
```

#### Configuration file

You can use a configuration file to set default exclude patterns and other options. The tool looks for config files in these locations (in order):

1. `flutter-wipe.toml` (current directory)
2. `flutter-wipe.config.toml` (current directory)
3. `~/.flutter-wipe.toml` (home directory)
4. `~/.config/flutter-wipe.toml` (config directory)

Or specify a custom config file:

```sh
flutter-wipe --config /path/to/config.toml
```

Example configuration file:

```toml
# Additional exclude patterns
exclude_patterns = [
    "custom-cache",
    "temp-flutter-projects",
    "archived-projects",
    "backup",
]

# Whether to use default excludes (true by default)
default_excludes = true
```

#### Default exclusions

By default, the tool excludes these directories:

- `.git` (Git repositories)
- `build` (Build artifacts)
- `node_modules` (Node.js dependencies)
- `.dart_tool` (Dart tools)
- `.pub-cache`, `pub-cache` (Pub cache)
- `flutter`, `flutter-sdk`, `.flutter` (Flutter SDK)
- `.mason_cache`, `.mason-cache`, `mason-cache` (Mason cache)
- Directories specified in `PUB_CACHE` and `FLUTTER_ROOT` environment variables
- Common Flutter SDK locations in home directory

You can disable default exclusions with:

```sh
flutter-wipe --no-default-excludes
```

## Building from source

To build `flutter-wipe` from source, you'll need the Rust toolchain installed.

1. Clone the repository:

    ```sh
    git clone https://github.com/AmanSikarwar/flutter-wipe.git
    ```

2. Build the project:

    ```sh
    cd flutter-wipe
    cargo build --release
    ```

The executable will be located in `target/release/flutter-wipe`.

## Contributing

Contributions are welcome! Please feel free to open an issue or submit a pull request.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
