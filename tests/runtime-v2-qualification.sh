#!/usr/bin/env bash
# One bounded executable gate for the inactive runtime-v2 preparation and
# semantic evidence. Every selected Rust test carries its own exact counts.
set -euo pipefail

REPO=$(CDPATH= cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd "$REPO"

# Build every selected test executable while the normal dependency cache is
# still visible. Nothing can change the source between this build and the
# executions below, so the later Cargo calls only select and run these files.
cargo test --locked -q -p pangopup-build --test runtime_release --no-run
cargo test --locked -q -p pangopup-cli --test snv_regression --no-run
cargo test --locked -q -p pangopup-index --test bundle_formats --no-run
cargo test --locked -q -p pangopup-cli --test model_routing --no-run
cargo test --locked -q -p pangopup-cli --all-targets --no-run
cargo test --locked -q -p pangopup-cli --features service-test-fixtures --test http_service_lifecycle --no-run
CARGO_BIN=$(command -v cargo)

# The selected tests can execute pangopup. Give every run this harness's
# private model cache and discard inherited product paths and cache settings.
. "$REPO/tests/support/private-cache-home.sh"

require_one_selection() {
	[ "$1" -eq 1 ]
}

require_one_execution() {
	output=$1
	running=$(printf '%s\n' "$output" | awk '$0 == "running 1 test" { count++ } END { print count + 0 }')
	passed=$(printf '%s\n' "$output" | awk '/^test result: ok\. 1 passed; 0 failed; 0 ignored; 0 measured; [0-9]+ filtered out; finished in / { count++ } END { print count + 0 }')
	[ "$running" -eq 1 ] && [ "$passed" -eq 1 ]
}

# Prove the selector itself does not accept an empty match before trusting it
# to coordinate the Rust filters below.
if require_one_selection 0; then
	echo 'runtime-v2 qualification accepted a zero-test selection' >&2
	exit 1
fi

# A runner can list the requested name and still skip it. Prove the execution
# check rejects that false-positive shape before using it below.
fake_list='named_test: test'
fake_selected=$(printf '%s\n' "$fake_list" | awk -F ': ' -v name='named_test' '$1 == name { count++ } END { print count + 0 }')
fake_output='running 1 test
test named_test ... ignored

test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s'
if ! require_one_selection "$fake_selected" || require_one_execution "$fake_output"; then
	echo 'runtime-v2 qualification execution-count self-test failed' >&2
	exit 1
fi

run_exact() {
	package=$1
	target=$2
	feature=$3
	name=$4
	if [ -n "$feature" ]; then
		list=$("$CARGO_BIN" test --locked -q -p "$package" --features "$feature" --test "$target" -- --list)
	else
		list=$("$CARGO_BIN" test --locked -q -p "$package" --test "$target" -- --list)
	fi
	selected=$(printf '%s\n' "$list" | awk -F ': ' -v name="$name" '$1 == name { count++ } END { print count + 0 }')
	if ! require_one_selection "$selected"; then
		echo "runtime-v2 qualification selected $selected tests for $name" >&2
		exit 1
	fi
	if [ -n "$feature" ]; then
		output=$("$CARGO_BIN" test --locked -q -p "$package" --features "$feature" --test "$target" "$name" -- --exact 2>&1) || {
			printf '%s\n' "$output" >&2
			exit 1
		}
	else
		output=$("$CARGO_BIN" test --locked -q -p "$package" --test "$target" "$name" -- --exact 2>&1) || {
			printf '%s\n' "$output" >&2
			exit 1
		}
	fi
	printf '%s\n' "$output"
	if ! require_one_execution "$output"; then
		echo "runtime-v2 qualification did not execute exactly one passing test for $name" >&2
		exit 1
	fi
}

run_bin_exact() {
	name=$1
	list=$("$CARGO_BIN" test --locked -q -p pangopup-cli --bin pangopup -- --list)
	selected=$(printf '%s\n' "$list" | awk -F ': ' -v name="$name" '$1 == name { count++ } END { print count + 0 }')
	if ! require_one_selection "$selected"; then
		echo "runtime-v2 qualification selected $selected tests for $name" >&2
		exit 1
	fi
	output=$("$CARGO_BIN" test --locked -q -p pangopup-cli --bin pangopup "$name" -- --exact 2>&1) || {
		printf '%s\n' "$output" >&2
		exit 1
	}
	printf '%s\n' "$output"
	if ! require_one_execution "$output"; then
		echo "runtime-v2 qualification did not execute exactly one passing test for $name" >&2
		exit 1
	fi
}

run_exact pangopup-build runtime_release '' runtime_v2_preparation_changes_only_two_transport_members_and_is_deterministic
run_exact pangopup-cli snv_regression '' all_one_thousand_direct_tsv_expectations_pass_one_real_provider
run_exact pangopup-cli snv_regression '' seven_cli_batches_match_the_direct_oracle_subsets
run_exact pangopup-cli snv_regression '' runtime_v2_cross_format_gate_counts_real_provider_and_command_cases
run_exact pangopup-cli snv_regression '' response_identity_normalization_rejects_missing_wrong_shared_and_unrelated_changes
run_exact pangopup-index bundle_formats '' fixed_and_sparse_bundles_have_identical_provider_answers
run_exact pangopup-cli model_routing '' hit_only_missing_assets_model_required_grammar_and_rejection_are_stable
run_exact pangopup-cli model_routing '' component_and_scoring_failures_are_redacted_and_transactional
run_bin_exact service::tests::all_precomputed_status_shapes_carry_the_service_identity
run_exact pangopup-cli http_service_lifecycle service-test-fixtures installed_success::runtime_v2_reachable_service_routing_differs_only_by_derived_identities
run_exact pangopup-cli http_service_lifecycle service-test-fixtures installed_success::real_executable_preserves_valid_result_beside_model_rejection
run_exact pangopup-cli http_service_lifecycle service-test-fixtures installed_success::status_publishes_a_recomputable_data_set_version

printf '%s\n' 'runtime-v2 qualification: 1000 requests; 7 nonempty groups; real providers and commands; routing, serializer, HTTP, and identities passed'
