# Tiltfile - Updated to support integration tests
load('ext://helm_remote', 'helm_remote')

allow_k8s_contexts('admin@talos-local')
hostname = os.getenv("HOSTNAME", "localhost")
default_registry('ghcr.io/nbhdai')
update_settings(max_parallel_updates=5)

# ============================================================================
# Secrets and Base Setup
# ============================================================================
local_resource(
    'secrets',
    labels=['setup'],
    deps=['.envhost'],  
    cmd='./setup/scripts/create-secrets.sh',
)

k8s_yaml('./setup/k8/model-proxy.yaml')
k8s_resource('ai-proxy',
    labels=['setup'],
    resource_deps=['secrets'],
)

# ============================================================================
# Workshop Hub - Core System
# ============================================================================

# Build Hub and Sidecar images
docker_build(
    'workshop-hub',
    '.',
    dockerfile='./crates/hub/Dockerfile',
    only=[
        './crates/hub/',
        './Cargo.toml',
        './Cargo.lock',
    ],
    live_update=[
        sync('./crates/hub/src', '/app/crates/hub/src'),
        run('cd /app && cargo build --release --bin workshop-hub', trigger=['./crates/hub/src']),
    ]
)

docker_build(
    'workshop-sidecar',
    '.',
    dockerfile='./crates/sidecar/Dockerfile',
    only=[
        './crates/sidecar/',
        './Cargo.toml',
        './Cargo.lock',
    ],
    live_update=[
        sync('./crates/sidecar/src', '/app/crates/sidecar/src'),
        run('cd /app && cargo build --release --bin workshop-sidecar', trigger=['./crates/sidecar/src']),
    ]
)

# Deploy Hub infrastructure
k8s_yaml('./workshops/dev-tilt.yaml')

k8s_resource('workshop-hub',
    port_forwards='8080:8080',
    labels=['hub'],
    resource_deps=['secrets'],
)

# ============================================================================
# Integration Tests Infrastructure
# ============================================================================

# Build integration tests image
docker_build(
    'workshop-integration-tests',
    '.',
    dockerfile='./integration-tests.Dockerfile',
    only=[
        './crates/integration-tests/',
        './Cargo.toml',
        './Cargo.lock',
    ],
)

# Deploy integration tests (as a job that can be retriggered)
k8s_yaml('./integration-tests-job.yaml')

# Make the integration tests retriggerable
k8s_resource('workshop-integration-tests',
    labels=['tests'],
    resource_deps=['workshop-hub'],
    trigger_mode=TRIGGER_MODE_MANUAL,  # Don't auto-run, trigger manually
)

# Add a button to run tests
local_resource(
    'run-integration-tests',
    labels=['tests'],
    cmd='kubectl delete job workshop-integration-tests -n default --ignore-not-found && kubectl apply -f integration-tests-job.yaml',
    resource_deps=['workshop-hub'],
    trigger_mode=TRIGGER_MODE_MANUAL,
    auto_init=False,
)

# Add a button to view test logs
local_resource(
    'test-logs',
    labels=['tests'],
    cmd='kubectl logs job/workshop-integration-tests -n default --tail=100',
    resource_deps=['workshop-integration-tests'],
    trigger_mode=TRIGGER_MODE_MANUAL,
    auto_init=False,
)

# ============================================================================
# Helper Commands
# ============================================================================

# Quick test command
local_resource(
    'quick-test',
    labels=['dev'],
    cmd='cargo test --package workshop-hub --lib',
    deps=['./crates/hub/src'],
    trigger_mode=TRIGGER_MODE_MANUAL,
    auto_init=False,
)

# Build all images
local_resource(
    'build-all',
    labels=['dev'],
    cmd='cargo build --release',
    deps=['./crates'],
    trigger_mode=TRIGGER_MODE_MANUAL,
    auto_init=False,
)