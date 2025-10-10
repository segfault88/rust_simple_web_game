use tokio::signal;
use tokio_util::sync::CancellationToken;
use tracing::info;

pub async fn shutdown_signal(cancel: CancellationToken) {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    tokio::select! {
        _ = ctrl_c => {
            info!("Ctrl+C received, cancelling...");
            cancel.cancel();
        },
        _ = cancel.cancelled() => {},
    }
}
