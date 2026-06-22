use std::sync::Arc;
use std::time::Duration;

use crate::{AppConfig, ArticleVerifier, Store};

const INTERVAL: Duration = Duration::from_secs(7 * 24 * 60 * 60);

pub fn spawn_revalidation(
    config: AppConfig,
    store: Store,
    verifier: Arc<dyn ArticleVerifier>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(INTERVAL);
        loop {
            interval.tick().await;
            for (tag, url) in store.all_tag_entries() {
                let tag_url = format!("https://{}/declare/{tag}", config.server_host());
                if !verifier.verify_backlink(&url, &tag_url).await
                    && let Err(error) = store.remove_entry(&tag, &url)
                {
                    eprintln!("failed to remove {url} from #{tag}: {error}");
                }
            }
        }
    })
}
