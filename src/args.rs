use crate::stability::StabilityConfig;
use crate::webhook::WebhookClientConfig;
use camino::Utf8PathBuf;
use clap::{Args as ClapArgs, Parser};
use reqwest::header::{HeaderName, HeaderValue};
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

    /// Optional HTTP header(s) to include with the webhook request (e.g. "Authorization: Bearer <token>"). Can be specified multiple times.
    #[arg(
        short = 'H',
        long = "webhook-header",
        env = "LYNCEUS_WEBHOOK_HEADER",
        value_parser = parse_header
    )]
    pub webhook_headers: Vec<(HeaderName, HeaderValue)>,
}

fn parse_json(s: &str) -> Result<serde_json::Value, String> {
    serde_json::from_str(s).map_err(|e| format!("invalid JSON: {}", e))
}

fn parse_header(s: &str) -> Result<(HeaderName, HeaderValue), String> {
    let (key, value) = s
        .split_once(':')
        .ok_or_else(|| format!("invalid header '{}', expected 'Key: Value'", s))?;
    let key = HeaderName::from_bytes(key.trim().as_bytes())
        .map_err(|e| format!("invalid header name '{}': {}", key.trim(), e))?;
    let value = HeaderValue::from_str(value.trim())
        .map_err(|e| format!("invalid header value '{}': {}", value.trim(), e))?;
    Ok((key, value))
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
            )?;
            if !self.webhook_headers.is_empty() {
                let headers_str = self
                    .webhook_headers
                    .iter()
                    .map(|(k, v)| format!("{}: {:?}", k.as_str(), v.to_str().unwrap_or("<binary>")))
                    .collect::<Vec<_>>()
                    .join(", ");
                write!(f, " headers=[{}]", headers_str)?;
            }
            Ok(())
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
            webhook_headers: Vec::new(),
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
        assert!(args.webhook.webhook_headers.is_empty());
    }

    #[test]
    fn test_webhook_args_display_with_headers() {
        let args = WebhookArgs {
            webhook_url: Some("http://localhost".to_string()),
            webhook_template: serde_json::json!({"path": "{{path}}"}),
            webhook_headers: vec![
                (
                    HeaderName::from_static("authorization"),
                    HeaderValue::from_static("Bearer token123"),
                ),
                (
                    HeaderName::from_static("x-custom"),
                    HeaderValue::from_static("custom-value"),
                ),
            ],
            ..Default::default()
        };

        let formatted = format!("{}", args);
        assert_eq!(
            formatted,
            "url=\"http://localhost\" retries=3 template={\"path\":\"{{path}}\"} headers=[authorization: \"Bearer token123\", x-custom: \"custom-value\"]"
        );
    }

    #[test]
    fn test_parse_header_valid() {
        let (name, value) = parse_header("Authorization: Bearer my-secret-token").unwrap();
        assert_eq!(name, HeaderName::from_static("authorization"));
        assert_eq!(value, HeaderValue::from_static("Bearer my-secret-token"));

        let (name2, value2) = parse_header("X-Custom:value_without_leading_space").unwrap();
        assert_eq!(name2, HeaderName::from_static("x-custom"));
        assert_eq!(
            value2,
            HeaderValue::from_static("value_without_leading_space")
        );
    }

    #[test]
    fn test_parse_header_invalid() {
        assert!(parse_header("NoColonHere").is_err());
        assert!(parse_header("Invalid Header Name: value").is_err());
    }

    #[test]
    fn test_parse_multiple_headers_cli() {
        let args = Args::try_parse_from([
            "lynceus",
            "/tmp",
            "-H",
            "Authorization: Bearer token123",
            "--webhook-header",
            "Accept: text/html, application/xhtml+xml, application/xml;q=0.9",
        ])
        .unwrap();

        assert_eq!(args.webhook.webhook_headers.len(), 2);
        assert_eq!(
            args.webhook.webhook_headers[0].0,
            HeaderName::from_static("authorization")
        );
        assert_eq!(
            args.webhook.webhook_headers[0].1,
            HeaderValue::from_static("Bearer token123")
        );
        assert_eq!(
            args.webhook.webhook_headers[1].0,
            HeaderName::from_static("accept")
        );
        assert_eq!(
            args.webhook.webhook_headers[1].1,
            HeaderValue::from_static("text/html, application/xhtml+xml, application/xml;q=0.9")
        );
    }
}
