# Lynceus

Lynceus is a lightweight, high-performance directory file watcher written in Rust. It is specifically designed to reliably monitor directories on **network shares (SMB/CIFS mounts)** where native OS filesystem events (like `inotify` or `FSEvents`) are either unavailable or fail to report events correctly.

Lynceus tracks newly created files and ensures that writing/copying processes have **completely finished** before reporting the file as created.

## Features

- **Network-Share Optimized**: Uses poll-based directory watching with content hashing/comparison to guarantee event capture across network mounts.
- **Concurrent Cooldown Checks**: Multiplexes up to 100 concurrent stability checks using an extremely lightweight Tokio async stream architecture. It automatically waits for large files to finish copying without blocking other detections.
- **Glob Pattern Filtering**: Filter detected files by standard glob patterns (e.g. `**/*.mp4`).
- **Flexible Webhook Customization**: Define custom JSON payloads utilizing the rich **Liquid template engine** syntax (supporting filters like `upcase`, `split`, and `last`).
- **Resilient Delivery**: Dispatches webhook notifications asynchronously and automatically retries transient failures with **exponential backoff**.
- **Fully Nix-Integrated**: Minimal OCI container images, formatting via `nix fmt` (using `treefmt`), and comprehensive CI checking.

---

## Three-Stage Processing Pipeline

Lynceus processes file system events sequentially through three highly optimized stages:

```mermaid
graph TD
    %% Watcher Section
    subgraph Watcher ["1. Directory Watcher & Filter"]
        A["Target Directory (SMB/CIFS)"] -->|Interval-based Polling| B["Detect Changed/New Files"]
        B --> C{"Glob Pattern Filter?"}
        C -->|Does Not Match| D["Ignore Event"]
        C -->|Matches| E["Forward to Stream"]
    end

    %% Stabilizer Section
    subgraph Stabilizer ["2. File Stabilizer (Concurrent)"]
        E --> F["Wait Queue (.buffer_unordered)"]
        F --> G["Initiate Cooldown Timer"]
        G --> H["Check Size & Mod Time"]
        H --> I{"Stable over Cooldown?"}
        I -->|No / Modifying| G
        I -->|Error limit exceeded| K["Discard (Timeout)"]
        I -->|Yes, stable-count reached| L["Emit 'File Created' Event"]
    end

    %% Webhook Section
    subgraph Webhook ["3. Webhook Dispatcher (Non-Blocking)"]
        L --> M["Format Payload (Liquid Templates)"]
        M --> N["Spawn Async Task (TaskTracker)"]
        N --> O["POST HTTP request"]
        O --> P{"Response Success?"}
        P -->|No & Retries Available| Q["Exponential Backoff"] --> N
        P -->|No & Max Retries Exceeded| R["Log Error & Drop"]
        P -->|Yes| S["Success (Log Status)"]
    end
```

### 1. Directory Watcher & Filter
- **Interval Polling**: Regularly scans the target watch directory to detect new files, handling network shares where native event APIs (e.g. `inotify`, `FSEvents`) fall short.
- **Debouncing**: Debounces incoming events to prevent overwhelming the processing queue.
- **Pattern Matching**: Applies glob filters (e.g., `--pattern "**/*.txt"`) early on to immediately discard irrelevant files.

### 2. File Stabilizer
- **Concurrent Monitoring**: Uses an async queue (`.buffer_unordered(100)`) to concurrently track up to 100 files simultaneously without blocking other detections.
- **Stability Heuristic**: Measures file size and modification times across successive `cooldown` intervals. A file is declared fully written only after surviving `stable-count` consecutive checks without size or modification changes.
- **Resilience**: If a file check fails continuously (e.g., locked file, network drop), it automatically times out and drops the event after `error-count` consecutive errors.

### 3. Webhook Dispatcher
- **Liquid Rendering**: Generates flexible JSON payloads dynamically using the Liquid template engine based on the `--webhook-template` option.
- **Non-blocking Dispatch**: Dispatches events asynchronously via background tasks (`tokio::spawn`), isolated from the main watching loop.
- **Exponential Backoff**: Resilient HTTP POST delivery that retries transient failures with progressive backoff.
- **Graceful Shutdown**: Integrates with a `TaskTracker` to guarantee pending webhooks are fully drained upon termination.

---

## Installation

Ensure you have Rust installed (MSRV 1.85+), then clone the repository and build the binary:

```bash
cargo build --release
```

---

## Usage

Run the compiled binary by passing the target directory path as a positional argument:

```bash
./target/release/lynceus /path/to/watch
```

### Configuration Options

```text
Usage: lynceus [OPTIONS] <PATH> [WEBHOOK_URL]

Arguments:
  <PATH>         Path to watch for changes [env: LYNCEUS_PATH=]
  [WEBHOOK_URL]  Optional webhook URL to post a message to when a file is created [env: LYNCEUS_WEBHOOK_URL=]

Options:
  -p, --pattern <PATTERN>
          Optional glob pattern relative to the watch path to filter created files (e.g. "**/*.txt") [env: LYNCEUS_PATTERN=]
      --webhook-template <WEBHOOK_TEMPLATE>
          Optional JSON template for the webhook payload. Supports `{{path}}`, `{{type}}`, and `{{timestamp}}` placeholders [env: LYNCEUS_WEBHOOK_TEMPLATE=] [default: {"type":"{{type}}","timestamp":"{{timestamp}}","path":"{{path}}"}]
      --webhook-retries <WEBHOOK_RETRIES>
          Number of retries when sending a webhook fails [env: LYNCEUS_WEBHOOK_RETRIES=] [default: 3]
  -H, --webhook-header <WEBHOOK_HEADER>
          Optional HTTP header(s) to include with the webhook request (e.g. "Authorization: Bearer <token>"). Can be specified multiple times [env: LYNCEUS_WEBHOOK_HEADER=]
  -i, --interval <INTERVAL>
          Polling interval (e.g. 2s, 500ms) [env: LYNCEUS_INTERVAL=] [default: 2s]
  -d, --debounce <DEBOUNCE>
          Debounce duration (e.g. 5s, 10s) [env: LYNCEUS_DEBOUNCE=] [default: 5s]
  -c, --cooldown <COOLDOWN>
          Cooldown interval for checking file stability (e.g. 10s, 30s) [env: LYNCEUS_COOLDOWN=] [default: 10s]
  -s, --stable-count <STABLE_COUNT>
          Number of consecutive stable checks required to consider the file created [env: LYNCEUS_STABLE_COUNT=] [default: 3]
  -e, --error-count <ERROR_COUNT>
          Number of consecutive error checks before timing out/giving up on the file [env: LYNCEUS_ERROR_COUNT=] [default: 5]
  -h, --help
          Print help
  -V, --version
          Print version
```

---

## Examples & Common Configurations

### 1. Glob Pattern Filtering
If you only care about specific files (e.g., media files), you can filter incoming events using glob syntax:

```bash
cargo run --release -- /path/to/watch --pattern "**/*.{mp4,mkv}"
```

### 2. Robust Network Copy Detection
For network copies (e.g. copying huge files over a slow SMB share), we want a longer stability cooldown. You can run Lynceus to poll every 5 seconds, debounce events for 15 seconds, and check file stability every 10 seconds:

```bash
cargo run --release -- /path/to/watch --interval 5s --debounce 15s --cooldown 10s
```

### 3. Customizable Webhook Notifications
You can specify an optional Discord/Slack or generic HTTP endpoint webhook. Webhooks are dispatched in the background and do not block the primary watcher loop.

#### Liquid-syntax Template Engine
Using the `--webhook-template` flag (or `LYNCEUS_WEBHOOK_TEMPLATE` env var), you can customize the JSON payload. Lynceus supports standard Liquid tags and filters.

* **Placeholders**:
  * `{{path}}`: Relative path of the created file.
  * `{{type}}`: Event type (e.g. `"file.created"`).
  * `{{timestamp}}`: Event timestamp in RFC 3339 format (e.g. `"2026-05-28T15:02:50Z"`).
* **Liquid Filters**: Extract filenames or transform text using filters (e.g., `{{path | split: '/' | last}}` extracts only the filename).

**Example (Slack-compatible webhook message)**:
```bash
cargo run --release -- /path/to/watch https://hooks.slack.com/services/... \
  --webhook-template '{"text": "New file created: {{path | split: '\''/'\'' | last}} at {{path}}"}'
```

#### Custom HTTP Headers
Pass authentication or metadata headers using `-H` / `--webhook-header` (or `LYNCEUS_WEBHOOK_HEADER` env var). Can be specified multiple times:

```bash
cargo run --release -- /path/to/watch https://api.example.com/webhooks \
  -H "Authorization: Bearer my-secret-token" \
  -H "X-Source: lynceus"
```

#### Transient Failure Retries
Transient network or server errors are automatically retried using an exponential backoff policy (defaults to 3 retries) before declaring failure.

---

## Logging & Diagnostics

Lynceus supports the standard `RUST_LOG` environment variable to configure logging levels. 

### Standard Info logs (Default)
```bash
$ cargo run -- /path/to/watch
2026-05-27T08:00:00Z  INFO lynceus: Starting Lynceus args=Args { path: "/path/to/watch", pattern: None, webhook_url: None, webhook_template: Object {"path": String("{{path}}"), "timestamp": String("{{timestamp}}"), "type": String("{{type}}")}, webhook_retries: 3, interval: Duration(2s), debounce: Duration(5s), cooldown: Duration(10s), stable_count: 3, error_count: 5 }
2026-05-27T08:00:00Z  INFO lynceus: Watching for new files watch_path="/path/to/watch"
2026-05-27T08:00:35Z  INFO lynceus: File created path="video.mp4"
```

### Detailed Debug logs
To see real-time stability polling ticks:
```bash
$ RUST_LOG=debug cargo run -- /path/to/watch
2026-05-27T08:00:05Z DEBUG lynceus: New file detected, waiting for write to complete path="video.mp4"
2026-05-27T08:00:15Z DEBUG lynceus: File is stable path="video.mp4" size="10.00 MiB" stable_count=1
2026-05-27T08:00:25Z DEBUG lynceus: File is stable path="video.mp4" size="10.00 MiB" stable_count=2
2026-05-27T08:00:35Z DEBUG lynceus: File is stable path="video.mp4" size="10.00 MiB" stable_count=3
2026-05-27T08:00:35Z  INFO lynceus: File created path="video.mp4"
```

---

## CI/CD & Development

Lynceus leverages **Nix** for fully reproducible formatting, builds, and sandbox checks:

- **Format Code**: Formats Rust (via `rustfmt`) and Nix code globally.
  ```bash
  nix fmt
  ```
- **Flake Checks**: Validates formatting (treefmt), cargo check, clippy analysis (`--deny warnings`), and the cargo test suite in a hermetic environment.
  ```bash
  nix flake check
  ```
- **Minimal OCI Container**: Builds a minimal, highly secure OCI/Docker container.
  ```bash
  nix build .#image
  ```
