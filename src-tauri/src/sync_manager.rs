use matrix_sdk::Client;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;

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

        let handle = tokio::spawn(async move {
            println!("Starting Matrix sync loop...");

            let sync_settings = matrix_sdk::config::SyncSettings::default();

            loop {
                match client.sync_once(sync_settings.clone()).await {
                    Ok(response) => {
                        println!("Sync completed successfully, next batch: {}", response.next_batch);
                    },
                    Err(e) => {
                        eprintln!("Sync error: {:?}", e);
                        // Wait a bit before retrying on error
                        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                    }
                }
            }
        });

        let mut sync_guard = self.sync_handle.write().await;
        *sync_guard = Some(handle);
        println!("Sync task started");
    }

    /// Stop the current sync loop
    pub async fn stop_sync(&self) {
        let mut sync_guard = self.sync_handle.write().await;

        if let Some(handle) = sync_guard.take() {
            println!("Stopping sync task...");
            handle.abort();
            println!("Sync task stopped");
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
