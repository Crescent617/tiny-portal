use std::sync::Arc;

#[tokio::main]
async fn main() {
    env_logger::init();
    let p = Arc::new(tiny_portal::PortForwarder::new(
        "127.0.0.1:8887",
        "127.0.0.1:8888",
        tiny_portal::Protocol::UDP,
    ));

    let p1 = p.clone();
    tokio::spawn(async move {
        let r = p1.start_udp_echo_test_server().await;
        log::info!("Result: {:?}", r);
    });

    let r = p.start().await;
    log::info!("Result: {:?}", r);
}
