use nexus_runtime_core::cognition::checkpointer::{SqliteCheckpointer, StateCheckpoint};
use sqlx::SqlitePool;
use uuid::Uuid;

#[tokio::test]
async fn test_branched_time_travel_integration() -> anyhow::Result<()> {
    let pool = SqlitePool::connect("sqlite::memory:").await?;
    let checkpointer = SqliteCheckpointer::new(pool);
    checkpointer.ensure_checkpoints_table().await?;

    let parent_id = Uuid::new_v4();
    let parent_checkpoint = StateCheckpoint {
        checkpoint_id: parent_id,
        parent_checkpoint_id: None,
        plan_id: "plan_1".to_string(),
        task_id: "task_1".to_string(),
        vmo_handles: vec![100, 101],
        channel_values: serde_json::json!({ "key": "parent_value" }),
        timestamp: 1000,
    };

    checkpointer.put(parent_checkpoint.clone()).await?;

    let child_a_id = Uuid::new_v4();
    let child_a = StateCheckpoint {
        checkpoint_id: child_a_id,
        parent_checkpoint_id: Some(parent_id),
        plan_id: "plan_1".to_string(),
        task_id: "task_2a".to_string(),
        vmo_handles: vec![100, 102],
        channel_values: serde_json::json!({ "key": "child_a_value" }),
        timestamp: 1005,
    };

    let child_b_id = Uuid::new_v4();
    let child_b = StateCheckpoint {
        checkpoint_id: child_b_id,
        parent_checkpoint_id: Some(parent_id),
        plan_id: "plan_1".to_string(),
        task_id: "task_2b".to_string(),
        vmo_handles: vec![100, 103],
        channel_values: serde_json::json!({ "key": "child_b_value" }),
        timestamp: 1006,
    };

    checkpointer.put(child_a.clone()).await?;
    checkpointer.put(child_b.clone()).await?;

    let fetched_parent = checkpointer.get(parent_id).await?.expect("parent must exist");
    assert_eq!(fetched_parent.checkpoint_id, parent_id);
    assert_eq!(fetched_parent.channel_values["key"], "parent_value");
    assert_eq!(fetched_parent.vmo_handles, vec![100, 101]);

    let fetched_a = checkpointer.get(child_a_id).await?.expect("child_a must exist");
    assert_eq!(fetched_a.parent_checkpoint_id, Some(parent_id));
    assert_eq!(fetched_a.channel_values["key"], "child_a_value");
    assert_eq!(fetched_a.vmo_handles, vec![100, 102]);

    let fetched_b = checkpointer.get(child_b_id).await?.expect("child_b must exist");
    assert_eq!(fetched_b.parent_checkpoint_id, Some(parent_id));
    assert_eq!(fetched_b.channel_values["key"], "child_b_value");
    assert_eq!(fetched_b.vmo_handles, vec![100, 103]);

    Ok(())
}
