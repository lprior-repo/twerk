use crate::api::{create_router, AppState, Config};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tower::make::Shared;
use twerk_core::id::NodeId;
use twerk_core::node::{Node, NodeStatus};
use twerk_infrastructure::broker::inmemory::InMemoryBroker;
use twerk_infrastructure::datastore::inmemory::InMemoryDatastore;
use twerk_infrastructure::datastore::Datastore;

pub struct TestServer {
    pub addr: SocketAddr,
    pub broker: Arc<InMemoryBroker>,
    pub datastore: Arc<InMemoryDatastore>,
    shutdown_tx: oneshot::Sender<()>,
}

impl TestServer {
    #[must_use]
    pub fn broker(&self) -> Arc<InMemoryBroker> {
        self.broker.clone()
    }

    #[must_use]
    pub fn datastore(&self) -> Arc<InMemoryDatastore> {
        self.datastore.clone()
    }

    pub async fn shutdown(self) {
        match self.shutdown_tx.send(()) {
            Ok(()) | Err(()) => {}
        }
    }
}

pub async fn start_test_server() -> Result<TestServer, std::io::Error> {
    let ds = Arc::new(InMemoryDatastore::new());
    let broker = Arc::new(InMemoryBroker::new());
    seed_test_worker(&ds).await?;
    let state = AppState::new(broker.clone(), ds.clone(), Config::default());
    let app = create_router(state);

    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

    tokio::spawn(async move {
        drop(
            axum::serve(listener, Shared::new(app))
                .with_graceful_shutdown(async {
                    drop(shutdown_rx.await);
                })
                .await,
        );
    });

    Ok(TestServer {
        addr,
        broker,
        datastore: ds,
        shutdown_tx,
    })
}

async fn seed_test_worker(ds: &Arc<InMemoryDatastore>) -> Result<(), std::io::Error> {
    ds.create_node(&Node {
        id: Some(NodeId::new("test-worker").map_err(std::io::Error::other)?),
        name: Some("test-worker".to_string()),
        last_heartbeat_at: Some(time::OffsetDateTime::now_utc()),
        queue: Some("default".to_string()),
        status: Some(NodeStatus::UP),
        ..Default::default()
    })
    .await
    .map_err(std::io::Error::other)
}

#[cfg(test)]
mod tests {
    use super::*;
    use twerk_infrastructure::broker::Broker;

    #[tokio::test]
    async fn server_starts_successfully() {
        let server = start_test_server().await.unwrap();
        assert!(server.addr.port() > 0);
        server.shutdown().await;
    }

    #[tokio::test]
    async fn server_accepts_requests() {
        let server = start_test_server().await.unwrap();
        let client = reqwest::Client::new();
        let response = client
            .get(format!("http://{}/health", server.addr))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), reqwest::StatusCode::OK);
        server.shutdown().await;
    }

    #[tokio::test]
    async fn server_broker_and_datastore_are_functional() {
        let server = start_test_server().await;
        assert!(server.is_ok());
        let server = server.unwrap();
        assert!(server.broker().health_check().await.is_ok());
        server.shutdown().await;
    }
}
