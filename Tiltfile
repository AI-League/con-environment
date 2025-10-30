allow_k8s_contexts('admin@talos-local')
hostname = os.getenv("HOSTNAME", "localhost")
default_registry(
    'ghcr.io/nbhdai',
)
update_settings(max_parallel_updates=5)

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

docker_build(
    'workshop-inspect-basic',
    './workshops/inspect-basic/',
)

k8s_yaml('./workshops/inspect-basic/tilt-service.yaml')
k8s_resource('inspect-basic',
    port_forwards='50051:8080',
    labels=['workshops'],
    resource_deps=['ai-proxy'],
)