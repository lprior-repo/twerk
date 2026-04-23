mod basics;
pub(super) mod black_box_support {
    use tokio::task::JoinSet;

    pub(super) use super::super::support::{
        assert_health_up, assert_metrics, assert_node_entry, direct_task, job_with_state, node,
        TestHarness, TestResponse,
    };

    pub(super) async fn repeated_gets(
        harness: &TestHarness,
        uri: &'static str,
        count: usize,
    ) -> Vec<TestResponse> {
        let mut set = JoinSet::new();

        (0..count).for_each(|_| {
            let harness = harness.clone();
            set.spawn(async move { harness.get(uri).await });
        });

        set.join_all().await
    }
}
mod concurrency;
mod performance;
