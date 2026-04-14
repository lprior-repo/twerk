use twerk_infrastructure::datastore::inmemory::InMemoryDatastore;
use twerk_infrastructure::datastore::Datastore;

#[tokio::test]
async fn with_tx_accepts_async_closure_and_runs() {
    let ds = InMemoryDatastore::new();
    // New API: pass an async closure that can call async Datastore methods on the borrowed &dyn Datastore
    ds.with_tx(|inner| async move { inner.health_check().await })
        .await
        .expect("with_tx should run the closure and return Ok");
}

// Note: legacy shape used a boxed, object-safe callback value like:
//
//  ds.with_tx(Box::new(|ds: &dyn Datastore| {
//      Box::pin(async move { ds.health_check().await })
//  }))
//
// That form is intentionally no longer supported by the public API; the
// above test proves the migrated generic async-closure API works.
