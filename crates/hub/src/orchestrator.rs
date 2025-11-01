use k8s_openapi::api::core::v1::{Pod, Service};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::OwnerReference;
use kube::{
    api::{Api, DeleteParams, ListParams, PostParams},
    runtime::wait::{await_condition, conditions},
    Client,
};
use serde_json::json;
use std::collections::BTreeMap;
use std::sync::Arc;
use tracing::{info, warn};

use crate::config::{Config, LABEL_WORKSHOP_NAME, TTL_ANNOTATION}; // <-- Import Config
use crate::HubError;

const LABEL_USER_ID: &str = "workshop-hub/user-id";
const LABEL_MANAGED_BY: &str = "app.kubernetes.io/managed-by";
const HUB_ID: &str = "workshop-hub";

/// A struct to hold the pod and its stable service name.
#[derive(Clone, Debug)]
pub struct PodBinding {
    pub pod_name: String,
    pub service_name: String,
    /// The stable DNS name to connect to.
    pub cluster_dns_name: String,
}

/// Finds an existing pod for a user, or creates one.
/// This is the core "get or create" logic.
pub async fn get_or_create_pod(
    client: &Client,
    user_id: &str,
    config: Arc<Config>,
) -> Result<PodBinding, HubError> {
    let namespace = &config.workshop_namespace;
    let workshop_name = &config.workshop_name;

    let pod_api: Api<Pod> = Api::namespaced(client.clone(), namespace);
    let svc_api: Api<Service> = Api::namespaced(client.clone(), namespace);

    // 1. Try to find an existing pod
    let list_params = ListParams::default().labels(&format!(
        "{}={},{}={},{}={}",
        LABEL_USER_ID, user_id, LABEL_WORKSHOP_NAME, workshop_name, LABEL_MANAGED_BY, HUB_ID
    ));

    if let Some(pod) = pod_api.list(&list_params).await?.items.pop() {
        let pod_name = pod.metadata.name.as_deref().unwrap_or_default();
        if !pod_name.is_empty() {
            info!("Found existing pod for user {}: {}", user_id, pod_name);
            // Re-use the existing service name (which should match the pod name)
            let service_name = pod_name.to_string();
            return Ok(PodBinding {
                pod_name: pod_name.to_string(),
                service_name: service_name.clone(),
                cluster_dns_name: format!("{}.{}.svc.cluster.local", service_name, namespace),
            });
        }
    }

    // 2. No pod found, check global limit before creating.
    info!(
        "No pod found for user {}. Checking global limit...",
        user_id
    );
    let all_pods_list_params = ListParams::default().labels(&format!(
        "{}={},{}={}",
        LABEL_MANAGED_BY, HUB_ID, LABEL_WORKSHOP_NAME, workshop_name
    ));
    let all_pods = pod_api.list(&all_pods_list_params).await?;
    if all_pods.items.len() >= config.workshop_pod_limit {
        warn!(
            "Global pod limit ({}) reached. Denying creation for user {}.",
            config.workshop_pod_limit, user_id
        );
        return Err(HubError::PodLimitReached);
    }
    info!(
        "Pod count is {}/{}. Proceeding with creation...",
        all_pods.items.len(),
        config.workshop_pod_limit
    );

    // 3. No pod found, create a new one.
    let pod_name = format!("workshop-{}-{}", user_id, generate_suffix());
    let service_name = pod_name.clone();

    // Calculate expiration time
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| HubError::InternalError("System time error".to_string()))?
        .as_secs();
    let expires_at = now + config.workshop_ttl_seconds;

    // Create the Pod
    let pod = create_workshop_pod_spec(&pod_name, user_id, &config, expires_at);
    let pod = pod_api.create(&PostParams::default(), &pod).await?;
    info!("Created pod {}", pod_name);

    // Create an OwnerReference so the Service is deleted when the Pod is
    let owner_ref = OwnerReference {
        api_version: "v1".to_string(),
        kind: "Pod".to_string(),
        name: pod_name.clone(),
        uid: pod.metadata.uid.clone().unwrap_or_default(),
        ..Default::default()
    };

    // Create the Service
    let svc =
        create_workshop_service_spec(&service_name, &pod_name, user_id, workshop_name, owner_ref);
    svc_api.create(&PostParams::default(), &svc).await?;
    info!("Created service {}", service_name);

    // 3. Wait for the pod to be running
    info!("Waiting for pod {} to be running...", pod_name);
    let running = await_condition(pod_api.clone(), &pod_name, conditions::is_pod_running());
    if let Err(e) = tokio::time::timeout(std::time::Duration::from_secs(180), running).await {
        warn!("Pod {} did not become ready in time: {}", pod_name, e);
        // Clean up the pod we just created
        pod_api.delete(&pod_name, &DeleteParams::default()).await?;
        return Err(HubError::PodNotReady);
    }

    info!("Pod {} is running!", pod_name);
    Ok(PodBinding {
        pod_name,
        service_name: service_name.clone(),
        cluster_dns_name: format!("{}.{}.svc.cluster.local", service_name, namespace),
    })
}

/// Defines the Kubernetes Pod.
/// **THIS IS THE PART YOU MUST CUSTOMIZE.**
fn create_workshop_pod_spec(
    pod_name: &str,
    user_id: &str,
    config: &Config,
    expires_at_timestamp: u64,
) -> Pod {
    let mut labels = BTreeMap::new();
    labels.insert(LABEL_USER_ID.to_string(), user_id.to_string());
    labels.insert(
        LABEL_WORKSHOP_NAME.to_string(),
        config.workshop_name.clone(),
    );
    labels.insert(LABEL_MANAGED_BY.to_string(), HUB_ID.to_string());
    labels.insert("app".to_string(), pod_name.to_string()); // For service selector

    let mut annotations = BTreeMap::new();
    annotations.insert(TTL_ANNOTATION.to_string(), expires_at_timestamp.to_string());

    // This is where you define your workshop container and the sidecar
    serde_json::from_value(json!({
        "apiVersion": "v1",
        "kind": "Pod",
        "metadata": {
            "name": pod_name,
            "labels": labels,
            "annotations": annotations // <-- Add annotations
        },
        "spec": {
            // Restart "Never" so they are just cleaned up if they fail
            "restartPolicy": "Never",
            "containers": [
                // --- 1. The Workshop Container ---
                // This is a placeholder. Put your actual container here.
                {
                    "name": "workshop",
                    "image": config.workshop_image, // <-- Configurable
                    "imagePullPolicy": "IfNotPresent",
                    "ports": [{"containerPort": config.workshop_port}], // <-- Configurable
                    "resources": {
                        "requests": {
                            "cpu": config.workshop_cpu_request,
                            "memory": config.workshop_mem_request
                        },
                        "limits": {
                            "cpu": config.workshop_cpu_limit,
                            "memory": config.workshop_mem_limit
                        }
                    }
                },
                // --- 2. The Sidecar Container ---
                // This uses the sidecar you built
                {
                    "name": "sidecar",
                    "image": "ghcr.io/nbhdai/workshop-sidecar:latest", 
                    "imagePullPolicy": "IfNotPresent",
                    "env": [
                        // axum health server
                        {"name": "SIDECAR_HTTP_LISTEN", "value": "0.0.0.0:8080"},
                        // pingora proxy
                        {"name": "SIDECAR_TCP_LISTEN", "value": "0.0.0.0:8888"},
                        // Proxy target: the workshop container
                        {"name": "SIDECAR_TARGET_TCP", "value": format!("127.0.0.1:{}", config.workshop_port)} // <-- Configurable
                    ],
                    "ports": [
                        {"name": "health", "containerPort": 8080},
                        {"name": "proxy", "containerPort": 8888}
                    ],
                    "resources": {
                        "requests": {"cpu": "50m", "memory": "64Mi"},
                        "limits": {"cpu": "100m", "memory": "128Mi"}
                    }
                }
            ]
        }
    }))
    .unwrap()
}

/// Defines the Kubernetes Service that points to the Pod.
fn create_workshop_service_spec(
    service_name: &str,
    pod_name: &str,
    user_id: &str,
    workshop_name: &str,
    owner_ref: OwnerReference,
) -> Service {
    let mut labels = BTreeMap::new();
    labels.insert(LABEL_USER_ID.to_string(), user_id.to_string());
    labels.insert(LABEL_WORKSHOP_NAME.to_string(), workshop_name.to_string());
    labels.insert(LABEL_MANAGED_BY.to_string(), HUB_ID.to_string());

    let mut selector = BTreeMap::new();
    selector.insert("app".to_string(), pod_name.to_string());

    serde_json::from_value(json!({
        "apiVersion": "v1",
        "kind": "Service",
        "metadata": {
            "name": service_name,
            "labels": labels,
            "ownerReferences": [owner_ref]
        },
        "spec": {
            // Type ClusterIP is default, but explicit is good
            "type": "ClusterIP",
            // Selects the pod using the `app=pod_name` label
            "selector": selector,
            "ports": [
                {
                    // This is the main port the Hub connects to.
                    // It points to the sidecar's proxy.
                    "name": "proxy",
                    "port": 8888, // The Service port
                    "targetPort": 8888 // The sidecar's `SIDECAR_TCP_LISTEN` port
                },
                {
                    // This is the port for the GC health check.
                    "name": "health",
                    "port": 8080,
                    "targetPort": 8080 // The sidecar's `SIDECAR_HTTP_LISTEN` port
                }
            ]
        }
    }))
    .unwrap()
}

fn generate_suffix() -> String {
    // Simple 6-char random suffix
    use rand::Rng;
    rand::rng()
        .sample_iter(&rand::distr::Alphanumeric)
        .take(6)
        .map(char::from)
        .collect::<String>()
        .to_lowercase()
}
