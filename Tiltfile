# Tiltfile - Using nix-flake for image builds
load('ext://helm_remote', 'helm_remote')
load('ext://nix_flake', 'build_flake_image')

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
# Workshop Hub - Core System (Built with Nix)
# ============================================================================

# Build Hub image using nix flake
build_flake_image(
    'workshop-hub',
    '.',  # Path to flake.nix (current directory)
    'workshop-hub',  # Output name from flake
    deps=[
        './crates/hub/src',
        './crates/hub/Cargo.toml',
        './Cargo.lock',
    ]
)

# Build Sidecar image using nix flake
build_flake_image(
    'workshop-sidecar',
    '.',
    'workshop-sidecar',
    deps=[
        './crates/sidecar/src',
        './crates/sidecar/Cargo.toml',
        './Cargo.lock',
    ]
)

# Build Integration Tests image using nix flake
build_flake_image(
    'workshop-integration-tests',
    '.',
    'workshop-integration-tests',
    deps=[
        './crates/integration-tests/src',
        './crates/integration-tests/Cargo.toml',
        './Cargo.lock',
    ]
)

# Deploy Hub infrastructure
k8s_yaml('./setup/k8/workshop.yaml')

k8s_resource('workshop-hub',
    port_forwards='8080:8080',
    labels=['hub'],
    resource_deps=['ai-proxy', 'workshop-redis'],
)

# # ============================================================================
# # Workshop dev
# # ============================================================================

docker_build("workshop-inspect-basic", "./workshops/inspect-basic")
k8s_yaml('./workshops/inspect-basic/tilt-service.yaml')

k8s_resource('inspect-basic',
    port_forwards='8085:8080',
    labels=['hub'],
    resource_deps=['ai-proxy', 'workshop-redis'],
)


# # ============================================================================
# # Integration Tests Infrastructure
# # ============================================================================

# # Deploy integration tests (as a job that can be retriggered)
# k8s_yaml('crates/integration-tests/config.yaml')

# # Make the integration tests retriggerable
# k8s_resource('workshop-integration-tests',
#     labels=['tests'],
#     resource_deps=['workshop-hub'],
#     trigger_mode=TRIGGER_MODE_MANUAL,  # Don't auto-run, trigger manually
# )

# # Add a button to run tests
# local_resource(
#     'run-integration-tests',
#     labels=['tests'],
#     cmd='kubectl delete job workshop-integration-tests -n default --ignore-not-found && kubectl apply -f integration-tests-job.yaml',
#     resource_deps=['workshop-hub'],
#     trigger_mode=TRIGGER_MODE_MANUAL,
#     auto_init=False,
# )

# # Add a button to view test logs
# local_resource(
#     'test-logs',
#     labels=['tests'],
#     cmd='kubectl logs job/workshop-integration-tests -n default --tail=100 || echo "No logs available yet"',
#     resource_deps=['workshop-integration-tests'],
#     trigger_mode=TRIGGER_MODE_MANUAL,
#     auto_init=False,
# )
