// blenny/tests/transport_hub_tests.rs
use blenny::transport::TransportHub;

#[test]
fn global_broadcast_reaches_subscriber() {
    let hub = TransportHub::new();
    let mut rx = hub.subscribe();

    hub.broadcast_html("<p>hello</p>");

    let msg = rx.try_recv().expect("should receive global message");
    assert_eq!(msg.category, "ui");
    assert_eq!(msg.html.unwrap(), "<p>hello</p>");
}

#[test]
fn broadcast_data_uses_data_category() {
    let hub = TransportHub::new();
    let mut rx = hub.subscribe();

    hub.broadcast_data(r#"{"key":"value"}"#);

    let msg = rx.try_recv().expect("should receive data message");
    assert_eq!(msg.category, "data");
    assert_eq!(msg.signals.unwrap(), r#"{"key":"value"}"#);
}

#[test]
fn topic_pub_sub_works() {
    let hub = TransportHub::new();

    let mut rx = hub.subscribe_topic("test.topic").expect("subscribe");
    hub.publish("test.topic", "hello".into()).expect("publish");

    let msg = rx.try_recv().expect("should receive topic message");
    assert_eq!(msg, "hello");
}

#[test]
fn user_registration_and_direct_message() {
    let hub = TransportHub::new();

    let (mut rx, _handle) = hub.register_user("alice").expect("register");
    hub.direct_html_to_user("alice", "<p>Private</p>");

    let msg = rx.try_recv().expect("should receive direct message");
    assert_eq!(msg.html.unwrap(), "<p>Private</p>");
}

#[test]
fn unregister_removes_user() {
    let hub = TransportHub::new();

    let (mut rx, handle) = hub.register_user("bob").expect("register");
    drop(handle); // triggers unregister

    hub.direct_html_to_user("bob", "<p>Lost</p>");

    // The message should not arrive because the user was unregistered
    assert!(rx.try_recv().is_err());
}

#[test]
fn multiple_connections_per_user() {
    let hub = TransportHub::new();

    let (mut rx1, h1) = hub.register_user("carol").expect("register");
    let (mut rx2, _h2) = hub.register_user("carol").expect("register");

    hub.direct_html_to_user("carol", "<p>Hi</p>");

    let msg1 = rx1.try_recv().expect("rx1 should get message");
    let msg2 = rx2.try_recv().expect("rx2 should get message");
    assert_eq!(msg1.html, msg2.html);

    // Drop one handle; the other connection should still work
    drop(h1);
    hub.direct_html_to_user("carol", "<p>Still there</p>");
    let msg = rx2.try_recv().expect("rx2 should still receive");
    assert_eq!(msg.html.unwrap(), "<p>Still there</p>");
}
