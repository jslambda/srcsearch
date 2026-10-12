#!/usr/bin/env bash
set -euo pipefail

workspace=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
scratch=$(mktemp -d)

fixture() {
    local dir
    dir=$(mktemp -d "$scratch/case.XXXXXX")
    mkdir -p "$dir/scripts"
    cp "$workspace/scripts/setup-ripgrep-benchmark" "$dir/scripts/"
    printf '%s\n' "$dir"
}

git() {
    printf 'git %s\n' "$*" >> "$command_log"
    if [[ "$1" == clone ]]; then
        mkdir -p "$3"
    fi
}

rg() {
    printf 'ripgrep 15.2.0\n'
}

cargo() {
    printf 'cargo %s\n' "$*" >> "$command_log"
    mkdir -p "$test_workspace/target/debug"
    printf '#!/bin/sh\nprintf "srcsearch 0.2.0\\n"\n' > "$test_workspace/target/debug/srcsearch"
    chmod +x "$test_workspace/target/debug/srcsearch"
}

export -f git rg cargo

for project_state in absent empty; do
    test_workspace=$(fixture)
    command_log="$test_workspace/commands"
    export test_workspace command_log
    if [[ "$project_state" == empty ]]; then
        mkdir -p "$test_workspace/.project"
    fi
    output=$("$test_workspace/scripts/setup-ripgrep-benchmark")
    [[ -d "$test_workspace/.project/ripgrep" ]]
    [[ "$output" == *'ripgrep 15.2.0'* ]]
    [[ "$output" == *'srcsearch 0.2.0'* ]]
    command grep -q '^git clone https://github.com/BurntSushi/ripgrep.git ' "$command_log"
    command grep -q '^git -C .* checkout --detach 3fce3b5bb0236da2df6d99672afb8a719642eca7$' "$command_log"
    command grep -q '^cargo build --manifest-path ' "$command_log"
done

for entry in visible .hidden; do
    test_workspace=$(fixture)
    command_log="$test_workspace/commands"
    export test_workspace command_log
    mkdir -p "$test_workspace/.project"
    touch "$test_workspace/.project/$entry"
    if "$test_workspace/scripts/setup-ripgrep-benchmark" > "$test_workspace/output" 2>&1; then
        printf 'Expected nonempty .project to fail.\n' >&2
        exit 1
    fi
    [[ ! -e "$command_log" ]]
    command grep -q 'is not empty' "$test_workspace/output"
done

test_workspace=$(fixture)
command_log="$test_workspace/commands"
export test_workspace command_log
ln -s "$test_workspace/elsewhere" "$test_workspace/.project"
if "$test_workspace/scripts/setup-ripgrep-benchmark" > "$test_workspace/output" 2>&1; then
    printf 'Expected symlinked .project to fail.\n' >&2
    exit 1
fi
[[ ! -e "$command_log" ]]
command grep -q 'not a file or symlink' "$test_workspace/output"

printf 'setup-ripgrep-benchmark tests passed\n'
