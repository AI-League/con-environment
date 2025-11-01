// crates/hub/tests/gc_tests.rs
// Tests for the garbage collector functionality

use k8s_openapi::api::core::v1::{Pod, Service};
use kube::{api::ListParams, Api};
use std::collections::BTreeMap;
use crate::gc;

use crate::tests::helpers::TestContext;

#[tokio::test]
#[ignore] // Remove this to run with actual k8s cluster
async fn test_gc_cleans_up_idle_pods() {
    let ctx = TestContext::new().await;
    ctx.cleanup().await;
    
    // Create a test pod
    let pod = ctx.create_test_pod("idle-user").await.unwrap();
    let pod_name = pod.metadata.name.as_ref().unwrap();
    
    // Wait for it to be running
    ctx.wait_for_pod_running(pod_name).await.ok();
    
    // Run GC with a very low idle threshold (0 seconds)
    // This should delete the pod immediately since it has no activity
    let pod_api: Api<Pod> = Api::namespaced(
        ctx.client.clone(),
        &ctx.config.workshop_namespace
    );
    let svc_api: Api<Service> = Api::namespaced(
        ctx.client.clone(),
        &ctx.config.workshop_namespace
    );
    
    let result = gc::cleanup_idle_pods(
        &pod_api,
        &svc_api,
        &ctx.config.workshop_name,
        0, // 0 second idle threshold
    ).await;
    
    assert!(result.is_ok());
    
    // Wait a bit for deletion to complete
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
    
    // Verify pod was deleted
    assert!(!ctx.pod_exists(pod_name).await);
    
    ctx.cleanup().await;
}

#[tokio::test]
#[ignore] // Remove this to run with actual k8s cluster
async fn test_gc_respects_ttl() {
    let ctx = TestContext::new().await;
    ctx.cleanup().await;
    
    // Create a pod with a TTL that has expired
    let pod_api: Api<Pod> = Api::namespaced(
        ctx.client.clone(),
        &ctx.config.workshop_namespace
    );
    
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
    
    pod_api.create(&kube::api::PostParams::default(), &pod).await.unwrap();
    
    // Run GC
    let svc_api: Api<Service> = Api::namespaced(
        ctx.client.clone(),
        &ctx.config.workshop_namespace
    );
    
    let result = gc::cleanup_idle_pods(
        &pod_api,
        &svc_api,
        &ctx.config.workshop_name,
        3600, // High idle threshold - shouldn't matter, TTL should trigger
    ).await;
    
    assert!(result.is_ok());
    
    // Wait for deletion
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
    
    // Pod should be deleted due to expired TTL
    assert!(!ctx.pod_exists("ttl-test-pod").await);
    
    ctx.cleanup().await;
}

#[tokio::test]
#[ignore] // Remove this to run with actual k8s cluster
async fn test_gc_cleans_failed_pods() {
    let ctx = TestContext::new().await;
    ctx.cleanup().await;
    
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
    
    pod_api.create(&kube::api::PostParams::default(), &pod).await.unwrap();
    
    // Wait a bit for the pod to enter a failed state
    tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
    
    // Run GC
    let svc_api: Api<Service> = Api::namespaced(
        ctx.client.clone(),
        &ctx.config.workshop_namespace
    );
    
    let result = gc::cleanup_idle_pods(
        &pod_api,
        &svc_api,
        &ctx.config.workshop_name,
        3600,
    ).await;
    
    assert!(result.is_ok());
    
    // Wait for deletion
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
    
    // Pod should be cleaned up
    assert!(!ctx.pod_exists("failed-test-pod").await);
    
    ctx.cleanup().await;
}

#[tokio::test]
#[ignore] // Remove this to run with actual k8s cluster
async fn test_gc_only_affects_managed_pods() {
    let ctx = TestContext::new().await;
    ctx.cleanup().await;
    
    let pod_api: Api<Pod> = Api::namespaced(
        ctx.client.clone(),
        &ctx.config.workshop_namespace
    );
    
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
    
    pod_api.create(&kube::api::PostParams::default(), &unmanaged_pod).await.unwrap();
    
    // Wait a moment
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
    
    // Run GC with zero idle threshold
    let svc_api: Api<Service> = Api::namespaced(
        ctx.client.clone(),
        &ctx.config.workshop_namespace
    );
    
    let result = gc::cleanup_idle_pods(
        &pod_api,
        &svc_api,
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
    
    // Clean up the unmanaged pod
    pod_api.delete("unmanaged-test-pod", &kube::api::DeleteParams::default()).await.ok();
    
    ctx.cleanup().await;
}

#[tokio::test]
#[ignore] // Remove this to run with actual k8s cluster
async fn test_gc_handles_missing_health_endpoint() {
    let ctx = TestContext::new().await;
    ctx.cleanup().await;
    
    // Create a pod without a sidecar (no health endpoint)
    let pod_api: Api<Pod> = Api::namespaced(
        ctx.client.clone(),
        &ctx.config.workshop_namespace
    );
    
    let mut labels = BTreeMap::new();
    labels.insert("workshop-hub/user-id".to_string(), "no-sidecar".to_string());
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
    
    pod_api.create(&kube::api::PostParams::default(), &pod).await.unwrap();
    
    // Wait for pod to be running
    ctx.wait_for_pod_running("no-health-pod").await.ok();
    
    // Create a matching service
    let svc_api: Api<Service> = Api::namespaced(
        ctx.client.clone(),
        &ctx.config.workshop_namespace
    );
    
    let service: Service = serde_json::from_value(serde_json::json!({
        "apiVersion": "v1",
        "kind": "Service",
        "metadata": {
            "name": "no-health-pod",
            "labels": labels
        },
        "spec": {
            "selector": {"app": "no-health-pod"},
            "ports": [{
                "name": "health",
                "port": 8080,
                "targetPort": 8080
            }]
        }
    })).unwrap();
    
    svc_api.create(&kube::api::PostParams::default(), &service).await.unwrap();
    
    // Run GC - should delete the pod because health check fails
    let result = gc::cleanup_idle_pods(
        &pod_api,
        &svc_api,
        &ctx.config.workshop_name,
        3600,
    ).await;
    
    assert!(result.is_ok());
    
    // Wait for cleanup
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
    
    // Pod should be deleted due to failed health check
    assert!(!ctx.pod_exists("no-health-pod").await);
    
    ctx.cleanup().await;
}