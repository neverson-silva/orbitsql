use std::time::Duration;

use tedious_rs::bulk_load::{BulkLoad, BulkLoadOptions};
use tedious_rs::packet::packet_type;
use tedious_rs::transport::MessageIo;
use tokio::io::AsyncReadExt;
use tokio::sync::oneshot;

#[tokio::test]
async fn cancellation_does_not_interrupt_a_pending_message_write() {
    let (client, mut server) = tokio::io::duplex(1);
    let mut io = MessageIo::new(client, 16).unwrap();
    let cancellation = BulkLoad::new("table", BulkLoadOptions::default())
        .unwrap()
        .cancellation_handle();
    let cancellation_for_task = cancellation.clone();
    let (started_sender, started_receiver) = oneshot::channel();
    let parts = futures_util::stream::once(async move {
        started_sender.send(()).unwrap();
        Ok(vec![1, 2, 3, 4, 5, 6, 7, 8])
    });
    let mut write = tokio::spawn(async move {
        io.send_message_stream_with_cancellation(
            packet_type::BULK_LOAD,
            parts,
            &cancellation_for_task,
        )
        .await
    });

    started_receiver.await.unwrap();
    tokio::task::yield_now().await;
    cancellation.cancel();
    assert!(
        tokio::time::timeout(Duration::from_millis(20), &mut write)
            .await
            .is_err()
    );

    let mut wire = [0; 16];
    server.read_exact(&mut wire).await.unwrap();
    assert!(!write.await.unwrap().unwrap());
}
