#[path = "support/appstate_commit_contract.rs"]
mod contract;

#[tokio::test]
async fn memory_patch_commit_preserves_order_and_collection_scope() {
    contract::assert_patch_commit_contract(&wacore::store::InMemoryBackend::new()).await;
}
