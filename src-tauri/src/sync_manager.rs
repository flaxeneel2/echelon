use matrix_sdk::Client;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use tracing::{debug, error};

pub struct SyncManager {
    sync_handle: RwLock<Option<JoinHandle<()>>>,
}

impl SyncManager {
    pub fn new() -> Self {
        Self {
            sync_handle: RwLock::new(None),
        }
    }

    /// Start the sync loop for a given Matrix client
    pub async fn start_sync(&self, client: Client) {
        // Stop any existing sync first
        self.stop_sync().await;

        debug!("Performing initial sync...");
        let sync_settings = matrix_sdk::config::SyncSettings::default();
        match client.sync_once(sync_settings.clone()).await {
            Ok(_) => {
                debug!("Initial sync successful");
            }
            Err(e) => {
                error!("Initial Sync failed: {}", e);
            }
        };

        let handle = tokio::spawn(async move {
            debug!("Starting Matrix sync loop...");

            if let Err(e) = client.sync(sync_settings).await {
                error!("Sync ended with error: {:?}", e);
            }
        });

        let mut sync_guard = self.sync_handle.write().await;
        *sync_guard = Some(handle);
        debug!("Sync task started");
    }

    /// Stop the current sync loop
    pub async fn stop_sync(&self) {
        let mut sync_guard = self.sync_handle.write().await;

        if let Some(handle) = sync_guard.take() {
            debug!("Stopping sync task...");
            handle.abort();
            debug!("Sync task stopped");
        }
    }

    /// Check if sync is currently running
    pub async fn is_syncing(&self) -> bool {
        let sync_guard = self.sync_handle.read().await;
        sync_guard.is_some()
    }
}

impl Drop for SyncManager {
    fn drop(&mut self) {
        // Try to stop sync when the manager is dropped
        if let Some(handle) = self.sync_handle.try_write().ok().and_then(|mut guard| guard.take()) {
            handle.abort();
        }
    }
}
