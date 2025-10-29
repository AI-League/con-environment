default_registry(
    'ghcr.io/aivillage',
)
update_settings(max_parallel_updates=5)

docker_build(
    'workshop-inspect-basic',
    './workshops/inspect-basic/',
)

k8s_yaml('./workshops/inspect-basic/tilt-service.yaml')
k8s_resource('inspect-basic',
    port_forwards='50051:50051',
    labels=['workshops']
)