use std::time::Duration;

use tedious_rs::sender::send_in_parallel;

#[tokio::test]
async fn udp_parallel_failure_returns_the_individual_error() {
    let result = send_in_parallel(
        &[
            "127.0.0.1:0".parse().unwrap(),
            "127.0.0.1:0".parse().unwrap(),
        ],
        9,
        b"request",
        Duration::from_millis(20),
    )
    .await;

    assert_eq!(result.unwrap_err(), "UDP response timeout");
}
