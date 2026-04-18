use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::oneshot;

use twerk_infrastructure::broker::inmemory::InMemoryBroker;
use twerk_infrastructure::broker::Broker;
use twerk_infrastructure::datastore::inmemory::InMemoryDatastore;
use twerk_web::api::{create_router, AppState, Config};

pub struct TestServer {
    pub server_url: String,
    pub shutdown_tx: oneshot::Sender<()>,
}

impl TestServer {
    pub async fn shutdown(self) {
        let _ = self.shutdown_tx.send(());
    }
}

pub async fn start_test_server() -> Result<TestServer, anyhow::Error> {
    let ds = Arc::new(InMemoryDatastore::new());
    let broker = Arc::new(InMemoryBroker::new()) as Arc<dyn Broker>;
    let state = AppState::new(broker, ds, Config::default());
    let app = create_router(state);

    let listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0))).await?;
    let addr = listener.local_addr()?;
    let port = addr.port();

    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

    let server = axum::serve(listener, app);

    tokio::spawn(async move {
        tokio::select! {
            result = server => {
                if let Err(e) = result {
                    tracing::error!("test server error: {}", e);
                }
            }
            _ = shutdown_rx => {
                tracing::debug!("test server received shutdown signal");
            }
        }
    });

    let server_url = format!("http://127.0.0.1:{port}");

    Ok(TestServer {
        server_url,
        shutdown_tx,
    })
}
