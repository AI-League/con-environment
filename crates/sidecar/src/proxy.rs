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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::{TcpListener, TcpStream};
    use tokio::time::sleep;

    // Helper to create a test config with TCP target
    fn test_config_tcp(target_addr: String) -> Config {
        Config {
            http_listen_addr: "127.0.0.1:0".to_string(), // Not used in proxy tests
            tcp_listen_addr: "127.0.0.1:0".to_string(),
            target_tcp_addr: Some(target_addr),
            target_uds_path: None,
        }
    }

    // Helper to create test AppState
    fn test_state() -> Arc<AppState> {
        Arc::new(AppState::new())
    }

    // Mock upstream server that echoes data back
    async fn mock_echo_server(listener: TcpListener) {
        loop {
            if let Ok((mut socket, _)) = listener.accept().await {
                tokio::spawn(async move {
                    let mut buf = vec![0u8; 1024];
                    while let Ok(n) = socket.read(&mut buf).await {
                        if n == 0 {
                            break;
                        }
                        let _ = socket.write_all(&buf[..n]).await;
                    }
                });
            }
        }
    }

    // Mock upstream server that closes immediately
    async fn mock_close_server(listener: TcpListener) {
        loop {
            if let Ok((socket, _)) = listener.accept().await {
                drop(socket); // Close immediately
            }
        }
    }
    
    #[tracing_test::traced_test]
    #[tokio::test]
    async fn test_basic_proxy_echo() {
        // Setup mock upstream
        let upstream_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let upstream_addr = upstream_listener.local_addr().unwrap();
        tokio::spawn(mock_echo_server(upstream_listener));

        // Setup proxy
        let config = Arc::new(test_config_tcp(upstream_addr.to_string()));
        let state = test_state();

        let proxy_listener = TcpListener::bind(&config.tcp_listen_addr).await.unwrap();
        let proxy_addr = proxy_listener.local_addr().unwrap();

        // Spawn proxy server
        let config_clone = config.clone();
        let state_clone = state.clone();
        tokio::spawn(async move {
            loop {
                if let Ok((stream, _)) = proxy_listener.accept().await {
                    let s = state_clone.clone();
                    let c = config_clone.clone();
                    tokio::spawn(async move {
                        let _ = proxy_connection(stream, s, c).await;
                    });
                }
            }
        });

        // Give server time to start
        sleep(Duration::from_millis(50)).await;

        // Test client
        let mut client = TcpStream::connect(proxy_addr).await.unwrap();

        // Send test data
        let test_data = b"Hello, proxy!";
        client.write_all(test_data).await.unwrap();

        // Read response
        let mut buf = vec![0u8; test_data.len()];
        client.read_exact(&mut buf).await.unwrap();

        assert_eq!(&buf, test_data, "Echoed data should match sent data");
    }

    #[tracing_test::traced_test]
    #[tokio::test]
    async fn test_activity_tracking() {
        // Setup mock upstream
        let upstream_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let upstream_addr = upstream_listener.local_addr().unwrap();
        tokio::spawn(mock_echo_server(upstream_listener));

        // Setup proxy
        let config = Arc::new(test_config_tcp(upstream_addr.to_string()));
        let state = test_state();

        let initial_activity = state.get_last_activity();

        let proxy_listener = TcpListener::bind(&config.tcp_listen_addr).await.unwrap();
        let proxy_addr = proxy_listener.local_addr().unwrap();

        let config_clone = config.clone();
        let state_clone = state.clone();
        tokio::spawn(async move {
            loop {
                if let Ok((stream, _)) = proxy_listener.accept().await {
                    let s = state_clone.clone();
                    let c = config_clone.clone();
                    tokio::spawn(async move {
                        let _ = proxy_connection(stream, s, c).await;
                    });
                }
            }
        });

        sleep(Duration::from_millis(50)).await;

        // Wait a bit to ensure timestamp can differ
        sleep(Duration::from_millis(1000)).await;

        // Connect and send data
        let mut client = TcpStream::connect(proxy_addr).await.unwrap();
        client.write_all(b"test").await.unwrap();

        // Read response
        let mut buf = vec![0u8; 4];
        client.read_exact(&mut buf).await.unwrap();

        // Check activity was updated
        let updated_activity = state.get_last_activity();
        assert!(
            updated_activity > initial_activity,
            "Activity timestamp ({}) should be updated after data transfer, ({})",
            initial_activity,
            updated_activity
        );
    }

    #[tracing_test::traced_test]
    #[tokio::test]
    async fn test_bidirectional_data_flow() {
        // Setup mock upstream
        let upstream_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let upstream_addr = upstream_listener.local_addr().unwrap();
        tokio::spawn(mock_echo_server(upstream_listener));

        // Setup proxy
        let config = Arc::new(test_config_tcp(upstream_addr.to_string()));
        let state = test_state();

        let proxy_listener = TcpListener::bind(&config.tcp_listen_addr).await.unwrap();
        let proxy_addr = proxy_listener.local_addr().unwrap();

        let config_clone = config.clone();
        let state_clone = state.clone();
        tokio::spawn(async move {
            loop {
                if let Ok((stream, _)) = proxy_listener.accept().await {
                    let s = state_clone.clone();
                    let c = config_clone.clone();
                    tokio::spawn(async move {
                        let _ = proxy_connection(stream, s, c).await;
                    });
                }
            }
        });

        sleep(Duration::from_millis(50)).await;

        let mut client = TcpStream::connect(proxy_addr).await.unwrap();

        // Send multiple messages
        for i in 0..5 {
            let msg = format!("Message {}", i);
            client.write_all(msg.as_bytes()).await.unwrap();

            let mut buf = vec![0u8; msg.len()];
            client.read_exact(&mut buf).await.unwrap();

            assert_eq!(buf, msg.as_bytes());
        }
    }

    #[tracing_test::traced_test]
    #[tokio::test]
    async fn test_upstream_connection_failure() {
        // Setup proxy with invalid upstream
        let config = Arc::new(test_config_tcp("127.0.0.1:1".to_string())); // Port 1 should be unavailable
        let state = test_state();

        let proxy_listener = TcpListener::bind(&config.tcp_listen_addr).await.unwrap();
        let proxy_addr = proxy_listener.local_addr().unwrap();

        let config_clone = config.clone();
        let state_clone = state.clone();
        tokio::spawn(async move {
            loop {
                if let Ok((stream, _)) = proxy_listener.accept().await {
                    let s = state_clone.clone();
                    let c = config_clone.clone();
                    tokio::spawn(async move {
                        let _ = proxy_connection(stream, s, c).await;
                    });
                }
            }
        });

        sleep(Duration::from_millis(50)).await;

        // Connection should fail or be immediately closed
        let result = TcpStream::connect(proxy_addr).await;
        if let Ok(mut client) = result {
            // If connection succeeds, writing should fail
            let write_result = client.write_all(b"test").await;
            assert!(
                write_result.is_err() || {
                    // Or reading should return EOF
                    let mut buf = vec![0u8; 4];
                    client.read(&mut buf).await.unwrap_or(0) == 0
                }
            );
        }
    }

    #[tracing_test::traced_test]
    #[tokio::test]
    async fn test_upstream_closes_connection() {
        // Setup mock upstream that closes immediately
        let upstream_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let upstream_addr = upstream_listener.local_addr().unwrap();
        tokio::spawn(mock_close_server(upstream_listener));

        // Setup proxy
        let config = Arc::new(test_config_tcp(upstream_addr.to_string()));
        let state = test_state();

        let proxy_listener = TcpListener::bind(&config.tcp_listen_addr).await.unwrap();
        let proxy_addr = proxy_listener.local_addr().unwrap();

        let config_clone = config.clone();
        let state_clone = state.clone();
        tokio::spawn(async move {
            loop {
                if let Ok((stream, _)) = proxy_listener.accept().await {
                    let s = state_clone.clone();
                    let c = config_clone.clone();
                    tokio::spawn(async move {
                        let _ = proxy_connection(stream, s, c).await;
                    });
                }
            }
        });

        sleep(Duration::from_millis(50)).await;

        // Connect to proxy
        let mut client = TcpStream::connect(proxy_addr).await.unwrap();

        // Try to read - should get EOF since upstream closed
        let mut buf = vec![0u8; 100];
        let n = client.read(&mut buf).await.unwrap();
        assert_eq!(n, 0, "Should receive EOF when upstream closes");
    }

    #[tracing_test::traced_test]
    #[tokio::test]
    async fn test_large_data_transfer() {
        // Setup mock upstream
        let upstream_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let upstream_addr = upstream_listener.local_addr().unwrap();
        tokio::spawn(mock_echo_server(upstream_listener));

        // Setup proxy
        let config = Arc::new(test_config_tcp(upstream_addr.to_string()));
        let state = test_state();

        let proxy_listener = TcpListener::bind(&config.tcp_listen_addr).await.unwrap();
        let proxy_addr = proxy_listener.local_addr().unwrap();

        let config_clone = config.clone();
        let state_clone = state.clone();
        tokio::spawn(async move {
            loop {
                if let Ok((stream, _)) = proxy_listener.accept().await {
                    let s = state_clone.clone();
                    let c = config_clone.clone();
                    tokio::spawn(async move {
                        let _ = proxy_connection(stream, s, c).await;
                    });
                }
            }
        });

        sleep(Duration::from_millis(50)).await;

        let mut client = TcpStream::connect(proxy_addr).await.unwrap();

        // Send 1MB of data
        let large_data = vec![0xAB; 1024 * 1024];
        client.write_all(&large_data).await.unwrap();

        // Read it back
        let mut received = vec![0u8; large_data.len()];
        client.read_exact(&mut received).await.unwrap();

        assert_eq!(
            received, large_data,
            "Large data transfer should work correctly"
        );
    }

    #[tracing_test::traced_test]
    #[tokio::test]
    async fn test_multiple_concurrent_connections() {
        // Setup mock upstream
        let upstream_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let upstream_addr = upstream_listener.local_addr().unwrap();
        tokio::spawn(mock_echo_server(upstream_listener));

        // Setup proxy
        let config = Arc::new(test_config_tcp(upstream_addr.to_string()));
        let state = test_state();

        let proxy_listener = TcpListener::bind(&config.tcp_listen_addr).await.unwrap();
        let proxy_addr = proxy_listener.local_addr().unwrap();

        let config_clone = config.clone();
        let state_clone = state.clone();
        tokio::spawn(async move {
            loop {
                if let Ok((stream, _)) = proxy_listener.accept().await {
                    let s = state_clone.clone();
                    let c = config_clone.clone();
                    tokio::spawn(async move {
                        let _ = proxy_connection(stream, s, c).await;
                    });
                }
            }
        });

        sleep(Duration::from_millis(50)).await;

        // Create multiple concurrent connections
        let mut handles = vec![];
        for i in 0..10 {
            let addr = proxy_addr;
            let handle = tokio::spawn(async move {
                let mut client = TcpStream::connect(addr).await.unwrap();
                let msg = format!("Client {}", i);
                client.write_all(msg.as_bytes()).await.unwrap();

                let mut buf = vec![0u8; msg.len()];
                client.read_exact(&mut buf).await.unwrap();

                assert_eq!(buf, msg.as_bytes());
            });
            handles.push(handle);
        }

        // Wait for all connections to complete
        for handle in handles {
            handle.await.unwrap();
        }
    }

    #[tracing_test::traced_test]
    #[tokio::test]
    async fn test_activity_stream_wrapper() {
        let state = test_state();
        let initial_time = state.get_last_activity();

        // Create a simple in-memory stream
        let (mut client, server) = tokio::io::duplex(1024);

        // Wrap it with ActivityStream
        let mut wrapped = ActivityStream::new(server, state.clone());

        // Wait a bit to ensure timestamp differs
        sleep(Duration::from_millis(1000)).await;

        // Write data through the wrapper
        client.write_all(b"test").await.unwrap();

        // Read through the wrapper
        let mut buf = vec![0u8; 4];
        wrapped.read_exact(&mut buf).await.unwrap();

        // Activity should be updated
        let updated_time = state.get_last_activity();
        assert!(updated_time > initial_time, "Activity should be tracked");
    }
}
