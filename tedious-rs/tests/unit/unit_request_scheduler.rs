use tedious_rs::request_scheduler::RequestScheduler;

#[tokio::test]
async fn scheduler_queues_parallel_leases_and_releases_after_drop() {
    let scheduler = RequestScheduler::new();
    let lease = scheduler.acquire().await.unwrap();
    assert!(scheduler.is_active());
    let waiting_scheduler = scheduler.clone();
    let waiting = tokio::spawn(async move { waiting_scheduler.acquire().await.unwrap() });
    tokio::task::yield_now().await;
    assert!(!waiting.is_finished());
    drop(lease);
    let _next = waiting.await.unwrap();
}

#[tokio::test]
async fn scheduler_remains_available_after_a_failed_acquisition_path() {
    let scheduler = RequestScheduler::new();
    {
        let _lease = scheduler.acquire().await.unwrap();
    }
    assert!(!scheduler.is_active());
    let _lease = scheduler.acquire().await.unwrap();
    assert!(scheduler.is_active());
}

#[tokio::test]
async fn scheduler_serves_waiters_in_ticket_order() {
    let scheduler = RequestScheduler::new();
    let first = scheduler.acquire().await.unwrap();
    let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
    let sender_one = sender.clone();
    let scheduler_one = scheduler.clone();
    let first_waiter = tokio::spawn(async move {
        sender_one.send(1).unwrap();
        let lease = scheduler_one.acquire().await.unwrap();
        sender_one.send(3).unwrap();
        drop(lease);
    });
    assert_eq!(receiver.recv().await, Some(1));
    let sender_two = sender.clone();
    let scheduler_two = scheduler.clone();
    let second_waiter = tokio::spawn(async move {
        sender_two.send(2).unwrap();
        let lease = scheduler_two.acquire().await.unwrap();
        sender_two.send(4).unwrap();
        drop(lease);
    });
    assert_eq!(receiver.recv().await, Some(2));
    drop(first);
    assert_eq!(receiver.recv().await, Some(3));
    assert_eq!(receiver.recv().await, Some(4));
    first_waiter.await.unwrap();
    second_waiter.await.unwrap();
}

#[tokio::test]
async fn scheduler_removes_canceled_waiters() {
    let scheduler = RequestScheduler::new();
    let first = scheduler.acquire().await.unwrap();
    let waiting_scheduler = scheduler.clone();
    let waiting = tokio::spawn(async move { waiting_scheduler.acquire().await });
    tokio::task::yield_now().await;
    waiting.abort();
    assert!(waiting.await.unwrap_err().is_cancelled());
    drop(first);
    let next = tokio::time::timeout(std::time::Duration::from_secs(1), scheduler.acquire())
        .await
        .unwrap()
        .unwrap();
    drop(next);
}
