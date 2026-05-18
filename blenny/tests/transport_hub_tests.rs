use blenny::transport::TransportHub;

#[tokio::test]
async fn global_broadcast_reaches_subscriber() {
    let hub = TransportHub::new();
    let mut rx = hub.subscribe();

    hub.broadcast_html("<p>hello</p>");

    let msg = rx.recv().await.unwrap();
    assert_eq!(msg.category, "ui");
    assert_eq!(msg.html.unwrap(), "<p>hello</p>");
}

#[tokio::test]
async fn topic_pub_sub_works() {
    let hub = TransportHub::new();
    let mut rx = hub.subscribe_topic("test.topic")
        .expect("Failed to subscribe to topic");

    hub.publish("test.topic", "hello".into());

    let msg = rx.recv().await.unwrap();
    assert_eq!(msg, "hello");
}

#[tokio::test]
async fn topic_created_on_subscribe() {
    let hub = TransportHub::new();
    let mut rx = hub.subscribe_topic("new.topic")
        .expect("Failed to subscribe to topic");
    // Must be able to publish and receive even though publish never called before
    hub.publish("new.topic", "first".into());
    let msg = rx.recv().await.unwrap();
    assert_eq!(msg, "first");
}

#[tokio::test]
async fn topic_multiple_subscribers_all_receive() {
    let hub = TransportHub::new();
    let mut rx1 = hub.subscribe_topic("multi")
        .expect("Failed to subscribe to topic");
    let mut rx2 = hub.subscribe_topic("multi")
        .expect("Failed to subscribe to topic");

    hub.publish("multi", "broadcast".into());

    let msg1 = rx1.recv().await.unwrap();
    let msg2 = rx2.recv().await.unwrap();
    assert_eq!(msg1, "broadcast");
    assert_eq!(msg2, "broadcast");
}

#[tokio::test]
async fn global_broadcast_only_goes_to_global_subscribers() {
    let hub = TransportHub::new();
    let mut global_rx = hub.subscribe();
    let mut topic_rx = hub.subscribe_topic("some.topic");

    hub.broadcast_html("global");
    hub.publish("some.topic", "topic".into());

    let global_msg = global_rx.recv().await.unwrap();
    assert_eq!(global_msg.html.unwrap(), "global");

    let topic_msg = topic_rx.recv().await.unwrap();
    assert_eq!(topic_msg, "topic");

    // After receiving the topic message, global subscriber should not have it
    assert!(global_rx.try_recv().is_err());
}
