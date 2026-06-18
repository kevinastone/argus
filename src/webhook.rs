use anyhow::Context;
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_retry::{RetryTransientMiddleware, policies::ExponentialBackoff};
use tokio_util::task::TaskTracker;
use tracing::Instrument;

use crate::args::WebhookArgs;
use crate::events::Event;

#[derive(Clone, Debug)]
pub struct WebhookClientConfig {
    pub template: serde_json::Value,
    pub retries: usize,
    pub min_backoff: std::time::Duration,
}

impl WebhookClientConfig {
    pub const DEFAULT_TEMPLATE: &'static str =
        r#"{"type":"{{type}}","timestamp":"{{timestamp}}","path":"{{path}}"}"#;
    pub const DEFAULT_RETRIES: usize = 3;
}

impl Default for WebhookClientConfig {
    fn default() -> Self {
        Self {
            template: serde_json::from_str(Self::DEFAULT_TEMPLATE).unwrap(),
            retries: Self::DEFAULT_RETRIES,
            min_backoff: std::time::Duration::from_secs(10),
        }
    }
}

impl From<&WebhookArgs> for WebhookClientConfig {
    fn from(args: &WebhookArgs) -> Self {
        Self {
            template: args.webhook_template.clone(),
            retries: args.webhook_retries,
            ..Self::default()
        }
    }
}

#[derive(Clone)]
pub struct WebhookClient {
    client: ClientWithMiddleware,
    url: String,
    template: liquid_json::LiquidJson,
    tracker: TaskTracker,
}

impl WebhookClient {
    pub fn new(url: String, config: WebhookClientConfig, tracker: TaskTracker) -> Self {
        let retry_policy = ExponentialBackoff::builder()
            .retry_bounds(config.min_backoff, std::time::Duration::from_secs(300))
            .build_with_max_retries(config.retries as u32);
        let client = ClientBuilder::new(reqwest::Client::new())
            .with(reqwest_tracing::TracingMiddleware::default())
            .with(RetryTransientMiddleware::new_with_policy(retry_policy))
            .build();

        Self {
            client,
            url,
            template: liquid_json::LiquidJson::new(config.template),
            tracker,
        }
    }

    /// Dispatches a non-blocking webhook POST notification about a created file.
    pub fn send_notification(&self, event: Event) {
        let client = self.client.clone();
        let url = self.url.clone();
        let tmpl = self.template.clone();
        let span = tracing::Span::current();

        self.tracker.spawn(
            async move {
                let res = async {
                    let data = serde_json::to_value(&event)
                        .map_err(|e| anyhow::anyhow!("Failed to serialize event: {}", e))?;
                    let payload = tmpl
                        .render(&data)
                        .map_err(|e| anyhow::anyhow!("Failed to render liquid template: {}", e))?;

                    let resp = client
                        .post(&url)
                        .json(&payload)
                        .send()
                        .await
                        .with_context(|| format!("Failed to send HTTP POST request to {}", url))?;

                    if !resp.status().is_success() {
                        anyhow::bail!("Webhook server returned status {}", resp.status());
                    }

                    Ok::<(), anyhow::Error>(())
                }
                .await;

                match res {
                    Ok(_) => {
                        tracing::info!(
                            url = %url,
                            ?event,
                            "Webhook notification sent successfully"
                        );
                    }
                    Err(e) => {
                        tracing::error!(
                            error = ?e,
                            url = %url,
                            ?event,
                            "Failed to send webhook notification"
                        );
                    }
                }
            }
            .instrument(span),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use camino::Utf8Path;
    use serde_json::json;

    #[test]
    fn test_render_template() {
        let template = json!({
            "event_upper": "{{type | upcase}}",
            "file_path": "{{path}}",
            "filename": "{{path | split: '/' | last}}",
            "nested": {
                "key": "value_{{type}}"
            },
            "array": ["{{path}}", 42, true, null]
        });

        let tmpl = liquid_json::LiquidJson::new(template.clone());
        let data = json!({
            "path": "dir/file.txt",
            "type": "file.created"
        });
        let rendered = tmpl.render(&data).unwrap();

        let expected = json!({
            "event_upper": "FILE.CREATED",
            "file_path": "dir/file.txt",
            "filename": "file.txt",
            "nested": {
                "key": "value_file.created"
            },
            "array": ["dir/file.txt", 42, true, null]
        });

        assert_eq!(rendered, expected);
    }

    #[tokio::test]
    async fn test_webhook_retry_success() {
        let mut server = mockito::Server::new_async().await;

        let mock_fail1 = server
            .mock("POST", "/")
            .with_status(500)
            .expect(1)
            .create_async()
            .await;

        let mock_fail2 = server
            .mock("POST", "/")
            .with_status(500)
            .expect(1)
            .create_async()
            .await;

        let mock_success = server
            .mock("POST", "/")
            .with_status(200)
            .expect(1)
            .create_async()
            .await;

        let tracker = TaskTracker::new();
        let client = WebhookClient::new(
            server.url(),
            WebhookClientConfig {
                template: json!({"path": "{{path}}"}),
                retries: 2,
                min_backoff: std::time::Duration::from_millis(1),
            },
            tracker.clone(),
        );

        client.send_notification(Event::file_created(
            Utf8Path::new("test.txt").to_path_buf(),
            0,
        ));

        // Wait for webhook to finish
        tracker.close();
        let finished = tokio::select! {
            _ = tracker.wait() => true,
            _ = tokio::time::sleep(std::time::Duration::from_secs(5)) => false,
        };

        assert!(
            finished,
            "Webhook notification took too long or failed to complete"
        );

        mock_fail1.assert_async().await;
        mock_fail2.assert_async().await;
        mock_success.assert_async().await;
    }

    #[tokio::test]
    async fn test_webhook_retry_failure() {
        let mut server = mockito::Server::new_async().await;

        let mock_fail1 = server
            .mock("POST", "/")
            .with_status(500)
            .expect(1)
            .create_async()
            .await;

        let mock_fail2 = server
            .mock("POST", "/")
            .with_status(500)
            .expect(1)
            .create_async()
            .await;

        let tracker = TaskTracker::new();
        let client = WebhookClient::new(
            server.url(),
            WebhookClientConfig {
                template: json!({"path": "{{path}}"}),
                retries: 1,
                min_backoff: std::time::Duration::from_millis(1),
            },
            tracker.clone(),
        );

        client.send_notification(Event::file_created(
            Utf8Path::new("test.txt").to_path_buf(),
            0,
        ));

        tracker.close();
        let finished = tokio::select! {
            _ = tracker.wait() => true,
            _ = tokio::time::sleep(std::time::Duration::from_secs(5)) => false,
        };

        assert!(finished, "Webhook notification took too long to fail");

        mock_fail1.assert_async().await;
        mock_fail2.assert_async().await;
    }

    #[tokio::test]
    async fn test_webhook_no_retries() {
        let mut server = mockito::Server::new_async().await;

        let mock_fail = server
            .mock("POST", "/")
            .with_status(500)
            .expect(1)
            .create_async()
            .await;

        let tracker = TaskTracker::new();
        let client = WebhookClient::new(
            server.url(),
            WebhookClientConfig {
                template: json!({"path": "{{path}}"}),
                retries: 0,
                min_backoff: std::time::Duration::from_millis(1),
            },
            tracker.clone(),
        );

        client.send_notification(Event::file_created(
            Utf8Path::new("test.txt").to_path_buf(),
            0,
        ));

        tracker.close();
        let finished = tokio::select! {
            _ = tracker.wait() => true,
            _ = tokio::time::sleep(std::time::Duration::from_secs(5)) => false,
        };

        assert!(finished, "Webhook notification took too long to fail");

        mock_fail.assert_async().await;
    }

    #[test]
    fn test_webhook_client_config_from_webhook_args() {
        let args = WebhookArgs {
            webhook_url: Some("http://example.com".to_string()),
            webhook_template: json!({"my_path": "{{path}}"}),
            webhook_retries: 5,
        };
        let config = WebhookClientConfig::from(&args);
        assert_eq!(config.template, json!({"my_path": "{{path}}"}));
        assert_eq!(config.retries, 5);
        assert_eq!(config.min_backoff, std::time::Duration::from_secs(10));
    }
}
