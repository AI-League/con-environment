// crates/integration-tests/src/main.rs

use anyhow::{Context, Result};
use tracing::{info, error};

mod client;

mod tests {
    pub mod auth;
    pub mod lifecycle;
    pub mod communication;
    pub mod gc;
    pub mod stress;
}

use client::TestClient;

#[tokio::main]
async fn main() -> Result<()> {
    // Setup logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into())
        )
        .init();

    info!("Starting Workshop Hub Integration Tests");
    info!("=========================================");

    // Create test client that connects to deployed system
    let client = TestClient::new().await
        .context("Failed to create test client")?;

    info!("Connected to cluster: {}", client.cluster_info());
    info!("Hub namespace: {}", client.hub_namespace());
    info!("Workshop namespace: {}", client.workshop_namespace());

    // Run health checks first
    info!("\n=== Pre-flight Checks ===");
    client.verify_deployment().await?;

    // Run test suites
    let mut failures = Vec::new();

    info!("\n=== Test Suite: Authentication ===");
    if let Err(e) = tests::auth::run_tests(&client).await {
        error!("Auth tests failed: {}", e);
        failures.push("Authentication");
    }

    info!("\n=== Test Suite: Pod Lifecycle ===");
    if let Err(e) = tests::lifecycle::run_tests(&client).await {
        error!("Lifecycle tests failed: {}", e);
        failures.push("Pod Lifecycle");
    }

    info!("\n=== Test Suite: Communication ===");
    if let Err(e) = tests::communication::run_tests(&client).await {
        error!("Communication tests failed: {}", e);
        failures.push("Communication");
    }

    info!("\n=== Test Suite: Garbage Collection ===");
    if let Err(e) = tests::gc::run_tests(&client).await {
        error!("GC tests failed: {}", e);
        failures.push("Garbage Collection");
    }

    info!("\n=== Test Suite: Stress Testing ===");
    if let Err(e) = tests::stress::run_tests(&client).await {
        error!("Stress tests failed: {}", e);
        failures.push("Stress Testing");
    }

    // Cleanup
    info!("\n=== Cleanup ===");
    client.cleanup_test_resources().await?;

    // Summary
    info!("\n=== Test Summary ===");
    if failures.is_empty() {
        info!("✅ All test suites passed!");
        Ok(())
    } else {
        error!("❌ Failed test suites:");
        for failure in failures {
            error!("  - {}", failure);
        }
        anyhow::bail!("Integration tests failed")
    }
}