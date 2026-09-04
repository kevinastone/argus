# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.5.3](https://github.com/kevinastone/lynceus/compare/v0.5.2...v0.5.3) - 2026-09-04

### Added

- *(webhook)* add custom webhook header support

### Other

- *(args)* implement Default for Args structures
- add decoupled in-process e2e integration tests

## [0.5.2](https://github.com/kevinastone/lynceus/compare/v0.5.1...v0.5.2) - 2026-06-18

### Other

- add 30-second request timeout
- include file size in file created webhook event
- run file check asynchronously using tokio::fs::metadata
- *(readme)* remove custom styling from mermaid diagram

## [0.5.1](https://github.com/kevinastone/lynceus/compare/v0.5.0...v0.5.1) - 2026-06-08

### Other

- simplify constant initialization and clean up watcher

## [0.5.0](https://github.com/kevinastone/lynceus/compare/v0.4.4...v0.5.0) - 2026-06-07

### Added

- log file size once a file has stabilized

### Other

- *(args)* [**breaking**] remove short and long command-line flags for webhook URL
- remove .cargo from .gitignore
- *(deps)* bump Cargo.toml dependencies to latest stable versions
- replace custom test_helpers with camino-tempfile

## [0.4.4](https://github.com/kevinastone/lynceus/compare/v0.4.3...v0.4.4) - 2026-06-06

### Added

- add short and long flags for webhook URL

### Other

- improve tracing context by using spans for file processing
- adopt camino for UTF-8 path safety and simplify path resolution
- split makeImage and expose cross-compiled binary packages
- *(nix)* disable checks in commonArgs
- *(nix)* simplify cross-compilation arguments in flake.nix
- *(nix)* use standard system names and optimize makeImage
- use RELEASE_PLZ_TOKEN for release-plz jobs

## [0.4.3](https://github.com/kevinastone/lynceus/compare/v0.4.2...v0.4.3) - 2026-06-05

### Other

- optimize multi-architecture container builds using cross-compilation
- pass registry repository name as argument to push script
- build and push multi-architecture container images using qemu and regctl
- *(nix)* simplify Docker image build and environment setup
- Drop gc and partial matching caches for cache-nix-action
- add processing pipeline mermaid diagram to README
- Add cargo registry token secret reference

## [0.4.2](https://github.com/kevinastone/lynceus/compare/v0.4.1...v0.4.2) - 2026-05-29

### Added

- restructure CLI arguments into logical subgroups and implement custom Display logging

### Other

- introduce WebhookClientConfig and implement From<&WebhookArgs>
- add gc-max-store-size-linux setting to nix store cache

## [0.4.1](https://github.com/kevinastone/lynceus/compare/v0.4.0...v0.4.1) - 2026-05-29

### Added

- implement graceful SIGTERM handling and immediate watcher shutdown

## [0.3.0](https://github.com/kevinastone/lynceus/compare/v0.2.0...v0.3.0) - 2026-05-28

### Added

- introduce modular Events type with shared timestamp and clean encapsulation

### Fixed

- *(events)* fix clippy ptr-arg warning in path serialization
- *(flake)* Add missing rust src to the devShell
- *(nix)* use fromTOML instead of builtins.fromTOML

### Other

- run release-plz workflow only on successful test runs
- add release-plz configuration enabling git-only mode
- use private Event::new to default timestamp initialization
- *(webhooks)* [**breaking**] Replace default webhook with webhook-standard payload
- *(flake)* dynamically resolve RUST_SRC_PATH from craneLib
- Update changelog to show v0.2.0 release
## [v0.2.0]

### 🚀 Features

- Add optional positional WEBHOOK_URL argument and notify on file stability
- Add successful webhook logging message
- Implement robust, graceful shutdown with TaskTracker select and safety timeout
- Add glob pattern support to the watch path with robust filtering
- Add braces { and } to special glob character set for watch path splitting
- Replace standard glob with wax for elegant, robust path partitioning
- Split watch path glob into --pattern flag, and refine stream & webhook engines
- Swap back from wax to glob crate
- Add openssl and pkg-config to naersk build inputs, and include cacert in container
- Add webhook payload templating option (--webhook-template)
- Integrate liquid-json for webhook payload templating
- Add configurable webhook retries with exponential backoff using reqwest-retry
- Integrate reqwest-tracing middleware to WebhookClient
- Set minimum retry interval to 10s for webhook client

### 🐛 Bug Fixes

- Resolve clippy warning by replacing option map with clean if-let syntax
- Resolve dockerTools deprecation warning and update nixpkgs

### 💼 Other

- Add CARGO_HTTP_USER_AGENT to commonArgs and devShell
- Override naersk fetchurl to inject a compliant User-Agent
- Add comment explaining customFetchurl for crates.io 403s
- Use space-free User-Agent in curlOpts to avoid bash word splitting
- Include lynceus in image copyToRoot to fix missing shared libraries
- Explicitly add openssl and set LD_LIBRARY_PATH in container to fix runtime library resolution
- Add coreutils to container image to provide standard shell commands
- Explicitly set pathsToLink in buildEnv to link /bin and /etc
- Package openssl.out instead of default bin output in the container image
- Refactor build to use standard rustPlatform and parse Cargo.toml for package metadata
- Use global fromTOML instead of builtins prefix
- Switch globbing crate to wax to support brace expansion patterns
- Log original pattern string instead of internal Glob representation
- Switch globbing backend to fast-glob for high-performance brace expansion
- Migrate flake.nix to crane build system
- Set SSL_CERT_FILE in crane commonArgs to fix sandboxed cert loading

### 🚜 Refactor

- Introduce FileStabilizer struct for stability checks
- Simplify argument and field names by dropping _interval and _duration suffixes
- Replace futures::stream::unfold with tokio_stream::wrappers::UnboundedReceiverStream
- Remove compare contents configuration from debouncer
- Rename _debouncer to debouncer
- Extract webhook client into a dedicated module
- Extract clap CLI arguments configuration to a dedicated args module
- Use inspect_ok and inspect_err stream combinators to flatten the main event processing loop
- Convert event loop into a fully chained, fluent Stream::for_each pipeline
- Use Option::zip and map to handle webhook notification without nested if-let blocks
- Simplify and robustify base watch path extraction from glob patterns
- Switch webhook to JSON payload using reqwest .json()
- Encapsulate polling and debounced watch logic in a new watcher module
- *(watcher)* Separate RawDirectoryWatcher from pattern matching stream filtering

### 📚 Documentation

- Document WEBHOOK_URL positional argument in README.md
- Create AGENTS.md with developer guidelines and nix fmt formatting guidance
- Update README with recent features, CLI changes, and Nix checks

### ⚡ Performance

- Query file metadata immediately on the first stability iteration to avoid initial delay

### 🎨 Styling

- Reformat codebase using nix fmt
- Log event stream finishing as an error indicating unexpected termination

### 🧪 Testing

- Simplify webhook retry integration tests using mockito
- Add test case verifying 0 retries does exactly 1 attempt
- *(stability)* Add unit tests for byte formatting and file stabilizer heuristics
- *(stability)* Refactor unit tests to control virtual time using tokio test-util
- *(watcher)* Use standard tokio tests for OS directory watching
- Extract redundant TempDir test helper into a shared test_helpers module

### ⚙️ Miscellaneous Tasks

- Add CI/CD workflows for stable/beta tests and Nix OCI container image delivery to GHCR
- Migrate test workflow to run nix flake check
- Add cargo check, clippy, test, and treefmt checks to nix flake check
- *(nix)* Use crane's devShell integration
- Extract container image labels dynamically from Cargo.toml
- Add cargo-release to devShell packages
- Prevent publishing to crates.io with publish = false
- *(ci)* Added release-plz for versioned release management
