#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let argv: Vec<String> = std::env::args().collect();
    let addr = &argv[1];
    tiny_portal::util::start_udp_echo_server(addr).await
}
