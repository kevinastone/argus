use crate::args::{Args, StabilizerArgs, WatcherArgs, WebhookArgs};
use crate::run;
use camino_tempfile::Builder;
use serde_json::json;
use std::fs;
use std::num::NonZeroUsize;
use std::time::Duration;
use tokio::sync::oneshot;

#[tokio::test]
async fn test_run_e2e_file_creation_to_webhook() {
    let mut server = mockito::Server::new_async().await;

    let mock = server
        .mock("POST", "/webhook")
        .match_header("content-type", "application/json")
        .match_body(mockito::Matcher::PartialJson(json!({
            "event_type": "file.created",
            "file": "sub/report.txt"
        })))
        .with_status(200)
        .expect(1)
        .create_async()
        .await;

    let temp_dir = Builder::new().prefix("e2e_pipeline").tempdir().unwrap();

    let args = Args {
        watcher: WatcherArgs {
            path: temp_dir.path().to_path_buf(),
            pattern: Some("**/*.txt".to_string()),
            interval: humantime::Duration::from(Duration::from_millis(5)),
            debounce: humantime::Duration::from(Duration::from_millis(10)),
        },
        stabilizer: StabilizerArgs {
            cooldown: humantime::Duration::from(Duration::from_millis(15)),
            stable_count: NonZeroUsize::new(2).unwrap(),
            error_count: NonZeroUsize::new(3).unwrap(),
        },
        webhook: WebhookArgs {
            webhook_url: Some(format!("{}/webhook", server.url())),
            webhook_template: json!({
                "event_type": "{{type}}",
                "file": "{{path}}",
                "time": "{{timestamp}}"
            }),
            webhook_retries: 2,
        },
    };

    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let runner_handle = tokio::spawn(async move {
        run(args, async {
            let _ = shutdown_rx.await;
        })
        .await
    });

    // Give watcher time to initialize
    tokio::time::sleep(Duration::from_millis(20)).await;

    // Create subfolder and matching file
    let sub_dir = temp_dir.path().join("sub");
    fs::create_dir_all(&sub_dir).unwrap();
    let match_file = sub_dir.join("report.txt");
    fs::write(&match_file, b"initial content").unwrap();

    // Create non-matching file
    fs::write(temp_dir.path().join("image.png"), b"png_data").unwrap();

    // Wait for mockito to receive the webhook
    let completed = tokio::select! {
        _ = async {
            loop {
                if mock.matched_async().await {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        } => true,
        _ = tokio::time::sleep(Duration::from_secs(3)) => false,
    };
    assert!(completed, "Timed out waiting for webhook to receive event");

    // Initiate graceful shutdown
    shutdown_tx
        .send(())
        .expect("failed to send shutdown signal");
    let run_res = runner_handle.await.expect("runner task panicked");
    assert!(run_res.is_ok(), "run() returned error: {:?}", run_res);

    mock.assert_async().await;
}

#[tokio::test]
async fn test_run_e2e_ignored_files_do_not_notify() {
    let mut server = mockito::Server::new_async().await;

    let mock = server
        .mock("POST", "/webhook")
        .with_status(200)
        .expect(0)
        .create_async()
        .await;

    let temp_dir = Builder::new().prefix("e2e_ignored").tempdir().unwrap();

    let args = Args {
        watcher: WatcherArgs {
            path: temp_dir.path().to_path_buf(),
            pattern: Some("**/*.csv".to_string()),
            interval: humantime::Duration::from(Duration::from_millis(5)),
            debounce: humantime::Duration::from(Duration::from_millis(10)),
        },
        stabilizer: StabilizerArgs {
            cooldown: humantime::Duration::from(Duration::from_millis(15)),
            stable_count: NonZeroUsize::new(2).unwrap(),
            error_count: NonZeroUsize::new(3).unwrap(),
        },
        webhook: WebhookArgs {
            webhook_url: Some(format!("{}/webhook", server.url())),
            webhook_template: json!({"path": "{{path}}"}),
            webhook_retries: 1,
        },
    };

    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let runner_handle = tokio::spawn(async move {
        run(args, async {
            let _ = shutdown_rx.await;
        })
        .await
    });

    tokio::time::sleep(Duration::from_millis(20)).await;

    // Create non-matching files
    fs::write(temp_dir.path().join("test.txt"), b"some text").unwrap();
    fs::write(temp_dir.path().join("data.json"), b"{}").unwrap();

    tokio::time::sleep(Duration::from_millis(150)).await;

    shutdown_tx
        .send(())
        .expect("failed to send shutdown signal");
    let run_res = runner_handle.await.expect("runner task panicked");
    assert!(run_res.is_ok(), "run() returned error: {:?}", run_res);

    mock.assert_async().await;
}
