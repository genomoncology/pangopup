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

normalize_markdown() {
    awk '
        {
            gsub(/[[:space:]]+/, " ")
            sub(/^ /, "")
            sub(/ $/, "")
            if (length($0) > 0) {
                if (seen) {
                    printf " "
                }
                printf "%s", $0
                seen = 1
            }
        }
        END {
            if (seen) {
                printf "\n"
            }
        }
    '
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

adr_0004_original='The shipped Linux installer streams that transport into an immutable XDG-data bundle, atomically selects it, and reuses it with cheap structural validation.'
adr_0004_supersession='Supersession note (2026-09-04): Ticket 0003 extended the shipped local installer to native macOS. The original Linux installer sentence below remains as history.'

check_adr_0004() {
    local content=$1
    require_text "$adr_0004_original" "$content" 'ADR 0004 original installer decision'
    require_text "$adr_0004_supersession" "$content" 'ADR 0004 macOS supersession note'
}

adr_0004=$(normalize_markdown <"$repository/architecture/decisions/0004-speed-first-runtime-release-assets.md")
check_adr_0004 "$adr_0004"

adr_0004_prefix=${adr_0004%%"$adr_0004_original"*}
adr_0004_suffix=${adr_0004#*"$adr_0004_original"}
adr_0004_mutated="${adr_0004_prefix}The shipped Linux placeholder.${adr_0004_suffix}"
if (check_adr_0004 "$adr_0004_mutated" >/dev/null 2>&1); then
    printf 'ADR 0004 guard accepted a placeholder for the original installer decision\n' >&2
    exit 1
fi
