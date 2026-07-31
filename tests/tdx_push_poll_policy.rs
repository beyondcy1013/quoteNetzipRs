use netzipapi_rust_demo::tdx_push_poll_policy::{PollAction, PushPollPolicy};

#[test]
fn healthy_push_uses_only_the_low_frequency_full_audit() {
    let mut policy = PushPollPolicy::new(30_000);

    assert_eq!(policy.take_due(29_999), Vec::<PollAction>::new());
    assert_eq!(policy.take_due(30_000), vec![PollAction::FullAudit]);
    assert_eq!(policy.take_due(59_999), Vec::<PollAction>::new());
    assert_eq!(policy.take_due(60_000), vec![PollAction::FullAudit]);
}

#[test]
fn failed_shard_is_polled_immediately_without_polling_healthy_shards() {
    let mut policy = PushPollPolicy::new(30_000);
    policy.mark_shard_failed(7, 12_000);

    assert_eq!(
        policy.take_due(12_000),
        vec![PollAction::RecoverShard { shard: 7 }]
    );
    assert_eq!(policy.take_due(12_999), Vec::<PollAction>::new());
    assert_eq!(
        policy.take_due(13_000),
        vec![PollAction::RecoverShard { shard: 7 }]
    );
}

#[test]
fn recovered_shard_stops_local_polling_but_keeps_full_audit_schedule() {
    let mut policy = PushPollPolicy::new(30_000);
    policy.mark_shard_failed(3, 5_000);
    assert_eq!(
        policy.take_due(5_000),
        vec![PollAction::RecoverShard { shard: 3 }]
    );

    policy.mark_shard_healthy(3);
    assert_eq!(policy.take_due(6_000), Vec::<PollAction>::new());
    assert_eq!(policy.take_due(30_000), vec![PollAction::FullAudit]);
}

#[test]
fn recovery_polling_is_bounded_and_fair_across_failed_shards() {
    let mut policy = PushPollPolicy::new(30_000);
    for shard in 0..3 {
        policy.mark_shard_failed(shard, 1_000);
    }

    assert_eq!(
        policy.take_due_bounded(1_000, 2),
        vec![
            PollAction::RecoverShard { shard: 0 },
            PollAction::RecoverShard { shard: 1 },
        ]
    );
    assert_eq!(
        policy.take_due_bounded(1_000, 2),
        vec![PollAction::RecoverShard { shard: 2 }]
    );
}
