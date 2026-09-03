use crate::stability::StabilityConfig;
use crate::webhook::WebhookClientConfig;
use camino::Utf8PathBuf;
use clap::{Args as ClapArgs, Parser};
use std::time::Duration;

#[derive(Parser, Debug, Default)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    #[clap(flatten)]
    pub watcher: WatcherArgs,

    #[clap(flatten)]
    pub stabilizer: StabilizerArgs,

    #[clap(flatten)]
    pub webhook: WebhookArgs,
}

#[derive(ClapArgs, Debug, Clone)]
pub struct WatcherArgs {
    /// Path to watch for changes
    #[arg(env = "LYNCEUS_PATH")]
    pub path: Utf8PathBuf,

    /// Optional glob pattern relative to the watch path to filter created files (e.g. "**/*.txt")
    #[arg(short, long, env = "LYNCEUS_PATTERN")]
    pub pattern: Option<String>,

    /// Polling interval (e.g. 2s, 500ms)
    #[arg(
        short,
        long,
        env = "LYNCEUS_INTERVAL",
        default_value_t = humantime::Duration::from(Duration::from_secs(2))
    )]
    pub interval: humantime::Duration,

    /// Debounce duration (e.g. 5s, 10s)
    #[arg(
        short,
        long,
        env = "LYNCEUS_DEBOUNCE",
        default_value_t = humantime::Duration::from(Duration::from_secs(5))
    )]
    pub debounce: humantime::Duration,
}

#[derive(ClapArgs, Debug, Clone)]
pub struct StabilizerArgs {
    /// Cooldown interval for checking file stability (e.g. 10s, 30s)
    #[arg(
        short,
        long,
        env = "LYNCEUS_COOLDOWN",
        default_value_t = humantime::Duration::from(StabilityConfig::default().cooldown)
    )]
    pub cooldown: humantime::Duration,

    /// Number of consecutive stable checks required to consider the file created
    #[arg(
        short,
        long,
        env = "LYNCEUS_STABLE_COUNT",
        default_value_t = StabilityConfig::DEFAULT_STABLE_LIMIT
    )]
    pub stable_count: std::num::NonZeroUsize,

    /// Number of consecutive error checks before timing out/giving up on the file
    #[arg(
        short,
        long,
        env = "LYNCEUS_ERROR_COUNT",
        default_value_t = StabilityConfig::DEFAULT_ERROR_LIMIT
    )]
    pub error_count: std::num::NonZeroUsize,
}

#[derive(ClapArgs, Debug, Clone)]
pub struct WebhookArgs {
    /// Optional webhook URL to post a message to when a file is created
    #[arg(env = "LYNCEUS_WEBHOOK_URL")]
    pub webhook_url: Option<String>,

    /// Optional JSON template for the webhook payload. Supports `{{path}}`, `{{type}}`, and `{{timestamp}}` placeholders.
    #[arg(
        long,
        env = "LYNCEUS_WEBHOOK_TEMPLATE",
        value_parser = parse_json,
        default_value = WebhookClientConfig::DEFAULT_TEMPLATE
    )]
    pub webhook_template: serde_json::Value,

    /// Number of retries when sending a webhook fails
    #[arg(
        long,
        env = "LYNCEUS_WEBHOOK_RETRIES",
        default_value_t = WebhookClientConfig::DEFAULT_RETRIES
    )]
    pub webhook_retries: usize,
}

fn parse_json(s: &str) -> Result<serde_json::Value, String> {
    serde_json::from_str(s).map_err(|e| format!("invalid JSON: {}", e))
}

impl std::fmt::Display for WatcherArgs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "path={:?} interval={} debounce={}",
            self.path, self.interval, self.debounce
        )?;
        if let Some(ref pattern) = self.pattern {
            write!(f, " pattern={:?}", pattern)?;
        }
        Ok(())
    }
}

impl std::fmt::Display for StabilizerArgs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "cooldown={} stable_count={} error_count={}",
            self.cooldown, self.stable_count, self.error_count
        )
    }
}

impl std::fmt::Display for WebhookArgs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(ref webhook_url) = self.webhook_url {
            write!(
                f,
                "url={:?} retries={} template={}",
                webhook_url, self.webhook_retries, self.webhook_template
            )
        } else {
            write!(f, "None")
        }
    }
}

impl std::fmt::Display for Args {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "watcher={{{}}} stabilizer={{{}}} webhook={{{}}}",
            self.watcher, self.stabilizer, self.webhook
        )
    }
}

impl Default for WatcherArgs {
    fn default() -> Self {
        Self {
            path: Utf8PathBuf::new(),
            pattern: None,
            interval: humantime::Duration::from(Duration::from_secs(2)),
            debounce: humantime::Duration::from(Duration::from_secs(5)),
        }
    }
}

impl Default for StabilizerArgs {
    fn default() -> Self {
        Self {
            cooldown: humantime::Duration::from(StabilityConfig::default().cooldown),
            stable_count: StabilityConfig::DEFAULT_STABLE_LIMIT,
            error_count: StabilityConfig::DEFAULT_ERROR_LIMIT,
        }
    }
}

impl Default for WebhookArgs {
    fn default() -> Self {
        Self {
            webhook_url: None,
            webhook_template: serde_json::from_str(WebhookClientConfig::DEFAULT_TEMPLATE).unwrap(),
            webhook_retries: WebhookClientConfig::DEFAULT_RETRIES,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_args_display() {
        let args = Args {
            watcher: WatcherArgs {
                path: Utf8PathBuf::from("/tmp"),
                pattern: Some("**/*.rs".to_string()),
                ..Default::default()
            },
            stabilizer: StabilizerArgs {
                cooldown: humantime::Duration::from(std::time::Duration::from_secs(10)),
                ..Default::default()
            },
            webhook: WebhookArgs {
                webhook_url: Some("http://localhost".to_string()),
                webhook_template: serde_json::json!({"path": "{{path}}"}),
                ..Default::default()
            },
        };

        let formatted = format!("{}", args);
        assert_eq!(
            formatted,
            "watcher={path=\"/tmp\" interval=2s debounce=5s pattern=\"**/*.rs\"} stabilizer={cooldown=10s stable_count=3 error_count=5} webhook={url=\"http://localhost\" retries=3 template={\"path\":\"{{path}}\"}}"
        );
    }

    #[test]
    fn test_args_default() {
        let args = Args::default();
        assert_eq!(args.watcher.path, "");
        assert_eq!(args.watcher.pattern, None);
        assert_eq!(args.watcher.interval.as_secs(), 2);
        assert_eq!(args.watcher.debounce.as_secs(), 5);
        assert_eq!(args.stabilizer.cooldown.as_secs(), 10);
        assert_eq!(args.stabilizer.stable_count.get(), 3);
        assert_eq!(args.stabilizer.error_count.get(), 5);
        assert_eq!(args.webhook.webhook_url, None);
        assert_eq!(args.webhook.webhook_retries, 3);
    }
}
