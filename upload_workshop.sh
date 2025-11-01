#!/bin/sh

docker build -t ghcr.io/nbhdai/workshop-inspect-basic:latest workshops/inspect-basic
docker push ghcr.io/nbhdai/workshop-inspect-basic:latest