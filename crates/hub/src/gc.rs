use k8s_openapi::api::core::v1::Pod;
use kube::api::{Api, DeleteParams, ListParams};
use serde::Deserialize;
use tracing::{info, warn};

use crate::config::{LABEL_WORKSHOP_NAME, TTL_ANNOTATION};

#[derive(Deserialize, Debug)]
struct SidecarHealth {
    status: String,
    last_activity_timestamp: u64,
    idle_seconds: u64,
}

/// Iterates through all managed pods and cleans up idle ones.
pub async fn cleanup_idle_pods(
    pod_api: &Api<Pod>,
    workshop_name: &str,
    max_idle_seconds: u64,
) -> Result<(), crate::HubError> {
    let list_params = ListParams::default().labels(&format!(
        "{}={},{}={}",
        "app.kubernetes.io/managed-by", "workshop-hub", LABEL_WORKSHOP_NAME, workshop_name
    ));

    let pods = pod_api.list(&list_params).await?;
    let client = reqwest::Client::new();

    if pods.items.is_empty() {
        info!("GC: No managed pods found.");
        return Ok(());
    }

    info!("GC: Checking {} managed pods...", pods.items.len());

    // Extract namespace from the Api - this is what the Api is namespaced to
    let namespace = pod_api.namespace().ok_or(crate::HubError::NamespaceMissing)?;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| crate::HubError::InternalError("System time error".to_string()))?
        .as_secs();

    for pod in pods.items {
        let pod_name = pod.metadata.name.as_deref().unwrap_or_default();
        if pod_name.is_empty() {
            continue;
        }

        // The service name is assumed to match the pod name
        let service_name = pod_name;

        // --- TTL Check ---
        // Check for TTL expiration first
        if let Some(annotations) = &pod.metadata.annotations {
            if let Some(expires_at_str) = annotations.get(TTL_ANNOTATION) {
                if let Ok(expires_at) = expires_at_str.parse::<u64>() {
                    if now > expires_at {
                        info!("GC: Pod {} has exceeded its max TTL. Deleting.", pod_name);
                        pod_api.delete(pod_name, &DeleteParams::default()).await?;
                        continue; // Move to next pod
                    }
                }
            }
        }

        // --- State Check ---
        // Pods in Pending/Failed/Succeeded state should be checked
        let phase = pod.status.as_ref().and_then(|s| s.phase.as_deref());
        if phase != Some("Running") {
            warn!("GC: Found non-Running pod {}. Deleting.", pod_name);
            // Service is auto-deleted via OwnerReference, just delete pod
            pod_api.delete(pod_name, &DeleteParams::default()).await?;
            continue;
        }

        // Pod is running, check its health endpoint
        // Connect to the service's "health" port using the namespace from the Api
        let health_url = format!(
            "http://{}.{}.svc.cluster.local:8080/health",
            service_name, namespace
        );

        match client
            .get(&health_url)
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await
        {
            Ok(response) => {
                if !response.status().is_success() {
                    warn!(
                        "GC: Health check for {} failed (status: {}). Deleting.",
                        pod_name,
                        response.status()
                    );
                    pod_api.delete(pod_name, &DeleteParams::default()).await?;
                    continue;
                }

                match response.json::<SidecarHealth>().await {
                    Ok(health) => {
                        info!("GC: Pod {} idle for {}s", pod_name, health.idle_seconds);
                        if health.idle_seconds > max_idle_seconds {
                            info!("GC: Pod {} exceeded idle time. Deleting.", pod_name);
                            pod_api.delete(pod_name, &DeleteParams::default()).await?;
                        }
                    }
                    Err(e) => {
                        warn!(
                            "GC: Failed to parse health from {}: {}. Deleting.",
                            pod_name, e
                        );
                        pod_api.delete(pod_name, &DeleteParams::default()).await?;
                    }
                }
            }
            Err(e) => {
                warn!(
                    "GC: Health check request for {} failed: {}. Deleting.",
                    pod_name, e
                );
                pod_api.delete(pod_name, &DeleteParams::default()).await?;
            }
        }
    }

    Ok(())
}