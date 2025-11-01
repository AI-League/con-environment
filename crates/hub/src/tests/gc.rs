
use k8s_openapi::api::core::v1::Pod;
use kube::{api::PostParams, Api};
use std::collections::BTreeMap;
use crate::gc;
use super::helpers::TestContext;

#[tracing_test::traced_test]
#[tokio::test]
async fn test_gc_cleans_up_idle_pods() {
    let ctx = TestContext::new_for_gc("test_gc_cleans_up_idle_pods").await;
    
    // Create a test pod with sidecar
    let pod = ctx.create_test_pod("idle-user").await
        .expect("Failed to create test pod");
    let pod_name = pod.metadata.name.as_ref().unwrap();
    
    // Wait for pod to be running
    ctx.wait_for_pod_running(pod_name).await
        .expect("Pod should reach running state");
    
    // Create matching service
    ctx.create_test_service("idle-user").await
        .expect("Failed to create test service");
    
    // Run GC with 0 second idle threshold (immediate cleanup)
    let pod_api: Api<Pod> = Api::namespaced(
        ctx.client.clone(),
        &ctx.config.workshop_namespace
    );
    
    let result = gc::cleanup_idle_pods(
        &pod_api,
        &ctx.config.workshop_name,
        0, // 0 second idle threshold
    ).await;
    
    assert!(result.is_ok(), "GC should succeed");
    
    // Wait for deletion to complete
    tokio::time::sleep(std::time::Duration::from_secs(4)).await;
    
    // Verify pod and service were deleted
    assert!(!ctx.pod_exists(pod_name).await, "Pod should be deleted");
    assert!(!ctx.service_exists(&format!("{}-idle-user", ctx.config.workshop_name)).await, 
            "Service should be deleted");
}

#[tracing_test::traced_test]
#[tokio::test]
async fn test_gc_respects_ttl() {
    let ctx = TestContext::new_for_gc("test_gc_respects_ttl").await;
    
    let pod_api: Api<Pod> = Api::namespaced(
        ctx.client.clone(),
        &ctx.config.workshop_namespace
    );
    
    // Create a pod with expired TTL
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    
    let expired_time = now - 100; // Expired 100 seconds ago
    
    let mut annotations = BTreeMap::new();
    annotations.insert(
        "workshop-hub/ttl-expires-at".to_string(),
        expired_time.to_string()
    );
    
    let mut labels = BTreeMap::new();
    labels.insert("workshop-hub/user-id".to_string(), "ttl-user".to_string());
    labels.insert("workshop-hub/workshop-name".to_string(), ctx.config.workshop_name.clone());
    labels.insert("app.kubernetes.io/managed-by".to_string(), "workshop-hub".to_string());
    
    let pod: Pod = serde_json::from_value(serde_json::json!({
        "apiVersion": "v1",
        "kind": "Pod",
        "metadata": {
            "name": "ttl-test-pod",
            "labels": labels,
            "annotations": annotations
        },
        "spec": {
            "containers": [{
                "name": "test",
                "image": "nginx:alpine",
                "ports": [{"containerPort": 80}]
            }]
        }
    })).unwrap();
    
    pod_api.create(&PostParams::default(), &pod).await
        .expect("Failed to create pod with TTL");
    
    // Run GC with high idle threshold - TTL should still trigger deletion
    
    let result = gc::cleanup_idle_pods(
        &pod_api,
        &ctx.config.workshop_name,
        3600, // High idle threshold - shouldn't matter
    ).await;
    
    assert!(result.is_ok(), "GC should succeed");
    
    // Wait for deletion
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    
    // Pod should be deleted due to expired TTL
    assert!(!ctx.pod_exists("ttl-test-pod").await, "Pod should be deleted due to expired TTL");
}
#[tokio::test]
async fn test_gc_only_affects_managed_pods() {
    let ctx = TestContext::new("test_gc_only_affects_managed_pods").await;
    
    let pod_api: Api<Pod> = Api::namespaced(
        ctx.client.clone(),
        &ctx.config.workshop_namespace
    );
    
    // Clean up any leftover unmanaged pod from previous failed test runs
    let _ = pod_api.delete("unmanaged-test-pod", &kube::api::DeleteParams::default()).await;
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
    
    // Create a managed pod
    let managed_pod = ctx.create_test_pod("managed-user").await.unwrap();
    
    // Create an unmanaged pod (no workshop-hub labels)
    let unmanaged_pod: Pod = serde_json::from_value(serde_json::json!({
        "apiVersion": "v1",
        "kind": "Pod",
        "metadata": {
            "name": "unmanaged-test-pod",
            "labels": {
                "app": "unmanaged"
            }
        },
        "spec": {
            "containers": [{
                "name": "test",
                "image": "nginx:alpine",
                "ports": [{"containerPort": 80}]
            }]
        }
    })).unwrap();
    
    pod_api.create(&kube::api::PostParams::default(), &unmanaged_pod)
        .await
        .expect("Failed to create unmanaged pod");
    
    // Wait a moment
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
    
    // Run GC with zero idle threshold
    
    let result = gc::cleanup_idle_pods(
        &pod_api,
        &ctx.config.workshop_name,
        0,
    ).await;
    
    assert!(result.is_ok());
    
    // Wait for cleanup
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
    
    // Managed pod should be deleted
    let managed_name = managed_pod.metadata.name.as_ref().unwrap();
    assert!(!ctx.pod_exists(managed_name).await);
    
    // Unmanaged pod should still exist
    assert!(ctx.pod_exists("unmanaged-test-pod").await);
    
    // Clean up the unmanaged pod (this will run even if assertions fail with proper cleanup)
    let _ = pod_api.delete("unmanaged-test-pod", &kube::api::DeleteParams::default()).await;
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
}

#[tracing_test::traced_test]
#[tokio::test]
async fn test_gc_handles_missing_health_endpoint() {
    let ctx = TestContext::new_for_gc("test_gc_handles_missing_health_endpoint").await;
    
    // Create a pod without sidecar (no health endpoint)
    let pod_api: Api<Pod> = Api::namespaced(
        ctx.client.clone(),
        &ctx.config.workshop_namespace
    );
    
    let mut labels = BTreeMap::new();
    labels.insert("workshop-hub/user-id".to_string(), "no-health".to_string());
    labels.insert("workshop-hub/workshop-name".to_string(), ctx.config.workshop_name.clone());
    labels.insert("app.kubernetes.io/managed-by".to_string(), "workshop-hub".to_string());
    
    let pod: Pod = serde_json::from_value(serde_json::json!({
        "apiVersion": "v1",
        "kind": "Pod",
        "metadata": {
            "name": "no-health-pod",
            "labels": labels
        },
        "spec": {
            "containers": [{
                "name": "test",
                "image": "nginx:alpine",
                "ports": [{"containerPort": 80}]
            }]
        }
    })).unwrap();
    
    pod_api.create(&PostParams::default(), &pod).await
        .expect("Failed to create pod without health endpoint");
    
    // Wait for pod to be running
    ctx.wait_for_pod_running("no-health-pod").await.ok();
    
    // Run GC - should handle missing health endpoint gracefully
    
    let result = gc::cleanup_idle_pods(
        &pod_api,
        &ctx.config.workshop_name,
        3600, // High threshold
    ).await;
    
    assert!(result.is_ok(), "GC should handle missing health endpoint gracefully");
    
    // Wait a bit
    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    
    // Pod should be deleted due to failed health check
    assert!(!ctx.pod_exists("no-health-pod").await, 
            "Pod without health endpoint should be considered unhealthy and deleted");
}

#[tracing_test::traced_test]
#[tokio::test]
async fn test_gc_cleans_failed_pods() {
    let ctx = TestContext::new_for_gc("test_gc_cleans_failed_pods").await;
    
    // Create a pod that will fail (invalid image)
    let pod_api: Api<Pod> = Api::namespaced(
        ctx.client.clone(),
        &ctx.config.workshop_namespace
    );
    
    let mut labels = BTreeMap::new();
    labels.insert("workshop-hub/user-id".to_string(), "failed-user".to_string());
    labels.insert("workshop-hub/workshop-name".to_string(), ctx.config.workshop_name.clone());
    labels.insert("app.kubernetes.io/managed-by".to_string(), "workshop-hub".to_string());
    
    let pod: Pod = serde_json::from_value(serde_json::json!({
        "apiVersion": "v1",
        "kind": "Pod",
        "metadata": {
            "name": "failed-test-pod",
            "labels": labels
        },
        "spec": {
            "restartPolicy": "Never",
            "containers": [{
                "name": "test",
                "image": "this-image-does-not-exist:latest",
                "ports": [{"containerPort": 80}]
            }]
        }
    })).unwrap();
    
    pod_api.create(&PostParams::default(), &pod).await
        .expect("Failed to create failing pod");
    
    // Wait for pod to enter failed state
    tokio::time::sleep(std::time::Duration::from_secs(10)).await;
    
    // Run GC
    
    let result = gc::cleanup_idle_pods(
        &pod_api,
        &ctx.config.workshop_name,
        3600,
    ).await;
    
    assert!(result.is_ok(), "GC should succeed");
    
    // Wait for deletion
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    
    // Failed pod should be cleaned up
    assert!(!ctx.pod_exists("failed-test-pod").await, "Failed pod should be cleaned up");
}