#!/usr/bin/env bash

# --- CharVim:test suite ---
# this script test setup.sh in a isolated environment to verify its behavior.
# all hail microslop!

# --- CharVim:setup ---
# create a temporary mock home directory
TEST_DIR=$(mktemp -d)
MOCK_HOME="$TEST_DIR/home"
MOCK_DOTS="$TEST_DIR/dots_repo"
PROJECT_ROOT=$(pwd)/..
mkdir -p "$MOCK_HOME"
mkdir -p "$MOCK_DOTS"

# copy the current project to the mock dots repo location
cp -r "$PROJECT_ROOT/." "$MOCK_DOTS"

echo "🧪 Starting tests in isolated environment: $TEST_DIR"

# --- CharVim:helper ---
# assertion function
assert_exists() {
    if [ -e "$1" ]; then
        echo "✅ PASS: $1 exists"
    else
        echo "❌ FAIL: $1 is missing"
        exit 1
    fi
}

assert_link() {
    if [ -L "$1" ]; then
        echo "✅ PASS: $1 is a symlink"
    else
        echo "❌ FAIL: $1 is NOT a symlink"
        exit 1
    fi
}

# --- CharVim:execution ---
# run the setup script with mocked environment
# use 'HOME' to redirect where it tries to install things.
# also mock 'brew' and 'timeout' to avoid actually installing stuff.

export HOME="$MOCK_HOME"
cd "$MOCK_DOTS" || exit 1

echo "🏃 Running setup.sh (mocked)..."

# mocking git, brew, and gum to speed up tests and avoid side effects
mock_bin="$TEST_DIR/bin"
mkdir -p "$mock_bin"

cat > "$mock_bin/git" <<EOF
#!/bin/bash
if [[ "\$*" == *"clone"* ]]; then
    # Create the directory that the clone would have created
    mkdir -p "\${!#}" 
    echo "Mock git clone to \${!#}"
fi
exit 0
EOF

cat > "$mock_bin/timeout" <<EOF
#!/bin/bash
# Just run the command without actually timing out in the test
shift 1
"\$@"
EOF

cat > "$mock_bin/curl" <<EOF
#!/bin/bash
echo "Mock curl called with: \$*"
exit 0
EOF

cat > "$mock_bin/brew" <<EOF
#!/bin/bash
# Pretend brew is always installed to skip the install step
echo "Mock brew called with: \$*"
exit 0
EOF
cat > "$mock_bin/gum" <<EOF
#!/bin/bash
# If 'spin' is used, execute the command after '--'
if [[ "\$1" == "spin" ]]; then
    shift
    while [[ "\$1" != "--" && "\$#" -gt 0 ]]; do shift; done
    shift
    exec "\$@"
else
    # For other gum commands like 'style', just exit success
    exit 0
fi
EOF
chmod +x "$mock_bin/git" "$mock_bin/timeout" "$mock_bin/brew" "$mock_bin/gum" "$mock_bin/curl"
export PATH="$mock_bin:$PATH"

# run the script
export CI=1
# run with output to a file to debug if fails
# we are already in MOCK_DOTS
bash ./setup.sh > "$TEST_DIR/setup_output.log" 2>&1

# --- CharVim:verification ---
echo "🧐 Verifying results..."

if [ ! -d "$MOCK_HOME/.config" ]; then
    echo "❌ FAIL: $MOCK_HOME/.config is missing"
    echo "--- Setup Output ---"
    cat "$TEST_DIR/setup_output.log"
    exit 1
fi

# check if .dots directory was "created" (or exists)
# note: in thw test, its not actually cloning, but checking if the logic handles it
assert_exists "$MOCK_HOME/.dots"

# check symlinks (based on setup.sh L220+)
assert_link "$MOCK_HOME/.config/bat"
assert_link "$MOCK_HOME/.config/tmux"
assert_link "$MOCK_HOME/.zshrc"

echo ""
echo "🎉 ALL TESTS PASSED!"

# --- CharVim:cleanup ---
rm -rf "$TEST_DIR"
