mod tcp;
pub use tcp::*;
mod udp;
pub use udp::*;

pub trait Portal {
    async fn start(&self) -> anyhow::Result<()>;
    fn status(&self) -> String;
}

pub mod util {
    use std::sync::Arc;

    use tokio::net::UdpSocket;

    pub async fn start_udp_echo_server(addr: &str) -> anyhow::Result<()> {
        let sock = Arc::new(UdpSocket::bind(addr).await?);
        let mut b1 = [0u8; super::udp::MAX_UDP_PACKET_SIZE];

        loop {
            let sock = sock.clone();

            match sock.recv_from(&mut b1).await {
                Ok((size, client_addr)) => {
                    log::debug!("Received {} bytes from {}", size, client_addr);
                    sock.send_to(&b1[..size], client_addr).await?;
                }
                Err(e) => log::error!("Error receiving from socket: {}", e),
            }
        }
    }

    pub async fn start_tcp_echo_server(addr: &str) -> anyhow::Result<()> {
        let listener = tokio::net::TcpListener::bind(addr).await?;
        loop {
            let (mut socket, _) = listener.accept().await?;
            tokio::spawn(async move {
                let (mut reader, mut writer) = socket.split();
                if let Err(e) = tokio::io::copy(&mut reader, &mut writer).await {
                    log::error!("failed to copy: {}", e);
                }
            });
        }
    }
}
