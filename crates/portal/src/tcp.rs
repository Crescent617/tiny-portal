use std::sync::Arc;

use tokio::io;
use tokio::net::{TcpListener, TcpStream};

#[derive(Debug)]
pub struct TcpPortForwarder {
    pub src: String,
    pub dst: String,
    conn_cnt: Arc<std::sync::atomic::AtomicU64>,
}

impl std::fmt::Display for TcpPortForwarder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} -> {} (TCP)", self.src, self.dst)
    }
}

impl TcpPortForwarder {
    pub fn new(src: &str, dst: &str) -> Self {
        Self {
            src: src.to_string(),
            dst: dst.to_string(),
            conn_cnt: Arc::new(0.into()),
        }
    }

    pub async fn start(&self) -> anyhow::Result<()> {
        log::info!("Running {}", self);

        let listener = TcpListener::bind(&self.src).await?;

        loop {
            match listener.accept().await {
                Ok((client, _)) => {
                    log::debug!("Accepted connection from {}", client.peer_addr()?);
                    tokio::spawn(handle_tcp(client, self.dst.clone(), self.conn_cnt.clone()));
                }
                Err(e) => log::error!("Error accepting connection: {}", e),
            }
        }
    }

    pub fn get_conn_cnt(&self) -> Arc<std::sync::atomic::AtomicU64> {
        return self.conn_cnt.clone();
    }

}

impl super::Portal for TcpPortForwarder {
    async fn start(&self) -> anyhow::Result<()> {
        self.start().await
    }
}

async fn handle_tcp(
    client: TcpStream,
    target_addr: impl tokio::net::ToSocketAddrs,
    cnt: Arc<std::sync::atomic::AtomicU64>,
) -> io::Result<()> {
    cnt.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    scopeguard::defer! {
        cnt.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
    };

    match TcpStream::connect(target_addr).await {
        Ok(target) => {
            let (mut cr, mut cw) = client.into_split();
            let (mut tr, mut tw) = target.into_split();

            let client_to_target = tokio::spawn(async move { io::copy(&mut cr, &mut tw).await });
            io::copy(&mut tr, &mut cw).await?;
            let _ = client_to_target.await?;
            log::debug!("Connection closed");
        }
        Err(e) => {
            log::error!("Error connecting to target: {}", e);
        }
    }

    Ok(())
}
