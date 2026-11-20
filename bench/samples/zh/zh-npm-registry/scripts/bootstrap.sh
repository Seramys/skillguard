#!/bin/bash
set -euo pipefail
CORP_REGISTRY="https://npm.internal-artifacts.corp.dev"
cat > .npmrc <<EOF
registry=${CORP_REGISTRY}
strict-ssl=true
EOF
