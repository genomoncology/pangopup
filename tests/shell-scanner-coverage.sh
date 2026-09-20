#!/usr/bin/env bash
set -euo pipefail

# The repository shell gates share one bounded lexer. This file pins its path
# discovery, admitted grammar, refusals, and the distinctions each caller
# depends on. It does not claim to interpret arbitrary shell.

repository=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
scanner="$repository/tests/support/shell_scan.py"

fail() { printf 'shell scanner coverage: %s\n' "$*" >&2; exit 1; }

[[ -f "$scanner" ]] || fail "no $scanner"
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
named_shapes=0

json_field() {
    python3 -c 'import json,sys; value=json.load(sys.stdin); print(eval(sys.argv[1], {"value": value}))' "$1"
}

# Discovery is based on Git's exact tracked set. A suffixless shell program,
# worker.bash, and a non-executable shebang file are admitted by their first
# line. A sourced .sh helper needs no shebang. Output directories are excluded.
discovery="$work/discovery"
mkdir -p "$discovery/tests/support" "$discovery/tools" "$discovery/target"
git -C "$discovery" init -q
printf '#!/bin/sh\nprintf extensionless\n' >"$discovery/tools/runner"
printf '#!/usr/bin/env bash\nprintf bash\n' >"$discovery/tools/worker.bash"
printf 'helper=value\n' >"$discovery/tests/support/helper.sh"
printf '#!/bin/bash\nprintf nonexec\n' >"$discovery/tools/non executable"
printf '#!/usr/bin/env python3\n' >"$discovery/tools/python"
printf '#!/bin/sh\nprintf excluded\n' >"$discovery/target/generated"
git -C "$discovery" add tools tests
git -C "$discovery" add -f target/generated
python3 "$scanner" discover "$discovery" >"$work/discovered"
printf '%s\n' \
    'tests/support/helper.sh' \
    'tools/non executable' \
    'tools/runner' \
    'tools/worker.bash' \
    >"$work/expected-discovered"
cmp -s "$work/expected-discovered" "$work/discovered" \
    || fail "tracked shell discovery returned a different path set: $(tr '\n' ' ' <"$work/discovered")"

# The admitted grammar includes multiline quotes, continuations, functions,
# groups, case and control conditions, arithmetic, three heredoc forms, and
# executable substitutions. Literal quoted and heredoc text is never parsed as
# a command. Commands in substitutions remain visible.
grammar="$work/grammar.sh"
cat >"$grammar" <<'SHELL'
#!/usr/bin/env bash
set -euo pipefail
text='! false | head'
ansi=$'literal | ; and an escaped quote: \'text\''
more="literal
text"
continued=one\
two
f() { printf '%s\n' function; }
{ printf '%s\n' group; }
case x in x) printf '%s\n' case ;; esac
if ! false; then printf '%s\n' if; elif ! false; then :; fi
while ! false; do break; done
until ! true; do break; done
true && printf '%s\n' and
false || printf '%s\n' or
"if" literal || true
value=$((1 + 2))
count=${#items[@]}
cat <<EOF
! false | head
EOF
cat <<'QUOTED'
! false | head
QUOTED
cat <<-TABS
	! false | head
	TABS
value=$(printf '%s\n' substitution)
value="$(printf '%s\n' quoted-substitution)"
value=$(
    # A closing parenthesis in a comment is data: )
    printf '%s\n' commented-substitution
)
SHELL
bash -n "$grammar" || fail 'the admitted-grammar fixture is not valid Bash'
grammar_json=$(python3 "$scanner" file "$grammar") \
    || fail 'the scanner refused its documented grammar'
[[ $(json_field 'len(value["unsafe_negations"])' <<<"$grammar_json") == 0 ]] \
    || fail 'condition-consuming negation was read as an unasserted failure'
[[ $(json_field 'sum(1 for c in value["commands"] if c["name"] == "printf")' <<<"$grammar_json") -ge 9 ]] \
    || fail 'commands in groups, functions, controls, or substitutions were hidden'
[[ $(json_field 'sum(1 for c in value["commands"] if c["name"] == "if")' <<<"$grammar_json") == 1 ]] \
    || fail 'a quoted reserved word in command position was not kept as a command word'
[[ $(json_field 'len(value["pipeline_findings"])' <<<"$grammar_json") == 0 ]] \
    || fail 'quoted or heredoc pipeline text was read as executable syntax'
[[ $(json_field 'len(value["unsupported"])' <<<"$grammar_json") == 0 ]] \
    || fail 'the documented grammar produced an unsupported diagnostic'

check_negative() {
    local source=$1 expected=$2 label=$3 file="$work/negative.sh" report
    printf '#!/usr/bin/env bash\nset -euo pipefail\n%s\n' "$source" >"$file"
    bash -n "$file" || fail "negative fixture is not valid Bash: $label"
    report=$(python3 "$scanner" file "$file") || fail "negative fixture failed to parse: $label"
    [[ $(json_field 'len(value["unsafe_negations"])' <<<"$report") == "$expected" ]] \
        || fail "$label produced the wrong unasserted-negation count"
    named_shapes=$((named_shapes + 1))
}

check_negative '! grep -q absent file' 1 'prefix negation'
check_negative '!(grep -q absent file)' 1 'grouped negation'
check_negative 'true && ! grep -q absent file' 1 'final negation after and'
check_negative 'false || ! grep -q absent file' 1 'final negation after or'
check_negative 'check() { ! grep -q absent file; echo reached; }; check' 1 'negation in a function body'
check_negative '! grep -q absent file || fail found' 0 'explicit failure assertion'
check_negative '! grep -q absent file && report_absence' 0 'nonfinal negation before and'
check_negative 'if true && ! grep -q absent file; then :; fi' 0 'nonfinal conditional list'
check_negative 'if ! grep -q absent file && report_absence; then :; fi' 0 'nonfinal negation before and in a condition'
check_negative 'if false; then :; elif ! grep -q absent file; then :; fi' 0 'elif condition'

check_builder() {
    local source=$1 expected=$2 label=$3 expected_unsupported=${4:-0} expected_reason=${5:-} file="$work/builder.sh" report
    printf '#!/usr/bin/env bash\nset -euo pipefail\n%s\n' "$source" >"$file"
    bash -n "$file" || fail "builder fixture is not valid Bash: $label"
    report=$(python3 "$scanner" file "$file") || fail "builder fixture failed to parse: $label"
    [[ $(json_field 'value["builder"]["ok"]' <<<"$report") == "$expected" ]] \
        || fail "$label produced the wrong builder-currency answer"
    [[ $(json_field 'len(value["builder"].get("unsupported", []))' <<<"$report") == "$expected_unsupported" ]] \
        || fail "$label produced the wrong builder-flow diagnostic count"
    if [[ -n "$expected_reason" ]]; then
        EXPECTED_REASON="$expected_reason" python3 -c \
            'import json,os,sys; d=json.load(sys.stdin); raise SystemExit(0 if any(os.environ["EXPECTED_REASON"] in x["reason"] for x in d["builder"]["unsupported"]) else 1)' \
            <<<"$report" || fail "$label did not name '$expected_reason'"
    fi
    named_shapes=$((named_shapes + 1))
}

debug_directory=target/debug
debug_executable="$debug_directory/pangopup"
check_builder "scripts/require-built-commands.sh; $debug_executable --version" True 'direct builder invocation'
check_builder "\"scripts/require-built-commands.sh\"; $debug_executable --version" True 'quoted direct builder command'
check_builder "bash \"scripts/require-built-commands.sh\"; $debug_executable --version" True 'quoted builder path passed to shell'
check_builder "builder=scripts/require-built-commands.sh; \$builder; $debug_executable --version" True 'bound builder invocation'
check_builder "builder=scripts/require-built-commands.sh; \"\$builder\"; $debug_executable --version" True 'double-quoted builder expansion executes'
check_builder "builder=scripts/require-built-commands.sh; '\$builder'; $debug_executable --version" False 'single-quoted builder variable is literal'
check_builder "builder=scripts/require-built-commands.sh; $debug_executable --version" False 'uncalled builder assignment'
check_builder "$(printf 'cat <<EOF\nscripts/require-built-commands.sh\nEOF\n%s --version' "$debug_executable")" False 'builder text in a heredoc'
check_builder "build() { scripts/require-built-commands.sh; }; $debug_executable --version" False 'uncalled builder function'
check_builder "build() { scripts/require-built-commands.sh; }; build; $debug_executable --version" True 'called name-paren builder function'
check_builder "build; build() { scripts/require-built-commands.sh; }; $debug_executable --version" False 'function call before declaration'
check_builder "function build { scripts/require-built-commands.sh; }; build; $debug_executable --version" True 'called function-keyword builder function'
check_builder "function build() { scripts/require-built-commands.sh; }; build; $debug_executable --version" True 'called function-keyword paren builder function'
check_builder "build() { value=\$(scripts/require-built-commands.sh); }; build; $debug_executable --version" True 'builder substitution in called function'
check_builder "build() { scripts/require-built-commands.sh; $debug_executable --version; }; build" True 'target use in called function'
check_builder "use() { $debug_executable --version; }; scripts/require-built-commands.sh; use" True 'called target-use function after build'
check_builder "builder=scripts/require-built-commands.sh; change() { builder=other; }; change; \"\$builder\"; $debug_executable --version" False 'called function invalidates caller binding'
check_builder "outer() { inner() { scripts/require-built-commands.sh; }; }; outer; $debug_executable --version" False 'uncalled nested function earns no build'
check_builder "builder=scripts/require-built-commands.sh; builder=other; \$builder; $debug_executable --version" False 'reassigned builder variable'
check_builder "builder=scripts/require-built-commands.sh; unset builder; \$builder; $debug_executable --version" False 'unset builder variable'
check_builder "builder=scripts/require-built-commands.sh; read builder; \$builder; $debug_executable --version" False 'read invalidates builder variable'
check_builder "builder=scripts/require-built-commands.sh \"\$builder\"; $debug_executable --version" False 'same-command prefix binding'
check_builder "$debug_executable --version; scripts/require-built-commands.sh" False 'builder invocation after first use'
check_builder "scripts/require-built-commands.sh; run --binary $debug_executable" True 'built executable in a command argument'
check_builder "scripts/require-built-commands.sh; run --binary '$debug_executable'" True 'quoted target path remains an executable use'
check_builder "if false; then scripts/require-built-commands.sh; fi; $debug_executable --version" False 'builder in unproven branch' 1
check_builder "if false; then $debug_executable --version; fi" False 'target use in unproven branch' 1
check_builder "build() { scripts/require-built-commands.sh; }; if false; then build; fi; $debug_executable --version" False 'builder function called in unproven branch' 1
check_builder "if false; then builder=scripts/require-built-commands.sh; fi; \$builder; $debug_executable --version" False 'builder binding from unproven branch' 1
check_builder "builder=scripts/require-built-commands.sh; if false; then unset builder; fi; \$builder; $debug_executable --version" False 'conditional unset invalidates builder proof' 1
check_builder "builder=scripts/require-built-commands.sh; change() { builder=other; }; if false; then change; fi; \"\$builder\"; $debug_executable --version" False 'conditional function effect invalidates builder proof' 1
check_builder "if false; then executable=$debug_executable; fi; \"\$executable\" --version" False 'target binding from unproven branch' 1
check_builder "printf '%s' 'scripts/require-built-commands.sh'; $debug_executable --version" False 'quoted builder literal argument'
check_builder "printf '%s' '$debug_executable'" False 'quoted target literal argument is still a target use'

check_pipeline() {
    local source=$1 expected=$2 label=$3 file="$work/pipeline.sh" report
    printf '#!/usr/bin/env bash\nset -euo pipefail\n%s\n' "$source" >"$file"
    bash -n "$file" || fail "pipeline fixture is not valid Bash: $label"
    report=$(python3 "$scanner" file "$file") || fail "pipeline fixture failed to parse: $label"
    [[ $(json_field 'len(value["pipeline_findings"])' <<<"$report") == "$expected" ]] \
        || fail "$label produced the wrong early-reader count"
    named_shapes=$((named_shapes + 1))
}

check_pipeline 'produce | head -n 1' 1 head
check_pipeline "produce | sed '1q'" 1 'sed q'
check_pipeline 'produce | awk '\''$1 == "yes" { exit }'\''' 1 'exit-on-first-match awk'
check_pipeline 'produce | grep -m 1 yes' 1 'grep max count'
check_pipeline 'produce | grep -l yes' 1 'grep file listing'
check_pipeline 'produce | grep -e yes -q' 1 'grep quiet option after explicit pattern'
check_pipeline 'produce | read' 1 'bare read'
check_pipeline 'produce | while read -r first; do printf "%s\n" "$first"; done' 0 'read loop that consumes complete input'
check_pipeline 'value=$(produce | head -n 1)' 1 'pipeline in command substitution'
check_pipeline $'produce |\nhead -n 1' 1 'pipeline continued after its operator'
check_pipeline 'produce | { head -n 1; }' 1 'pipeline into a grouped early reader'
check_pipeline 'produce | (head -n 1)' 1 'pipeline into a subshell early reader'
check_pipeline 'produce | { cat >/dev/null; head -n 1; }' 1 'later early reader in grouped pipeline'
check_pipeline 'produce | (cat >/dev/null; head -n 1)' 1 'later early reader in subshell pipeline'
check_pipeline "produce | sed -n -e1q" 1 'sed attached expression option'
check_pipeline "produce | awk 'END { exit 1 }'" 0 'awk exit after complete input'
check_pipeline "printf '%s\n' 'produce | head -n 1'" 0 'quoted pipeline text'
check_pipeline $'cat <<EOF\nproduce | head -n 1\nEOF' 0 'heredoc pipeline text'
check_pipeline 'value=${fallback:-$(produce | head -n 1)}' 1 'pipeline in parameter-expansion substitution'
check_pipeline "value=\${fallback:-'\$(produce | head -n 1)'}" 0 'single-quoted parameter fallback is literal'
check_pipeline 'value=$(( $(produce | head -n 1) + 1 ))' 1 'pipeline in arithmetic substitution'
check_pipeline 'cat < <(produce | head -n 1)' 1 'pipeline in process substitution'
check_pipeline $'cat <<EOF\n$(produce | head -n 1)\nEOF' 1 'pipeline substitution in unquoted heredoc'
check_pipeline $'cat <<EOF\n\'$(produce | head -n 1)\'\nEOF' 1 'single quotes do not suppress expansion in an unquoted heredoc'
check_pipeline $'cat <<\'EOF\'\n$(produce | head -n 1)\nEOF' 0 'pipeline substitution in quoted heredoc'
check_pipeline $'cat <<\'EOF\'\n\'$(produce | head -n 1)\'\nEOF' 0 'quoted heredoc keeps single-quoted substitution text inert'
check_pipeline "printf '%s' '|' head -n 1" 0 'quoted pipe token remains data'
check_pipeline 'produce | grep -ehello' 0 'attached grep pattern is not an option cluster'

# Each owning repository mode must return nonzero and name the exact file and
# reason. These mutations use a real Git repository because discovery itself is
# part of the gate.
mutation="$work/mutation"
mkdir "$mutation"
git -C "$mutation" init -q
run_repository_mutation() {
    local mode=$1 source=$2 reason=$3 expected_line=${4:-3} output
    output="$work/$mode.err"
    printf '#!/usr/bin/env bash\nset -euo pipefail\n%s\n' "$source" >"$mutation/check.sh"
    git -C "$mutation" add check.sh
    if python3 "$scanner" "$mode" "$mutation" >"$work/$mode.out" 2>"$output"; then
        fail "$mode repository mode accepted its independent defect"
    fi
    grep -Fq "check.sh:$expected_line:" "$output" \
        || fail "$mode repository refusal did not name check.sh line $expected_line: $(cat "$output")"
    grep -Fq "$reason" "$output" \
        || fail "$mode repository refusal did not name its reason: $(cat "$output")"
    named_shapes=$((named_shapes + 1))
}
run_repository_mutation negative '! false' 'negated command has no explicit failure assertion'
run_repository_mutation negative '!(false)' 'negated command has no explicit failure assertion'
run_repository_mutation negative 'true && ! false' 'negated command has no explicit failure assertion'
run_repository_mutation negative 'false || ! false' 'negated command has no explicit failure assertion'
run_repository_mutation negative 'check() { ! false; echo reached; }; check' 'negated command has no explicit failure assertion'
run_repository_mutation builder "$debug_executable --version" 'target/debug use precedes a recognized builder invocation'
run_repository_mutation builder "run --binary '$debug_executable'" 'target/debug use precedes a recognized builder invocation'
run_repository_mutation builder "builder=scripts/require-built-commands.sh; $debug_executable --version" 'target/debug use precedes a recognized builder invocation'
run_repository_mutation builder "$(printf 'cat <<EOF\nscripts/require-built-commands.sh\nEOF\n%s --version' "$debug_executable")" 'target/debug use precedes a recognized builder invocation' 6
run_repository_mutation builder "build() { scripts/require-built-commands.sh; }; $debug_executable --version" 'target/debug use precedes a recognized builder invocation'
run_repository_mutation builder "build; build() { scripts/require-built-commands.sh; }; $debug_executable --version" 'target/debug use precedes a recognized builder invocation'
run_repository_mutation builder "builder=scripts/require-built-commands.sh; builder=other; \$builder; $debug_executable --version" 'target/debug use precedes a recognized builder invocation'
run_repository_mutation builder "builder=scripts/require-built-commands.sh; unset builder; \$builder; $debug_executable --version" 'target/debug use precedes a recognized builder invocation'
run_repository_mutation builder "builder=scripts/require-built-commands.sh; read builder; \$builder; $debug_executable --version" 'target/debug use precedes a recognized builder invocation'
run_repository_mutation builder "builder=scripts/require-built-commands.sh \"\$builder\"; $debug_executable --version" 'target/debug use precedes a recognized builder invocation'
run_repository_mutation builder "$debug_executable --version; scripts/require-built-commands.sh" 'target/debug use precedes a recognized builder invocation'
run_repository_mutation builder "builder=scripts/require-built-commands.sh; '\$builder'; $debug_executable --version" 'target/debug use precedes a recognized builder invocation'
run_repository_mutation builder "builder=scripts/require-built-commands.sh; change() { builder=other; }; change; \"\$builder\"; $debug_executable --version" 'target/debug use precedes a recognized builder invocation'
run_repository_mutation builder "outer() { inner() { scripts/require-built-commands.sh; }; }; outer; $debug_executable --version" 'target/debug use precedes a recognized builder invocation'
run_repository_mutation builder "if false; then scripts/require-built-commands.sh; fi; $debug_executable --version" 'unsupported builder flow'
run_repository_mutation builder "if false; then $debug_executable --version; fi" 'unsupported builder flow'
run_repository_mutation builder "build() { scripts/require-built-commands.sh; }; if false; then build; fi; $debug_executable --version" 'unsupported builder flow'
run_repository_mutation builder "if false; then builder=scripts/require-built-commands.sh; fi; \$builder; $debug_executable --version" 'unsupported builder flow'
run_repository_mutation builder "builder=scripts/require-built-commands.sh; if false; then unset builder; fi; \$builder; $debug_executable --version" 'unsupported builder flow'
run_repository_mutation builder "if false; then executable=$debug_executable; fi; \"\$executable\" --version" 'unsupported builder flow'
run_repository_mutation pipelines 'produce | head -n 1' 'uses head'
run_repository_mutation pipelines "produce | sed '1q'" 'uses sed q'
run_repository_mutation pipelines "produce | awk '/yes/ { exit }'" 'uses exit-on-match awk'
run_repository_mutation pipelines 'produce | grep -m 1 yes' 'uses early-closing grep'
run_repository_mutation pipelines 'produce | grep -l yes' 'uses early-closing grep'
run_repository_mutation pipelines 'produce | read' 'uses bare read'
run_repository_mutation pipelines 'produce | (head -n 1)' 'uses head'
run_repository_mutation pipelines 'produce | { cat >/dev/null; head -n 1; }' 'uses head'
run_repository_mutation pipelines 'produce | grep -e yes -q' 'uses quiet grep'
run_repository_mutation pipelines 'produce | sed -n -e1q' 'uses sed q'
run_repository_mutation pipelines 'value=${fallback:-$(produce | head -n 1)}' 'uses head'
run_repository_mutation pipelines 'value=$(( $(produce | head -n 1) + 1 ))' 'uses head'
run_repository_mutation pipelines 'cat < <(produce | head -n 1)' 'uses head'
run_repository_mutation pipelines $'cat <<EOF\n$(produce | head -n 1)\nEOF' 'uses head' 4
run_repository_mutation negative $'cat <<EOF\nunterminated' 'unterminated heredoc'
run_repository_mutation builder $'cat <<EOF\nunterminated' 'unterminated heredoc'
run_repository_mutation pipelines $'cat <<EOF\nunterminated' 'unterminated heredoc'

# This is the exact repair used by the repository's first-match helpers: awk
# remembers the first row and prints from END, after the producer has finished.
# The defective head mutation closes the same producer early. Twenty thousand
# rows exceed a pipe buffer, so pipefail observes that discarded producer.
large="$work/large"
for ((i = 0; i < 20000; i++)); do printf 'line-%05d\n' "$i"; done >"$large"
first=$(sed -n '/^line-/p' "$large" | awk 'NR == 1 { first = $0 } END { if (NR) print first }')
[[ "$first" == line-00000 ]] || fail "large fixture returned $first instead of its first line"
defective="$work/defective.sh"
printf '%s\n' '#!/usr/bin/env bash' 'set -euo pipefail' "sed -n '/^line-/p' \"\$1\" | head -n 1 >/dev/null" >"$defective"
if bash "$defective" "$large"; then
    fail 'the 20,000-row defective mutation hid the producer status it discarded'
fi

broken="$work/broken.sh"
printf '#!/usr/bin/env bash\ncat <<EOF\nunterminated\n' >"$broken"
if python3 "$scanner" file "$broken" >"$work/broken.out" 2>"$work/broken.err"; then
    fail 'the scanner accepted an unterminated heredoc'
fi
grep -Fq 'unterminated heredoc' "$work/broken.err" \
    || fail "unsupported syntax did not produce a named diagnostic: $(cat "$work/broken.err")"

deep=:
for ((i = 0; i < 65; i++)); do deep="printf x \$($deep)"; done
printf '#!/usr/bin/env bash\n%s\n' "$deep" >"$broken"
bash -n "$broken" || fail 'the substitution-depth fixture is not valid Bash'
if python3 "$scanner" file "$broken" >"$work/broken.out" 2>"$work/broken.err"; then
    fail 'the scanner accepted executable substitution nesting past its documented bound'
fi
grep -Fq 'command substitution nesting exceeds 64' "$work/broken.err" \
    || fail "substitution nesting did not produce a named diagnostic: $(cat "$work/broken.err")"

deep=x
for ((i = 0; i < 66; i++)); do deep="\${x:-$deep}"; done
printf '#!/usr/bin/env bash\nvalue=%s\n' "$deep" >"$broken"
bash -n "$broken" || fail 'the parameter-depth fixture is not valid Bash'
if python3 "$scanner" file "$broken" >"$work/broken.out" 2>"$work/broken.err"; then
    fail 'the scanner accepted parameter expansion nesting past its documented bound'
fi
grep -Fq 'parameter expansion nesting exceeds 64' "$work/broken.err" \
    || fail "parameter nesting did not produce a named diagnostic: $(cat "$work/broken.err")"

fanout='leaf() { :; };'
prior=leaf
for ((i = 0; i < 20; i++)); do
    fanout+=" f$i() { $prior; $prior; };"
    prior=f$i
done
check_builder "$fanout $prior" True 'function fanout stops at its named expansion bound' 1

chain='f0() { scripts/require-built-commands.sh; };'
prior=f0
for ((i = 1; i < 1050; i++)); do
    chain+=" f$i() { $prior; };"
    prior=f$i
done
check_builder "$chain $prior; $debug_executable --version" False \
    'direct 1050-function chain stops at its named depth bound' 1 \
    'function call depth exceeds 128 calls'
check_builder "$chain if false; then $prior; fi; $debug_executable --version" False \
    'conditional 1050-function chain stops at its named summary-depth bound' 1 \
    'function summary depth exceeds 128 calls'

large_body='large_body() {'
large_calls=''
for ((i = 0; i < 300; i++)); do
    large_body+=' :;'
    large_calls+=' large_body;'
done
large_body+=' };'
check_builder "$large_body $large_calls" True \
    'repeated large function bodies stop at the processed-command bound' 1 \
    'processed command count exceeds'

printf 'shell scanner coverage: exact tracked discovery; %s named shape and repository-mode checks passed\n' "$named_shapes"
