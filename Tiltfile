allow_k8s_contexts('admin@talos-local')
hostname = os.getenv("HOSTNAME", "localhost")
default_registry(
    'ghcr.io/nbhdai',
)
update_settings(max_parallel_updates=5)

k8s_yaml('./setup/k8/model-proxy.yaml')
k8s_resource('ai-proxy',
    labels=['setup']
)

docker_build(
    'workshop-inspect-basic',
    './workshops/inspect-basic/',
)

k8s_yaml('./workshops/inspect-basic/tilt-service.yaml')
k8s_resource('inspect-basic',
    port_forwards='50051:50051',
    labels=['workshops']
)