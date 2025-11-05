use super::*;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::sleep;

// Mock upstream echo server
async fn mock_upstream_server(port: u16) {
    let listener = TcpListener::bind(format!("127.0.0.1:{}", port))
        .await
        .unwrap();
    
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

#[tokio::test]
async fn test_sidecar_end_to_end() {
    // This test starts the entire sidecar application and tests it end-to-end:
    // 1. Mock upstream server (simulates the workshop container)
    // 2. Start sidecar (HTTP health + TCP proxy)
    // 3. Check initial health
    // 4. Wait to accumulate idle time
    // 5. Send traffic through proxy
    // 6. Verify health endpoint shows activity was tracked

    // Step 1: Start mock upstream server
    let upstream_port = 19001; // Use a high port to avoid conflicts
    tokio::spawn(mock_upstream_server(upstream_port));
    sleep(Duration::from_millis(100)).await;

    // Step 2: Configure sidecar directly (not using envy in tests)
    let config = Config {
        http_listen: "127.0.0.1:18080".to_string(),
        tcp_listen: "127.0.0.1:18888".to_string(),
        target_tcp: Some(format!("127.0.0.1:{}", upstream_port)),
        target_uds: None,
    };

    assert!(config.validate().is_ok(), "Config should be valid");
    let config = Arc::new(config);

    // Step 3: Create shared state
    let state = Arc::new(AppState::new());

    // Step 4: Start HTTP health server
    let http_state = state.clone();
    let http_config = config.clone();
    tokio::spawn(async move {
        if let Err(e) = http_server::run_http_server(http_state, http_config).await {
            eprintln!("HTTP server error: {}", e);
        }
    });

    // Step 5: Start TCP proxy server
    let proxy_state = state.clone();
    let proxy_config = config.clone();
    tokio::spawn(async move {
        if let Err(e) = proxy::run_proxy(proxy_state, proxy_config).await {
            eprintln!("Proxy server error: {}", e);
        }
    });

    // Give servers time to start
    sleep(Duration::from_millis(200)).await;

    // Step 6: Test initial health check
    let client = reqwest::Client::new();
    let resp = client
        .get("http://127.0.0.1:18080/health")
        .send()
        .await
        .expect("Health check should succeed");

    assert_eq!(resp.status(), 200, "Health endpoint should return 200");
    
    let health: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(health["status"], "ok");
    
    let initial_idle = health["idle_seconds"].as_u64().unwrap();
    let initial_timestamp = health["last_activity_timestamp"].as_i64().unwrap();
    
    println!("Initial idle_seconds: {}", initial_idle);
    println!("Initial timestamp: {}", initial_timestamp);
    
    assert!(
        initial_idle <= 1,
        "Initial idle should be 0-1 seconds, got {}",
        initial_idle
    );

    // Step 7: Wait for idle time to accumulate
    println!("Waiting 3 seconds for idle time to accumulate...");
    sleep(Duration::from_secs(3)).await;

    // Step 8: Check that idle time has increased
    let resp = client
        .get("http://127.0.0.1:18080/health")
        .send()
        .await
        .unwrap();
    
    let health: serde_json::Value = resp.json().await.unwrap();
    let idle_after_wait = health["idle_seconds"].as_u64().unwrap();
    
    println!("Idle seconds after 3s wait: {}", idle_after_wait);
    
    assert!(
        idle_after_wait >= 3,
        "After waiting 3 seconds, idle should be at least 3, got {}",
        idle_after_wait
    );

    // Step 9: Send traffic through the TCP proxy to trigger activity
    println!("Sending traffic through proxy...");
    let mut proxy_client = TcpStream::connect("127.0.0.1:18888")
        .await
        .expect("Should connect to proxy");

    let test_message = b"Hello through sidecar proxy!";
    proxy_client
        .write_all(test_message)
        .await
        .expect("Should write to proxy");

    let mut response = vec![0u8; test_message.len()];
    proxy_client
        .read_exact(&mut response)
        .await
        .expect("Should read from proxy");

    assert_eq!(
        &response,
        test_message,
        "Should receive echoed message"
    );

    println!("Proxy traffic successful!");

    // Step 10: Check health again - idle should be reset to ~0
    // Wait a tiny bit for activity to propagate
    sleep(Duration::from_millis(100)).await;

    let resp = client
        .get("http://127.0.0.1:18080/health")
        .send()
        .await
        .unwrap();
    
    let health: serde_json::Value = resp.json().await.unwrap();
    let idle_after_activity = health["idle_seconds"].as_u64().unwrap();
    let timestamp_after_activity = health["last_activity_timestamp"].as_i64().unwrap();
    
    println!("Idle seconds after proxy activity: {}", idle_after_activity);
    println!("Timestamp after activity: {}", timestamp_after_activity);
    
    assert!(
        idle_after_activity <= 1,
        "After proxy activity, idle should be 0-1 seconds, got {}",
        idle_after_activity
    );

    assert!(
        timestamp_after_activity > initial_timestamp,
        "Timestamp should have increased from {} to {}",
        initial_timestamp,
        timestamp_after_activity
    );

    // Step 11: Verify continuous activity tracking
    println!("Testing continuous activity tracking...");
    
    // Send multiple messages with delays
    for i in 0..3 {
        sleep(Duration::from_secs(1)).await;
        
        let msg = format!("Message {}", i);
        proxy_client.write_all(msg.as_bytes()).await.unwrap();
        
        let mut buf = vec![0u8; msg.len()];
        proxy_client.read_exact(&mut buf).await.unwrap();
        
        // Check health - should stay low
        let resp = client
            .get("http://127.0.0.1:18080/health")
            .send()
            .await
            .unwrap();
        
        let health: serde_json::Value = resp.json().await.unwrap();
        let idle = health["idle_seconds"].as_u64().unwrap();
        
        println!("Idle after message {}: {}s", i, idle);
        
        assert!(
            idle <= 2,
            "During activity, idle should stay low, got {}",
            idle
        );
    }

    println!("✓ End-to-end test passed!");
}

#[tokio::test]
async fn test_sidecar_with_no_activity() {
    // Test that idle time increases when there's no activity
    
    let upstream_port = 19002;
    tokio::spawn(mock_upstream_server(upstream_port));
    sleep(Duration::from_millis(100)).await;

    let config = Config {
        http_listen: "127.0.0.1:18081".to_string(),
        tcp_listen: "127.0.0.1:18889".to_string(),
        target_tcp: Some(format!("127.0.0.1:{}", upstream_port)),
        target_uds: None,
    };

    let config = Arc::new(config);
    let state = Arc::new(AppState::new());

    let http_state = state.clone();
    let http_config = config.clone();
    tokio::spawn(async move {
        let _ = http_server::run_http_server(http_state, http_config).await;
    });

    let proxy_state = state.clone();
    let proxy_config = config.clone();
    tokio::spawn(async move {
        let _ = proxy::run_proxy(proxy_state, proxy_config).await;
    });

    sleep(Duration::from_millis(200)).await;

    let client = reqwest::Client::new();

    // Check idle time increases over several checks
    let mut last_idle = 0;
    
    for i in 0..5 {
        sleep(Duration::from_secs(1)).await;
        
        let resp = client
            .get("http://127.0.0.1:18081/health")
            .send()
            .await
            .unwrap();
        
        let health: serde_json::Value = resp.json().await.unwrap();
        let idle = health["idle_seconds"].as_u64().unwrap();
        
        println!("Check {}: idle = {}s", i, idle);
        
        assert!(
            idle >= last_idle,
            "Idle time should increase or stay same, was {} now {}",
            last_idle,
            idle
        );
        
        last_idle = idle;
    }

    println!("✓ No-activity test passed! Final idle: {}s", last_idle);
    assert!(last_idle >= 4, "After 5 seconds, should be at least 4s idle");
}