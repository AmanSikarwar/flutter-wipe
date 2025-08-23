# Changelog

All notable changes to this project will be documented in this file.

## [0.3.0]

### Features

- **Multi-threading support**: Parallel processing for scanning and cleaning operations
- **Thread control**: New `--threads` (`-j`) option to specify number of worker threads
- **Sequential mode**: New `--sequential` flag to disable parallel processing
- **Enhanced progress tracking**: Separate progress bars for scanning, size calculation, and cleaning phases
- **Configuration support**: New config options for `threads` and `sequential` settings
- **Performance optimizations**: Concurrent directory scanning and project cleaning

## [0.2.0]

### Features

- Configuration file support
- Exclude patterns functionality
- Default exclusion lists
- Improved CLI interface

## [0.1.0] - Initial Release

### Initial Features

- Basic Flutter project scanning
- Sequential cleaning operations
- Simple command-line interface
