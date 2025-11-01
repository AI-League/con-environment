use std::io;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::net::{TcpListener, TcpStream, UnixStream};
use tracing::{error, info, warn};

use crate::config::Config;
use crate::AppState;

/// An enum to represent our two possible upstream connection types.
enum UpstreamStream {
    Tcp(TcpStream),
    Uds(UnixStream),
}

// Implement AsyncRead and AsyncWrite for the enum so we can
// use it generically in tokio::io::copy_bidirectional.
impl AsyncRead for UpstreamStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        match self.get_mut() {
            UpstreamStream::Tcp(s) => Pin::new(s).poll_read(cx, buf),
            UpstreamStream::Uds(s) => Pin::new(s).poll_read(cx, buf),
        }
    }
}

impl AsyncWrite for UpstreamStream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        match self.get_mut() {
            UpstreamStream::Tcp(s) => Pin::new(s).poll_write(cx, buf),
            UpstreamStream::Uds(s) => Pin::new(s).poll_write(cx, buf),
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        match self.get_mut() {
            UpstreamStream::Tcp(s) => Pin::new(s).poll_flush(cx),
            UpstreamStream::Uds(s) => Pin::new(s).poll_flush(cx),
        }
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        match self.get_mut() {
            UpstreamStream::Tcp(s) => Pin::new(s).poll_shutdown(cx),
            UpstreamStream::Uds(s) => Pin::new(s).poll_shutdown(cx),
        }
    }
}

/// A wrapper around an I/O stream that updates the AppState on activity.
#[pin_project::pin_project]
struct ActivityStream<S> {
    #[pin]
    inner: S,
    state: Arc<AppState>,
}

impl<S> ActivityStream<S> {
    fn new(inner: S, state: Arc<AppState>) -> Self {
        Self { inner, state }
    }
}

impl<S: AsyncRead> AsyncRead for ActivityStream<S> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = self.project();
        match this.inner.poll_read(cx, buf) {
            Poll::Ready(Ok(())) if !buf.filled().is_empty() => {
                this.state.update_activity();
                Poll::Ready(Ok(()))
            }
            other => other,
        }
    }
}

impl<S: AsyncWrite> AsyncWrite for ActivityStream<S> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        let this = self.project();
        match this.inner.poll_write(cx, buf) {
            Poll::Ready(Ok(n)) if n > 0 => {
                this.state.update_activity();
                Poll::Ready(Ok(n))
            }
            other => other,
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        self.project().inner.poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        self.project().inner.poll_shutdown(cx)
    }
}

/// Main TCP proxy loop. Listens for connections and spawns a task for each.
pub async fn run_proxy(state: Arc<AppState>, config: Arc<Config>) -> io::Result<()> {
    let listener = TcpListener::bind(&config.tcp_listen_addr).await?;
    info!("TCP Proxy listening on {}", &config.tcp_listen_addr);

    loop {
        match listener.accept().await {
            Ok((downstream_stream, downstream_addr)) => {
                info!("Accepted new connection from: {}", downstream_addr);

                // Clone state and config for the new task
                let state_clone = state.clone();
                let config_clone = config.clone();

                tokio::spawn(async move {
                    if let Err(e) =
                        proxy_connection(downstream_stream, state_clone, config_clone).await
                    {
                        warn!(
                            "Connection from {} ended with error: {}",
                            downstream_addr, e
                        );
                    } else {
                        info!("Connection from {} ended gracefully.", downstream_addr);
                    }
                });
            }
            Err(e) => {
                error!("Failed to accept connection: {}", e);
            }
        }
    }
}

/// Connects to the configured upstream (TCP or UDS).
async fn connect_upstream(config: &Config) -> io::Result<UpstreamStream> {
    if let Some(tcp_addr) = &config.target_tcp_addr {
        let stream = TcpStream::connect(tcp_addr).await?;
        info!("Connected to upstream TCP: {}", tcp_addr);
        Ok(UpstreamStream::Tcp(stream))
    } else if let Some(uds_path) = &config.target_uds_path {
        let stream = UnixStream::connect(uds_path).await?;
        info!("Connected to upstream UDS: {}", uds_path);
        Ok(UpstreamStream::Uds(stream))
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "No upstream target configured",
        ))
    }
}

/// Handles a single proxy connection.
async fn proxy_connection(
    downstream: TcpStream,
    state: Arc<AppState>,
    config: Arc<Config>,
) -> io::Result<()> {
    // 1. Connect to the upstream (workshop container)
    let upstream = connect_upstream(&config).await?;

    // 2. Wrap both streams to update activity
    let mut wrapped_downstream = ActivityStream::new(downstream, state.clone());
    let mut wrapped_upstream = ActivityStream::new(upstream, state);

    // 3. Proxy data
    info!("Starting bi-directional copy...");
    tokio::io::copy_bidirectional(&mut wrapped_downstream, &mut wrapped_upstream).await?;

    Ok(())
}
