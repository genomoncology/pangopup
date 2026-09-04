#!/usr/bin/env bash
set -euo pipefail

repository=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
workflow="$repository/.github/workflows/ci.yml"

require_text() {
    local text=$1
    local content=$2
    local description=$3
    case "$content" in
        *"$text"*) ;;
        *)
            printf 'missing %s: %s\n' "$description" "$text" >&2
            exit 1
            ;;
    esac
}

compiler_step=$(sed -n '/^      - name: Install the Linux ARM64 cross compiler$/,/^      - name: Install uv 0.8.0$/p' "$workflow")
require_text 'sudo apt-get update' "$compiler_step" 'Linux package index update'
require_text 'sudo apt-get install --yes gcc-aarch64-linux-gnu' "$compiler_step" 'Linux ARM64 cross compiler installation'

arm_step=$(sed -n '/^      - name: Check the Linux ARM64 command build$/,/^      - name: Test the portable native service fixture$/p' "$workflow")
require_text 'CC_aarch64_unknown_linux_gnu: aarch64-linux-gnu-gcc' "$arm_step" 'bundled C dependency compiler'
require_text 'CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER: aarch64-linux-gnu-gcc' "$arm_step" 'Rust target linker'
require_text 'run: cargo check --locked --target aarch64-unknown-linux-gnu --package pangopup-cli' "$arm_step" 'Linux ARM64 command build'

model_routing="$repository/crates/pangopup-cli/tests/model_routing.rs"
if grep -Fq '#[cfg(target_os = "linux")]' "$model_routing"; then
    printf 'portable model-routing tests remain gated to Linux\n' >&2
    exit 1
fi
