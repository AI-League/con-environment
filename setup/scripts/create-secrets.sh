#!/bin/bash

# Script to create Kubernetes secrets from environment variables
# This ensures secrets exist before services start

set -e

# Load environment variables from .env file if it exists
if [ -f .env ]; then
    export $(cat .envhost | grep -v '^#' | xargs)
fi

# Function to create or update a secret
create_or_update_secret() {
    local secret_name=$1
    shift
    local args="$@"
    
    if kubectl get secret $secret_name > /dev/null 2>&1; then
        echo "Updating secret: $secret_name"
        kubectl delete secret $secret_name
    else
        echo "Creating secret: $secret_name"
    fi
    
    kubectl create secret generic $secret_name $args
}

# Create API keys secret
create_or_update_secret api-keys \
    --from-literal=GEMINI_API_KEY="${GEMINI_API_KEY:-}" \
    --from-literal=OPENAI_API_KEY="${OPENAI_API_KEY:-}" \
    --from-literal=ANTHROPIC_API_KEY="${ANTHROPIC_API_KEY:-}"

echo "✅ All secrets created/updated successfully"