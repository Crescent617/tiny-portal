use std::net::{SocketAddr, ToSocketAddrs};
use std::sync::Arc;
use std::time::Duration;
use tokio::io;
use tokio::net::{TcpListener, TcpStream, UdpSocket};
use tokio::time::timeout;

const MAX_UDP_PACKET_SIZE: usize = 65536;

#[derive(Debug)]
pub enum Protocol {
    TCP,
    UDP,
}

#[derive(Debug)]
pub struct PortForwarder {
    pub src: String,
    pub dst: String,
    pub protocol: Protocol,
    done: tokio::sync::Notify,
}

impl std::fmt::Display for PortForwarder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} -> {} ({:?})", self.src, self.dst, self.protocol)
    }
}

impl PortForwarder {
    pub fn new(src: &str, dst: &str, protocol: Protocol) -> Self {
        Self {
            src: src.to_string(),
            dst: dst.to_string(),
            protocol,
            done: tokio::sync::Notify::new(),
        }
    }

    pub fn stop(&self) {
        self.done.notify_waiters()
    }

    pub async fn start(&self) -> anyhow::Result<()> {
        match self.protocol {
            Protocol::TCP => self.run_tcp().await,
            Protocol::UDP => self.run_udp().await,
        }
    }

    pub async fn start_udp_echo_test_server(&self) -> anyhow::Result<()> {
        log::info!("Running {}", self);

        let sock = Arc::new(UdpSocket::bind(&self.dst).await?);

        let mut b1 = [0u8; MAX_UDP_PACKET_SIZE];

        loop {
            let sock = sock.clone();

            tokio::select! {
                res = sock.recv_from(&mut b1) => {
                   match res {
                        Ok((size, client_addr)) => {
                            log::debug!("Received {} bytes from {}", size, client_addr);
                            sock.send_to(&b1[..size], client_addr).await?;
                        }
                        Err(e) => log::error!("Error receiving from socket: {}", e),
                    }
                }
                _ = self.done.notified() => {
                    return Ok(());
                }
            }
        }
    }

    async fn run_tcp(&self) -> anyhow::Result<()> {
        log::info!("Running {}", self);

        let listener = TcpListener::bind(&self.src).await?;

        loop {
            tokio::select! {
                req = listener.accept() => {
                    match req {
                        Ok((client, _)) => {
                            log::debug!("Accepted connection from {}", client.peer_addr()?);
                            tokio::spawn(handle_tcp(client, self.dst.clone()));
                        }
                        Err(e) => log::error!("Error accepting connection: {}", e),
                    }
                }
                _ = self.done.notified() => {
                    return Ok(());
                }
            }
        }
    }

    async fn run_udp(&self) -> anyhow::Result<()> {
        log::info!("Running {}", self);

        let sock = Arc::new(UdpSocket::bind(&self.src).await?);
        let dst_addr: SocketAddr = self.dst.parse()?;

        let mut b1 = [0u8; MAX_UDP_PACKET_SIZE];

        loop {
            let sock = sock.clone();

            tokio::select! {
                res = sock.recv_from(&mut b1) => {
                   match res {
                        Ok((size, client_addr)) => {
                            log::debug!("Received {} bytes from {}", size, client_addr);
                            let fw_sock = UdpSocket::bind("0.0.0.0:0").await?;
                            fw_sock.send_to(&b1[..size], dst_addr).await?;

                            tokio::spawn(async move {
                                let mut b2 = [0u8; MAX_UDP_PACKET_SIZE];
                                if let Ok(r) = timeout(Duration::from_secs(5), fw_sock.recv_from(&mut b2)).await {
                                    if let Ok((size, _)) = r {
                                        log::debug!("Forwarding {} bytes to {}", size, client_addr);
                                        if let Err(e) = sock.send_to(&b2[..size], client_addr).await {
                                            log::error!("Error sending to socket: {}", e);
                                        }
                                    }
                                } else {
                                    log::error!("Timeout waiting for response from {}", dst_addr);
                                }
                            });
                        }
                        Err(e) => log::error!("Error receiving from socket: {}", e),
                    }
                }
                _ = self.done.notified() => {
                    return Ok(());
                }
            }
        }
    }
}

impl Drop for PortForwarder {
    fn drop(&mut self) {
        self.stop();
    }
}

async fn handle_tcp(
    client: TcpStream,
    target_addr: impl tokio::net::ToSocketAddrs,
) -> io::Result<()> {
    match TcpStream::connect(target_addr).await {
        Ok(target) => {
            let (mut cr, mut cw) = client.into_split();
            let (mut tr, mut tw) = target.into_split();

            let client_to_target = tokio::spawn(async move { io::copy(&mut cr, &mut tw).await });
            io::copy(&mut tr, &mut cw).await?;
            let _ = client_to_target.await?;
        }
        Err(e) => {
            log::error!("Error connecting to target: {}", e);
        }
    }

    Ok(())
}
