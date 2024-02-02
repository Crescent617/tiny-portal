use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::UdpSocket;
use tokio::sync::Mutex;

pub const MAX_UDP_PACKET_SIZE: usize = 65536;
const UDP_CHECK_INTERVAL: Duration = Duration::from_secs(5);
const DEFAULT_UDP_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug)]
struct UdpConn {
    forward_sock: Arc<UdpSocket>,
    last_activity: Arc<Mutex<std::time::Instant>>,
    join_handle: tokio::task::JoinHandle<()>,
}

impl UdpConn {
    async fn new(
        client_addr: SocketAddr,
        dst_addr: SocketAddr,
        server_sock: Arc<UdpSocket>,
    ) -> anyhow::Result<Self> {
        let forward_sock = Arc::new(UdpSocket::bind("0.0.0.0:0").await?);
        forward_sock.connect(dst_addr).await?;

        let last_activity = Arc::new(Mutex::new(std::time::Instant::now()));

        let forward_sock_clone = forward_sock.clone();
        let last_activity_clone = last_activity.clone();

        let join_handle = tokio::spawn(async move {
            let mut b2 = [0u8; MAX_UDP_PACKET_SIZE];
            while let Ok((size, _)) = forward_sock_clone.recv_from(&mut b2).await {
                Self::update_activity(last_activity_clone.clone()).await;

                log::debug!("Forwarding {} bytes to {}", size, client_addr);
                if let Err(e) = server_sock.send_to(&b2[..size], client_addr).await {
                    log::error!("Error sending to socket: {}", e);
                }
            }
        });

        Ok(Self {
            forward_sock,
            last_activity,
            join_handle,
        })
    }

    async fn send(&mut self, data: &[u8]) -> anyhow::Result<()> {
        self.forward_sock.send(data).await?;
        Self::update_activity(self.last_activity.clone()).await;
        Ok(())
    }

    async fn update_activity(last: Arc<Mutex<std::time::Instant>>) {
        let mut t = last.lock().await;
        *t = std::time::Instant::now();
    }
}

impl Drop for UdpConn {
    fn drop(&mut self) {
        self.join_handle.abort();
    }
}

#[derive(Debug)]
pub struct UdpPortForwarder {
    pub src: String,
    pub dst: String,
    conns: Arc<Mutex<HashMap<SocketAddr, UdpConn>>>,
}

impl std::fmt::Display for UdpPortForwarder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} -> {} (UDP)", self.src, self.dst)
    }
}

impl UdpPortForwarder {
    pub fn new(src: &str, dst: &str) -> Self {
        Self {
            src: src.to_string(),
            dst: dst.to_string(),
            conns: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    async fn cleanup(conns: Arc<Mutex<HashMap<SocketAddr, UdpConn>>>, timeout: Duration) -> usize {
        let mut conns = conns.lock().await;
        let mut to_remove = Vec::new();
        for (k, v) in conns.iter() {
            if v.last_activity.lock().await.elapsed() > timeout {
                to_remove.push(*k);
            }
        }
        for k in &to_remove {
            conns.remove(k);
        }
        to_remove.len()
    }

    pub async fn start(&self) -> anyhow::Result<()> {
        log::info!("Running {}", self);

        let conns = self.conns.clone();
        let _cleanup_drop_guard = TaskDropGuard(tokio::spawn(async move {
            log::info!("Starting cleanup task");
            loop {
                tokio::time::sleep(UDP_CHECK_INTERVAL).await;
                let n = Self::cleanup(conns.clone(), DEFAULT_UDP_TIMEOUT).await;
                log::debug!("Cleaned up {} connections", n);
            }
        }));

        let sock = Arc::new(UdpSocket::bind(&self.src).await?);
        let dst_addr: SocketAddr = self.dst.parse()?;

        let mut b1 = [0u8; MAX_UDP_PACKET_SIZE];

        loop {
            let sock = sock.clone();

            match sock.recv_from(&mut b1).await {
                Ok((size, client_addr)) => {
                    log::debug!("Received {} bytes from {}", size, client_addr);
                    match self.conns.lock().await.entry(client_addr) {
                        Entry::Occupied(mut e) => {
                            log::debug!(
                                "Reuse forwarding {} bytes from {} to {}",
                                size,
                                client_addr,
                                dst_addr
                            );
                            e.get_mut().send(&b1[..size]).await?;
                        }
                        Entry::Vacant(e) => {
                            log::debug!(
                                "New forwarding {} bytes from {} to {}",
                                size,
                                client_addr,
                                dst_addr
                            );
                            let mut conn =
                                UdpConn::new(client_addr, dst_addr, sock.clone()).await?;
                            conn.send(&b1[..size]).await?;
                            e.insert(conn);
                        }
                    }
                }
                Err(e) => log::error!("Error receiving from socket: {}", e),
            }
        }
    }
}

struct TaskDropGuard<T>(tokio::task::JoinHandle<T>);
impl<T> Drop for TaskDropGuard<T> {
    fn drop(&mut self) {
        self.0.abort();
    }
}