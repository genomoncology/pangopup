#!/usr/bin/env bash
set -euo pipefail
repo=$(cd "$(dirname "$0")/.." && pwd)
python3 "$repo/tests/container-production-model-oracle.py"
