load('ext://helm_remote', 'helm_remote')
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

# Making sure it works in jupyterhub
k8s_yaml('./workshops/inspect-basic/cilium-policies.yaml')
helm_remote('jupyterhub',
    repo_name='jupyterhub',
    repo_url='https://hub.jupyter.org/helm-chart/',
    values='./workshops/inspect-basic/lab-service.yaml'
)
k8s_resource('hub',
    port_forwards='8081:8081',
    labels=['workshops'],
    resource_deps=['ai-proxy'],
)

# Developing it quickly
docker_build(
    'workshop-inspect-basic',
    './workshops/inspect-basic/',
)
k8s_yaml('./workshops/inspect-basic/tilt-service.yaml')
k8s_resource('inspect-basic',
    port_forwards='50505:8080',
    labels=['workshops'],
    resource_deps=['ai-proxy'],
)