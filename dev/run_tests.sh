#!/usr/bin/env bash
# run_tests.sh — Validation tests for cmdlog (Rust binary + shell hooks)
# Usage: bash /path/to/cmdlog/dev/run_tests.sh

set -uo pipefail

PASS=0
FAIL=0
TOTAL=0

# Derive all paths from this script's location (always dev/)
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CMDLOG_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
CMDLOG="$CMDLOG_DIR/cmdlog"
CARGO="${CARGO:-$(command -v cargo 2>/dev/null || echo cargo)}"

# Use temp HOME to isolate from user's real config and from the user's
# real ~/.cmdlog.tsv (the binary defaults log path to $HOME/.cmdlog.tsv).
ORIG_HOME="$HOME"
TEST_HOME=$(mktemp -d)
export HOME="$TEST_HOME"
# Create empty config to prevent init_config from copying default.conf
touch "$HOME/.cmdlog.conf"

pass() { ((PASS++)); ((TOTAL++)); printf "  \033[32mPASS\033[0m  %s\n" "$1"; }
fail() { ((FAIL++)); ((TOTAL++)); printf "  \033[31mFAIL\033[0m  %s\n" "$1"; }
section() { printf "\n\033[1m=== %s ===\033[0m\n" "$1"; }
# grep -c that returns 0 instead of failing on no match
gcount() { grep -c "$@" 2>/dev/null || true; }

# --------------------------------------------------------------------------
section "Build (from dev/)"
# --------------------------------------------------------------------------

# Always build from SCRIPT_DIR (dev/) to avoid stray target/ at project root
(cd "$SCRIPT_DIR" && "$CARGO" build --release 2>&1) && pass "cargo build --release" || { fail "cargo build --release"; exit 1; }
cp "$SCRIPT_DIR/target/release/cmdlog" "$CMDLOG"

# Stage the canonical install layout inside the test HOME so the hardcoded
# paths (binary at $HOME/.local/bin/cmdlog, default.conf at
# $HOME/.local/share/cmdlog/default.conf) resolve to the just-built artifacts.
mkdir -p "$HOME/.local/bin" "$HOME/.local/share/cmdlog"
ln -sf "$CMDLOG" "$HOME/.local/bin/cmdlog"
cp "$CMDLOG_DIR/default.conf" "$HOME/.local/share/cmdlog/default.conf"

# --------------------------------------------------------------------------
section "Cargo unit/integration tests (from dev/)"
# --------------------------------------------------------------------------

(cd "$SCRIPT_DIR" && "$CARGO" test -- --test-threads=1 2>&1)
rc=$?
[[ $rc -eq 0 ]] && pass "cargo test" || fail "cargo test (exit $rc)"

# --------------------------------------------------------------------------
section "Binary exists and runs"
# --------------------------------------------------------------------------

[[ -x "$CMDLOG" ]] && pass "cmdlog binary is executable" || fail "cmdlog binary not found"
$CMDLOG help >/dev/null 2>&1 && pass "cmdlog help exits 0" || fail "cmdlog help failed"

# --------------------------------------------------------------------------
section "Hook syntax validation"
# --------------------------------------------------------------------------

bash --norc --noprofile -c "source $CMDLOG_DIR/hook/cmdlog.bash 2>/dev/null" && pass "hook.bash sources without error" || fail "hook.bash source error"
zsh --no-rcs -c "source $CMDLOG_DIR/hook/cmdlog.zsh 2>/dev/null" && pass "hook.zsh sources without error" || fail "hook.zsh source error"
tcsh -f -c "source $CMDLOG_DIR/hook/cmdlog.tcsh" 2>/dev/null && pass "hook.tcsh sources without error" || fail "hook.tcsh source error"

# --------------------------------------------------------------------------
section "Re-source safety"
# --------------------------------------------------------------------------

# bash: re-sourcing should not duplicate __cmdlog_record in PROMPT_COMMAND
n=$(bash --norc --noprofile -i -c "
    source $CMDLOG_DIR/hook/cmdlog.bash 2>/dev/null
    source $CMDLOG_DIR/hook/cmdlog.bash 2>/dev/null
    source $CMDLOG_DIR/hook/cmdlog.bash 2>/dev/null
    echo \"\$PROMPT_COMMAND\" | grep -o '__cmdlog_record' | wc -l
" 2>/dev/null)
n=$(echo "$n" | tr -d ' ')
[[ "$n" -eq 1 ]] && pass "bash: re-source does not duplicate PROMPT_COMMAND" || fail "bash: re-source duplicated PROMPT_COMMAND ($n occurrences)"

# zsh: re-sourcing should not duplicate __cmdlog_record in precmd_functions
n=$(zsh -i --no-rcs -c "
    source $CMDLOG_DIR/hook/cmdlog.zsh 2>/dev/null
    source $CMDLOG_DIR/hook/cmdlog.zsh 2>/dev/null
    source $CMDLOG_DIR/hook/cmdlog.zsh 2>/dev/null
    echo \${#\${(M)precmd_functions:#__cmdlog_record}}
" 2>/dev/null)
n=$(echo "$n" | tr -d ' ')
[[ "$n" -eq 1 ]] && pass "zsh: re-source does not duplicate precmd_functions" || fail "zsh: re-source duplicated precmd_functions ($n occurrences)"

# tcsh: re-sourcing should not produce errors
tcsh -f -c "source $CMDLOG_DIR/hook/cmdlog.tcsh; source $CMDLOG_DIR/hook/cmdlog.tcsh; source $CMDLOG_DIR/hook/cmdlog.tcsh" 2>/dev/null && pass "tcsh: re-source produces no errors" || fail "tcsh: re-source produced errors"

# --------------------------------------------------------------------------
section "Interactive-only guard"
# --------------------------------------------------------------------------

out=$(bash --norc --noprofile -c "source $CMDLOG_DIR/hook/cmdlog.bash 2>/dev/null; declare -F __cmdlog_record || true" 2>&1)
[[ -z "$out" ]] && pass "bash: hook inactive in non-interactive shell" || fail "bash: hook active in non-interactive shell"

out=$(zsh --no-rcs -c "source $CMDLOG_DIR/hook/cmdlog.zsh 2>/dev/null; whence __cmdlog_record || true" 2>&1)
[[ -z "$out" ]] && pass "zsh: hook inactive in non-interactive shell" || fail "zsh: hook active in non-interactive shell"

# --------------------------------------------------------------------------
section "Record: builtin detection"
# --------------------------------------------------------------------------

rm -f $HOME/.cmdlog.tsv
for cmd in "cd /tmp" "echo hello" "pwd" "export FOO=bar" "set +x" "local x=1" \
           "alias ll=ls" "source ~/.bashrc" "return 0" "typeset -A arr"; do
    $CMDLOG record bash /test 0 "$cmd"
done
n=$(wc -l < $HOME/.cmdlog.tsv 2>/dev/null || echo 0)
[[ "$n" -eq 0 ]] && pass "builtins: all 10 builtin commands skipped" || fail "builtins: $n commands logged (expected 0)"

# --------------------------------------------------------------------------
section "Record: formerly waived commands now recorded"
# --------------------------------------------------------------------------

rm -f $HOME/.cmdlog.tsv
for cmd in "ls -la" "grep -rn TODO ." "cat file.txt" "sort data.csv" \
           "man bash" "less README.md" "find . -name '*.py'" "head -20 log"; do
    $CMDLOG record bash /test 0 "$cmd"
done
n=$(wc -l < $HOME/.cmdlog.tsv 2>/dev/null || echo 0)
[[ "$n" -eq 8 ]] && pass "record: all 8 formerly-waived commands now recorded" || fail "record: $n commands logged (expected 8)"

# --------------------------------------------------------------------------
section "Record: external commands logged"
# --------------------------------------------------------------------------

rm -f $HOME/.cmdlog.tsv
for cmd in "git status" "python3 script.py" "make -j8" "ssh server uname" "cmake -B build"; do
    $CMDLOG record bash /test 0 "$cmd"
done
n=$(wc -l < $HOME/.cmdlog.tsv 2>/dev/null || echo 0)
[[ "$n" -eq 5 ]] && pass "external: all 5 commands logged" || fail "external: $n commands logged (expected 5)"

# --------------------------------------------------------------------------
section "Record: pipe override"
# --------------------------------------------------------------------------

rm -f $HOME/.cmdlog.tsv
$CMDLOG record bash /test 0 "echo hello | tr a b"         # builtin + pipe → LOG
$CMDLOG record bash /test 0 "cat f | grep x | wc -l"       # waived + pipe → LOG
$CMDLOG record bash /test 0 "ls -la | head"                 # waived + pipe → LOG
n=$(wc -l < $HOME/.cmdlog.tsv 2>/dev/null || echo 0)
[[ "$n" -eq 3 ]] && pass "pipe: all 3 pipe commands logged (override)" || fail "pipe: $n commands logged (expected 3)"

# --------------------------------------------------------------------------
section "Record: deduplication"
# --------------------------------------------------------------------------

rm -f $HOME/.cmdlog.tsv
$CMDLOG record bash /test 0 "git status"
$CMDLOG record bash /test 0 "git status"
$CMDLOG record bash /test 0 "git status"
$CMDLOG record bash /test 0 "make -j8"
$CMDLOG record bash /test 0 "make -j8"
# Record writes all 5 entries (no record-time dedup)
n=$(wc -l < $HOME/.cmdlog.tsv 2>/dev/null || echo 0)
[[ "$n" -eq 5 ]] && pass "dedup: all 5 entries written (no record-time dedup)" || fail "dedup: $n entries (expected 5)"

# Load-time dedup collapses consecutive duplicates
n=$($CMDLOG list --no-color -a | wc -l)
[[ "$n" -eq 2 ]] && pass "dedup: list collapses consecutive duplicates (5→2)" || fail "dedup: list shows $n entries (expected 2)"

# Non-consecutive same command preserved
$CMDLOG record bash /test 0 "git status"
n=$($CMDLOG list --no-color -a | wc -l)
[[ "$n" -eq 3 ]] && pass "dedup: non-consecutive same command shown" || fail "dedup: list shows $n entries (expected 3)"

# --------------------------------------------------------------------------
section "Record: shell type in log"
# --------------------------------------------------------------------------

rm -f $HOME/.cmdlog.tsv
$CMDLOG record bash /test 0 "git status"
$CMDLOG record zsh /test 0 "python3 foo.py"
$CMDLOG record tcsh /test 0 "ssh server"
n_bash=$(gcount 'bash' $HOME/.cmdlog.tsv)
n_zsh=$(gcount 'zsh' $HOME/.cmdlog.tsv)
n_tcsh=$(gcount 'tcsh' $HOME/.cmdlog.tsv)
[[ "$n_bash" -eq 1 && "$n_zsh" -eq 1 && "$n_tcsh" -eq 1 ]] && pass "shell type: bash/zsh/tcsh all recorded" || fail "shell type mismatch: bash=$n_bash zsh=$n_zsh tcsh=$n_tcsh"

# --------------------------------------------------------------------------
section "Record: TSV format"
# --------------------------------------------------------------------------

rm -f $HOME/.cmdlog.tsv
$CMDLOG record bash /home/edwardc 0 "git push origin main"
line=$(cat $HOME/.cmdlog.tsv)
bad=$(echo "$line" | grep -cvP '^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\tbash\t/home/edwardc\t0\tgit push origin main$' || true)
[[ "$bad" -eq 0 ]] && pass "format: correct TSV (DATE\\tSHELL\\tPWD\\tEXIT_CODE\\tCMD)" || fail "format: bad TSV: $line"

# --------------------------------------------------------------------------
section "List: query filters"
# --------------------------------------------------------------------------

cat > $HOME/.cmdlog.tsv << 'DATA'
2026-04-05T09:15:00	bash	/home/edwardc/project-a	0	git status
2026-04-05T09:16:30	bash	/home/edwardc/project-a	0	make -j8 all
2026-04-05T10:00:00	zsh	/home/edwardc/project-b	0	python3 train.py --epochs 10
2026-04-05T14:30:00	tcsh	/home/edwardc/project-a	0	grep -rn TODO src/
2026-04-06T08:00:00	bash	/home/edwardc/project-a	0	git pull origin main
2026-04-06T08:05:00	bash	/home/edwardc/project-a	0	echo $PATH | tr : '\n'
2026-04-06T09:00:00	zsh	/home/edwardc/project-b	0	docker compose up -d
2026-04-06T09:30:00	tcsh	/tmp	0	nvcc --version
2026-04-06T10:00:00	bash	/home/edwardc/project-a	0	pytest tests/ -v
2026-04-06T10:15:00	zsh	/home/edwardc/project-b	0	cmake -B build
DATA

# Default
n=$($CMDLOG list --no-color | wc -l)
[[ "$n" -eq 10 ]] && pass "list: default shows all 10 (< 20)" || fail "list: default shows $n"

# -n limit
n=$($CMDLOG list --no-color -n 3 | wc -l)
[[ "$n" -eq 3 ]] && pass "list: -n 3 shows 3" || fail "list: -n 3 shows $n"

# -a
n=$($CMDLOG list --no-color -a | wc -l)
[[ "$n" -eq 10 ]] && pass "list: -a shows all 10" || fail "list: -a shows $n"

# -t shell
n=$($CMDLOG list --no-color -a -t bash | wc -l)
[[ "$n" -eq 5 ]] && pass "list: -t bash returns 5" || fail "list: -t bash returns $n"

n=$($CMDLOG list --no-color -a -t zsh | wc -l)
[[ "$n" -eq 3 ]] && pass "list: -t zsh returns 3" || fail "list: -t zsh returns $n"

n=$($CMDLOG list --no-color -a -t tcsh | wc -l)
[[ "$n" -eq 2 ]] && pass "list: -t tcsh returns 2" || fail "list: -t tcsh returns $n"

# -d date
n=$($CMDLOG list --no-color -a -d 2026-04-05 | wc -l)
[[ "$n" -eq 4 ]] && pass "list: -d 2026-04-05 returns 4" || fail "list: -d 2026-04-05 returns $n"

n=$($CMDLOG list --no-color -a -d 2026-04-06 | wc -l)
[[ "$n" -eq 6 ]] && pass "list: -d 2026-04-06 returns 6" || fail "list: -d 2026-04-06 returns $n"

n=$($CMDLOG list --no-color -a -d 2026-04 | wc -l)
[[ "$n" -eq 10 ]] && pass "list: -d 2026-04 returns all 10" || fail "list: -d 2026-04 returns $n"

# -s search
n=$($CMDLOG list --no-color -a -s git | wc -l)
[[ "$n" -eq 2 ]] && pass "list: -s git returns 2" || fail "list: -s git returns $n"

# -p path
n=$($CMDLOG list --no-color -a -p /home/edwardc/project-a | wc -l)
[[ "$n" -eq 6 ]] && pass "list: -p project-a returns 6" || fail "list: -p project-a returns $n"

n=$($CMDLOG list --no-color -a -p /home/edwardc/project-b | wc -l)
[[ "$n" -eq 3 ]] && pass "list: -p project-b returns 3" || fail "list: -p project-b returns $n"

n=$($CMDLOG list --no-color -a -p /tmp | wc -l)
[[ "$n" -eq 1 ]] && pass "list: -p /tmp returns 1" || fail "list: -p /tmp returns $n"

# Combined
n=$($CMDLOG list --no-color -a -d 2026-04-06 -t bash | wc -l)
[[ "$n" -eq 3 ]] && pass "list: date+shell returns 3" || fail "list: date+shell returns $n"

n=$($CMDLOG list --no-color -a -d 2026-04-06 -t bash -s pytest | wc -l)
[[ "$n" -eq 1 ]] && pass "list: date+shell+search returns 1" || fail "list: date+shell+search returns $n"

n=$($CMDLOG list --no-color -a -t zsh -p /home/edwardc/project-b | wc -l)
[[ "$n" -eq 3 ]] && pass "list: shell+path returns 3" || fail "list: shell+path returns $n"

# No match
out=$($CMDLOG list --no-color -s zzzznonexistent 2>&1)
[[ "$out" == "No matching entries." ]] && pass "list: no match message" || fail "list: no match output: '$out'"

# --no-color
out=$($CMDLOG list --no-color -n 1)
if echo "$out" | grep -qP '\033\['; then
    fail "list: --no-color has ANSI codes"
else
    pass "list: --no-color clean output"
fi

# --help
$CMDLOG list --help >/dev/null 2>&1 && pass "list: --help exits 0" || fail "list: --help failed"

# Missing log
mv $HOME/.cmdlog.tsv $HOME/.cmdlog.tsv.bak
out=$($CMDLOG list 2>&1)
rc=$?
mv $HOME/.cmdlog.tsv.bak $HOME/.cmdlog.tsv
[[ "$out" == "No matching entries." ]] && pass "list: missing log shows 'No matching entries.'" || fail "list: missing log output: '$out'"

# Empty log
> $HOME/.cmdlog.tsv
out=$($CMDLOG list 2>&1)
[[ "$out" == "No matching entries." ]] && pass "list: empty log message" || fail "list: empty log output: '$out'"

# --------------------------------------------------------------------------
section "E2E: bash hook + binary"
# --------------------------------------------------------------------------

E2E_LOG=$(mktemp)
bash --norc --noprofile -i << BSESSION 2>/dev/null
source $CMDLOG_DIR/hook/cmdlog.bash
git --version
cd /tmp
ls -la
echo hello
cat /dev/null | head
pwd
make --version
BSESSION

n=$(gcount 'git --version' $HOME/.cmdlog.tsv)
[[ "$n" -ge 1 ]] && pass "e2e bash: 'git --version' recorded" || fail "e2e bash: 'git --version' not found"

n=$(gcount 'make --version' $HOME/.cmdlog.tsv)
[[ "$n" -ge 1 ]] && pass "e2e bash: 'make --version' recorded" || fail "e2e bash: 'make --version' not found"

n=$(gcount 'cat /dev/null | head' $HOME/.cmdlog.tsv)
[[ "$n" -ge 1 ]] && pass "e2e bash: pipe command recorded" || fail "e2e bash: pipe command not found"

n=$(gcount 'cd /tmp' $HOME/.cmdlog.tsv)
[[ "$n" -eq 0 ]] && pass "e2e bash: 'cd /tmp' skipped (builtin)" || fail "e2e bash: 'cd /tmp' recorded"

n=$(gcount '	ls -la' $HOME/.cmdlog.tsv)
[[ "$n" -ge 1 ]] && pass "e2e bash: 'ls -la' recorded (no record-time waive)" || fail "e2e bash: 'ls -la' not found"

rm -f "$E2E_LOG"

# --------------------------------------------------------------------------
section "E2E: zsh hook + binary"
# --------------------------------------------------------------------------

rm -f $HOME/.cmdlog.tsv
zsh --no-rcs -i << ZSESSION 2>/dev/null
HISTSIZE=1000
SAVEHIST=0
source $CMDLOG_DIR/hook/cmdlog.zsh
git --version
cd /tmp
ls -la
echo hello
cat /dev/null | head
pwd
make --version
exit
ZSESSION

n=$(gcount 'git --version' $HOME/.cmdlog.tsv)
[[ "$n" -ge 1 ]] && pass "e2e zsh: 'git --version' recorded" || fail "e2e zsh: 'git --version' not found"

n=$(gcount 'make --version' $HOME/.cmdlog.tsv)
[[ "$n" -ge 1 ]] && pass "e2e zsh: 'make --version' recorded" || fail "e2e zsh: 'make --version' not found"

n=$(gcount 'cat /dev/null | head' $HOME/.cmdlog.tsv)
[[ "$n" -ge 1 ]] && pass "e2e zsh: pipe command recorded" || fail "e2e zsh: pipe command not found"

n=$(gcount 'cd /tmp' $HOME/.cmdlog.tsv)
[[ "$n" -eq 0 ]] && pass "e2e zsh: 'cd /tmp' skipped (builtin)" || fail "e2e zsh: 'cd /tmp' recorded"

n=$(gcount '	ls -la' $HOME/.cmdlog.tsv)
[[ "$n" -ge 1 ]] && pass "e2e zsh: 'ls -la' recorded (no record-time waive)" || fail "e2e zsh: 'ls -la' not found"

# --------------------------------------------------------------------------
section "E2E: tcsh hook logic + binary"
# --------------------------------------------------------------------------

# tcsh precmd only fires with a real tty. Simulate the precmd logic.
rm -f $HOME/.cmdlog.tsv
for cmd in "git --version" "cd /tmp" "ls -la" "echo hello" \
           "cat /dev/null | head" "pwd" "make --version"; do
    $CMDLOG record tcsh /test 0 "$cmd"
done

n=$(gcount 'git --version' $HOME/.cmdlog.tsv)
[[ "$n" -ge 1 ]] && pass "e2e tcsh: 'git --version' recorded" || fail "e2e tcsh: 'git --version' not found"

n=$(gcount 'make --version' $HOME/.cmdlog.tsv)
[[ "$n" -ge 1 ]] && pass "e2e tcsh: 'make --version' recorded" || fail "e2e tcsh: 'make --version' not found"

n=$(gcount 'cat /dev/null | head' $HOME/.cmdlog.tsv)
[[ "$n" -ge 1 ]] && pass "e2e tcsh: pipe command recorded" || fail "e2e tcsh: pipe command not found"

n=$(gcount 'cd /tmp' $HOME/.cmdlog.tsv)
[[ "$n" -eq 0 ]] && pass "e2e tcsh: 'cd /tmp' skipped (builtin)" || fail "e2e tcsh: 'cd /tmp' recorded"

n=$(gcount '	ls -la' $HOME/.cmdlog.tsv)
[[ "$n" -ge 1 ]] && pass "e2e tcsh: 'ls -la' recorded (no record-time waive)" || fail "e2e tcsh: 'ls -la' not found"

# --------------------------------------------------------------------------
section "List: waive filtering at display time"
# --------------------------------------------------------------------------

rm -f $HOME/.cmdlog.tsv
# Write config with waive list to test HOME
cat > "$HOME/.cmdlog.conf" << 'CONF'
[waive]
commands = ["ls", "cat"]
CONF

$CMDLOG record bash /test 0 "git status"
$CMDLOG record bash /test 0 "ls -la"
$CMDLOG record bash /test 0 "cat file.txt"
$CMDLOG record bash /test 0 "make -j8"
$CMDLOG record bash /test 0 "cat f | grep x"

n=$($CMDLOG list --no-color -a | wc -l)
[[ "$n" -eq 3 ]] && pass "waive display: shows 3 (git, make, piped cat)" || fail "waive display: shows $n (expected 3)"

# Restore empty config for remaining tests
echo "" > "$HOME/.cmdlog.conf"

# --------------------------------------------------------------------------
section "Hook: cmdlog wrapper function defined"
# --------------------------------------------------------------------------

# bash wrapper
out=$(bash -i -c "source $CMDLOG_DIR/hook/cmdlog.bash 2>/dev/null; type cmdlog" 2>&1)
echo "$out" | grep -q 'function' && pass "bash: cmdlog wrapper function defined" || fail "bash: cmdlog wrapper function missing"

# zsh wrapper
out=$(zsh -i -c "source $CMDLOG_DIR/hook/cmdlog.zsh 2>/dev/null; whence -w cmdlog" 2>&1)
echo "$out" | grep -q 'function' && pass "zsh: cmdlog wrapper function defined" || fail "zsh: cmdlog wrapper function missing"

# tcsh wrapper
out=$(tcsh -c "set prompt=x; source $CMDLOG_DIR/hook/cmdlog.tcsh; alias cmdlog" 2>&1)
[[ -n "$out" ]] && pass "tcsh: cmdlog wrapper alias defined" || fail "tcsh: cmdlog wrapper alias missing"

# --------------------------------------------------------------------------
section "TUI: non-interactive fallback"
# --------------------------------------------------------------------------

cat > $HOME/.cmdlog.tsv << 'DATA'
2026-04-06T08:00:00	bash	/home/edwardc/project-a	0	git pull origin main
2026-04-06T09:00:00	zsh	/home/edwardc/project-b	0	docker compose up -d
2026-04-06T10:00:00	bash	/home/edwardc/project-a	0	pytest tests/ -v
DATA

# Piped output should use linear mode (no escape sequences from TUI)
n=$($CMDLOG list --no-color -a | wc -l)
[[ "$n" -eq 3 ]] && pass "tui: piped output uses linear fallback" || fail "tui: piped output shows $n lines (expected 3)"

# --no-tui flag
n=$($CMDLOG list --no-tui -a 2>/dev/null | wc -l)
[[ "$n" -eq 3 ]] && pass "tui: --no-tui forces linear output" || fail "tui: --no-tui shows $n lines (expected 3)"

# --no-color + --no-tui combined
out=$($CMDLOG list --no-color --no-tui -n 1)
if echo "$out" | grep -qP '\033\['; then
    fail "tui: --no-color --no-tui has ANSI codes"
else
    pass "tui: --no-color --no-tui clean output"
fi

# --help still works
$CMDLOG list --help 2>&1 | grep -q 'no-tui' && pass "tui: --help mentions --no-tui" || fail "tui: --help missing --no-tui"

# --------------------------------------------------------------------------
section "Interactive: tcsh alias no if? prompt (expect)"
# --------------------------------------------------------------------------

# Test that cmdlog subcommands don't produce "if?" in tcsh
for subcmd_label in "help:cmdlog help" "--badarg:cmdlog --badarg" "record:cmdlog record tcsh /tmp 0 test_expect"; do
    label="${subcmd_label%%:*}"
    cmd="${subcmd_label#*:}"
    result=$(expect -c "
        log_user 0
        set timeout 5
        spawn /usr/bin/tcsh -f -i
        expect -re {[>$%#]}
        send \"source $CMDLOG_DIR/hook/cmdlog.tcsh\r\"
        expect -re {[>$%#]}
        send \"$cmd\r\"
        expect {
            \"if?\"  { puts BROKEN }
            \"else?\" { puts BROKEN }
            -re {[>$%#]} { puts OK }
            timeout { puts TIMEOUT }
        }
        send \"exit\r\"
        expect eof
    " 2>/dev/null)
    [[ "$result" == "OK" ]] && pass "tcsh interactive: '$label' no if? prompt" || fail "tcsh interactive: '$label' got $result"
done

# --------------------------------------------------------------------------
section "Interactive: tcsh prompt appears after hook (expect)"
# --------------------------------------------------------------------------

result=$(expect -c "
    log_user 0
    set timeout 5
    spawn /usr/bin/tcsh -f -i
    expect -re {[>$%#]}
    send \"source $CMDLOG_DIR/hook/cmdlog.tcsh\r\"
    expect {
        -re {[>$%#]} { puts OK }
        timeout { puts TIMEOUT }
    }
    send \"exit\r\"
    expect eof
" 2>/dev/null)
[[ "$result" == "OK" ]] && pass "tcsh interactive: prompt appears after hook" || fail "tcsh interactive: prompt missing ($result)"

# --------------------------------------------------------------------------
section "Interactive: tcsh precmd records commands (expect)"
# --------------------------------------------------------------------------

rm -f "$HOME/.cmdlog.tsv"
expect -c "
    log_user 0
    set timeout 5
    spawn /usr/bin/tcsh -f -i
    expect -re {[>$%#]}
    send \"source $CMDLOG_DIR/hook/cmdlog.tcsh\r\"
    expect -re {[>$%#]}
    send \"git --version\r\"
    expect -re {[>$%#]}
    send \"make --version\r\"
    expect -re {[>$%#]}
    send \"exit\r\"
    expect eof
" 2>/dev/null

n=$(gcount 'git --version' "$HOME/.cmdlog.tsv")
[[ "$n" -ge 1 ]] && pass "tcsh precmd: 'git --version' recorded via precmd" || fail "tcsh precmd: 'git --version' not found"

n=$(gcount 'make --version' "$HOME/.cmdlog.tsv")
[[ "$n" -ge 1 ]] && pass "tcsh precmd: 'make --version' recorded via precmd" || fail "tcsh precmd: 'make --version' not found"

# --------------------------------------------------------------------------
section "Interactive: bash wrapper passthrough (expect)"
# --------------------------------------------------------------------------

# Verify that the bash cmdlog wrapper passes through non-list subcommands
result=$(expect -c "
    log_user 0
    set timeout 5
    spawn bash --norc --noprofile -i
    expect -re {[$#]}
    send \"source $CMDLOG_DIR/hook/cmdlog.bash\r\"
    expect -re {[$#]}
    send \"cmdlog help\r\"
    expect {
        \"Cross-shell\" { puts OK }
        timeout { puts TIMEOUT }
    }
    send \"exit\r\"
    expect eof
" 2>/dev/null)
[[ "$result" == "OK" ]] && pass "bash interactive: cmdlog help via wrapper" || fail "bash interactive: cmdlog help ($result)"

# --------------------------------------------------------------------------
section "Inject: config defaults (no [inject] section)"
# --------------------------------------------------------------------------

# With empty config, defaults should apply (bash/zsh only — tcsh default "tiocsti"
# triggers a kernel probe that fails outside a PTY, covered by Rust unit tests)
[[ "$($CMDLOG config inject.bash)" == "readline" ]] && pass "config inject.bash default = readline" || fail "config inject.bash default"
[[ "$($CMDLOG config inject.zsh)" == "print-z" ]] && pass "config inject.zsh default = print-z" || fail "config inject.zsh default"

# --------------------------------------------------------------------------
section "Inject: config overrides"
# --------------------------------------------------------------------------

# Use non-TIOCSTI values to avoid kernel probe in test runner
cat > "$HOME/.cmdlog.conf" <<'TOML'
[inject]
bash = "history"
zsh = "history"
tcsh = "history"
TOML

[[ "$($CMDLOG config inject.bash)" == "history" ]] && pass "config inject.bash override = history" || fail "config inject.bash override"
[[ "$($CMDLOG config inject.zsh)" == "history" ]] && pass "config inject.zsh override = history" || fail "config inject.zsh override"
[[ "$($CMDLOG config inject.tcsh)" == "history" ]] && pass "config inject.tcsh override = history" || fail "config inject.tcsh override"

# Restore empty config for remaining tests
> "$HOME/.cmdlog.conf"

# --------------------------------------------------------------------------
section "Doctor: config check and repair"
# --------------------------------------------------------------------------

# Invalid inject for bash — doctor should report (no auto-fix; user must edit)
cat > "$HOME/.cmdlog.conf" <<'TOML'
[inject]
bash = "foobar"
TOML
$CMDLOG doctor bash 2>/dev/null
rc=$?
[[ $rc -eq 1 ]] && pass "doctor bash: invalid inject exits 1" || fail "doctor bash: exits $rc"

# Invalid inject for zsh — doctor should report (no auto-fix)
cat > "$HOME/.cmdlog.conf" <<'TOML'
[inject]
zsh = "bad_method"
TOML
$CMDLOG doctor zsh 2>/dev/null
rc=$?
[[ $rc -eq 1 ]] && pass "doctor zsh: invalid inject exits 1" || fail "doctor zsh: exits $rc"

# Invalid inject for tcsh — doctor should report (no auto-fix)
cat > "$HOME/.cmdlog.conf" <<'TOML'
[inject]
tcsh = "nonexistent"
TOML
$CMDLOG doctor tcsh 2>/dev/null
rc=$?
[[ $rc -eq 1 ]] && pass "doctor tcsh: invalid inject exits 1" || fail "doctor tcsh: exits $rc"

# Invalid show.time — doctor should auto-fix to "age"
cat > "$HOME/.cmdlog.conf" <<'TOML'
[show]
time = "ago"
TOML
$CMDLOG doctor bash 2>/dev/null
rc=$?
[[ $rc -eq 1 ]] && pass "doctor: invalid show.time exits 1" || fail "doctor: show.time exits $rc"
grep -q 'time = "age"' "$HOME/.cmdlog.conf" && pass "doctor: auto-fixed show.time to age" || fail "doctor: show.time not fixed"

# Invalid order.recency — doctor should auto-fix to "asc"
cat > "$HOME/.cmdlog.conf" <<'TOML'
[order]
recency = "sideways"
TOML
$CMDLOG doctor bash 2>/dev/null
rc=$?
[[ $rc -eq 1 ]] && pass "doctor: invalid order.recency exits 1" || fail "doctor: order.recency exits $rc"
grep -q 'recency = "asc"' "$HOME/.cmdlog.conf" && pass "doctor: auto-fixed order.recency to asc" || fail "doctor: order.recency not fixed"

# Unknown key in [filter] — doctor should catch typo
cat > "$HOME/.cmdlog.conf" <<'TOML'
[filter]
this_shel = true
TOML
$CMDLOG doctor bash 2>/dev/null
rc=$?
[[ $rc -eq 1 ]] && pass "doctor: unknown filter key exits 1" || fail "doctor: unknown filter key exits $rc"

# Unknown key in [show] — doctor should catch typo
cat > "$HOME/.cmdlog.conf" <<'TOML'
[show]
tme = "age"
TOML
$CMDLOG doctor bash 2>/dev/null
rc=$?
[[ $rc -eq 1 ]] && pass "doctor: unknown show key exits 1" || fail "doctor: unknown show key exits $rc"

# Unknown top-level section
cat > "$HOME/.cmdlog.conf" <<'TOML'
[foobar]
x = 1
TOML
$CMDLOG doctor bash 2>/dev/null
rc=$?
[[ $rc -eq 1 ]] && pass "doctor: unknown section exits 1" || fail "doctor: unknown section exits $rc"

# Valid config — doctor should exit 0
cat > "$HOME/.cmdlog.conf" <<'TOML'
[inject]
bash = "readline"
[show]
time = "age"
TOML
$CMDLOG doctor bash 2>/dev/null
rc=$?
[[ $rc -eq 0 ]] && pass "doctor: valid config exits 0" || fail "doctor: valid config exits $rc"

# Missing config — doctor should create from defaults and exit 0
rm -f "$HOME/.cmdlog.conf"
$CMDLOG doctor bash 2>/dev/null
rc=$?
[[ $rc -eq 0 ]] && pass "doctor: missing config exits 0" || fail "doctor: missing config exits $rc"
[[ -f "$HOME/.cmdlog.conf" ]] && pass "doctor: config file created" || fail "doctor: config file not created"

# Unparseable config — doctor should back up and regenerate
echo "this is [[[ not valid toml" > "$HOME/.cmdlog.conf"
$CMDLOG doctor bash 2>/dev/null
rc=$?
[[ $rc -eq 1 ]] && pass "doctor: unparseable config exits 1" || fail "doctor: unparseable exits $rc"
[[ -f "$HOME/.cmdlog.conf.bak" ]] && pass "doctor: backup created" || fail "doctor: no backup"
rm -f "$HOME/.cmdlog.conf.bak"

# Wrong type — doctor should fix
cat > "$HOME/.cmdlog.conf" <<'TOML'
[show]
time = 123
TOML
$CMDLOG doctor bash 2>/dev/null
rc=$?
[[ $rc -eq 1 ]] && pass "doctor: wrong type exits 1" || fail "doctor: wrong type exits $rc"
grep -q 'time = "age"' "$HOME/.cmdlog.conf" && pass "doctor: fixed wrong type to default" || fail "doctor: type not fixed"

# Missing section — doctor should fill (soft, exit 0)
cat > "$HOME/.cmdlog.conf" <<'TOML'
[show]
time = "age"
shell = false
dir = "relpath"
repo = true
count = true
exit_code = false
order = ["repo", "path", "count", "exit", "time", "shell"]
[filter]
this_shell = false
this_dir = false
this_repo = false
today = false
operator = "off"
exit_code = "off"
TOML
$CMDLOG doctor bash 2>/dev/null
rc=$?
[[ $rc -eq 0 ]] && pass "doctor: missing section fill exits 0" || fail "doctor: missing section exits $rc"
grep -q '\[inject\]' "$HOME/.cmdlog.conf" && pass "doctor: inject section filled" || fail "doctor: inject not filled"

# Restore empty config for remaining tests
> "$HOME/.cmdlog.conf"

# --------------------------------------------------------------------------
section "Inject: TIOCSTI e2e (expect)"
# --------------------------------------------------------------------------

# Test cmdlog inject pushes text into tcsh prompt via TIOCSTI
result=$(expect -c "
    log_user 0
    set timeout 5
    spawn /usr/bin/tcsh -f -i
    expect -re {[>$%#]}
    send \"$CMDLOG inject tcsh 'echo hello_tiocsti'\r\"
    expect {
        \"echo hello_tiocsti\" { puts OK }
        -re {[>$%#]} { puts NO_INJECT }
        timeout { puts TIMEOUT }
    }
    send \"exit\r\"
    expect eof
" 2>/dev/null)
[[ "$result" == "OK" ]] && pass "tcsh: TIOCSTI inject puts text in prompt" || fail "tcsh: TIOCSTI inject ($result)"

# Test cmdlog inject works in zsh
result=$(expect -c "
    log_user 0
    set timeout 5
    spawn zsh -f -i
    expect -re {[>$%#]}
    send \"$CMDLOG inject zsh 'echo hello_zsh'\r\"
    expect {
        \"echo hello_zsh\" { puts OK }
        -re {[>$%#]} { puts NO_INJECT }
        timeout { puts TIMEOUT }
    }
    send \"exit\r\"
    expect eof
" 2>/dev/null)
[[ "$result" == "OK" ]] && pass "zsh: TIOCSTI inject puts text in prompt" || fail "zsh: TIOCSTI inject ($result)"

# --------------------------------------------------------------------------
section "Inject: history mode e2e (expect)"
# --------------------------------------------------------------------------

# Configure tcsh to use history mode
cat > "$HOME/.cmdlog.conf" <<'TOML'
[inject]
tcsh = "history"
TOML

# cmdlog inject should exit 1 in history mode (no TIOCSTI attempted)
$CMDLOG inject tcsh "echo should_not_inject" 2>/dev/null
rc=$?
[[ $rc -eq 1 ]] && pass "tcsh history mode: inject exits 1" || fail "tcsh history mode: inject exits $rc (expected 1)"

# Verify bash config override works (no expect needed)
cat > "$HOME/.cmdlog.conf" <<'TOML'
[inject]
bash = "history"
TOML
[[ "$($CMDLOG config inject.bash)" == "history" ]] && pass "bash history mode: config returns history" || fail "bash history mode: config"

# Configure zsh to use history mode
cat > "$HOME/.cmdlog.conf" <<'TOML'
[inject]
zsh = "history"
TOML

result=$(expect -c "
    log_user 0
    set timeout 5
    spawn zsh -f -i
    expect -re {[>$%#]}
    send \"$CMDLOG config inject.zsh\r\"
    expect {
        \"history\" { puts OK }
        timeout { puts TIMEOUT }
    }
    send \"exit\r\"
    expect eof
" 2>/dev/null)
[[ "$result" == "OK" ]] && pass "zsh history mode: config returns history" || fail "zsh history mode: config ($result)"

# Restore empty config
> "$HOME/.cmdlog.conf"

# --------------------------------------------------------------------------
section "E2E: exit code recording (bash)"
# --------------------------------------------------------------------------

rm -f $HOME/.cmdlog.tsv
bash --norc --noprofile -i << BSESSION 2>/dev/null
source $CMDLOG_DIR/hook/cmdlog.bash
git --version
ls /nonexistent_cmdlog_test_path_12345
BSESSION

ec=$(grep 'git --version' $HOME/.cmdlog.tsv | awk -F'\t' '{print $4}')
[[ "$ec" == "0" ]] && pass "exit code: success recorded as 0" || fail "exit code: success recorded as $ec"

ec=$(grep 'ls /nonexistent' $HOME/.cmdlog.tsv | awk -F'\t' '{print $4}')
[[ "$ec" != "0" && -n "$ec" ]] && pass "exit code: failure recorded as non-zero ($ec)" || fail "exit code: failure recorded as $ec"

# --------------------------------------------------------------------------
section "E2E: exit code with PROMPT_COMMAND chaining (bash)"
# --------------------------------------------------------------------------

rm -f $HOME/.cmdlog.tsv
bash --norc --noprofile -i << BSESSION 2>/dev/null
# Simulate another tool's PROMPT_COMMAND
__other_tool_hook() { /bin/true; }
PROMPT_COMMAND=("__other_tool_hook")

# Source cmdlog — it prepends to the array
source $CMDLOG_DIR/hook/cmdlog.bash

# Run a failing command then a succeeding command
ls /nonexistent_cmdlog_chain_test_12345
git --version
BSESSION

ec=$(grep 'ls /nonexistent_cmdlog_chain' $HOME/.cmdlog.tsv | awk -F'\t' '{print $4}')
[[ "$ec" != "0" && -n "$ec" ]] && pass "bash chaining: failure exit code captured ($ec)" || fail "bash chaining: failure exit code was $ec"

ec=$(grep 'git --version' $HOME/.cmdlog.tsv | awk -F'\t' '{print $4}')
[[ "$ec" == "0" ]] && pass "bash chaining: success exit code captured" || fail "bash chaining: success exit code was $ec"

# --------------------------------------------------------------------------
section "E2E: exit code with precmd chaining (zsh)"
# --------------------------------------------------------------------------

rm -f $HOME/.cmdlog.tsv
# zsh's precmd doesn't reliably fire when stdout/stderr is a regular file or
# /dev/null — it skips prompt cycles. Use expect to provide a pty, which makes
# precmd fire as it would in a real interactive session.
expect -c "
    log_user 0
    set timeout 5
    spawn -noecho zsh --no-rcs -i
    expect -re {[>$%#]}
    send \"HISTSIZE=1000\r\"
    expect -re {[>$%#]}
    send \"SAVEHIST=0\r\"
    expect -re {[>$%#]}
    send \"__other_tool_precmd() { true; }\r\"
    expect -re {[>$%#]}
    send \"precmd_functions=(__other_tool_precmd)\r\"
    expect -re {[>$%#]}
    send \"source $CMDLOG_DIR/hook/cmdlog.zsh\r\"
    expect -re {[>$%#]}
    send \"ls /nonexistent_cmdlog_zsh_chain_12345\r\"
    expect -re {[>$%#]}
    send \"git --version\r\"
    expect -re {[>$%#]}
    send \"exit\r\"
    expect eof
" >/dev/null 2>&1

ec=$(grep 'ls /nonexistent_cmdlog_zsh' $HOME/.cmdlog.tsv | awk -F'\t' '{print $4}')
[[ "$ec" != "0" && -n "$ec" ]] && pass "zsh chaining: failure exit code captured ($ec)" || fail "zsh chaining: failure exit code was $ec"

ec=$(grep 'git --version' $HOME/.cmdlog.tsv | awk -F'\t' '{print $4}')
[[ "$ec" == "0" ]] && pass "zsh chaining: success exit code captured" || fail "zsh chaining: success exit code was $ec"

# Cleanup temp HOME
export HOME="$ORIG_HOME"
rm -rf "$TEST_HOME"

# --------------------------------------------------------------------------
# Summary
# --------------------------------------------------------------------------
echo ""
printf "\033[1m=== Results: %d passed, %d failed, %d total ===\033[0m\n" "$PASS" "$FAIL" "$TOTAL"
[[ "$FAIL" -eq 0 ]] && printf "\033[32mAll tests passed.\033[0m\n" || printf "\033[31mSome tests failed.\033[0m\n"
exit "$FAIL"
