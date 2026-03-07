#!/usr/bin/env bash

# --- Test Suite for setup.sh ---
# This script runs setup.sh in a isolated environment to verify its behavior.

# 1. SETUP: Create a temporary mock home directory
TEST_DIR=$(mktemp -d)
MOCK_HOME="$TEST_DIR/home"
MOCK_DOTS="$TEST_DIR/dots_repo"
PROJECT_ROOT=$(pwd)/..
mkdir -p "$MOCK_HOME"
mkdir -p "$MOCK_DOTS"

# Copy the current project to the mock dots repo location
cp -r "$PROJECT_ROOT/." "$MOCK_DOTS"

echo "🧪 Starting tests in isolated environment: $TEST_DIR"

# 2. HELPER: Assertion function
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

# 3. EXECUTION: Run the setup script with mocked environment
# We'll use 'HOME' to redirect where it tries to install things.
# We'll also mock 'brew' and 'timeout' to avoid actually installing stuff.

export HOME="$MOCK_HOME"
cd "$MOCK_DOTS"

echo "🏃 Running setup.sh (mocked)..."

# Mocking git, brew, and gum to speed up tests and avoid side effects
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

# Run the script! 
(
    export HOME="$MOCK_HOME"
    export PATH="$mock_bin:$PATH"
    export CI=1
    cd "$MOCK_DOTS"
    # Run with output to a file so we can debug if it fails
    bash ./setup.sh > "$TEST_DIR/setup_output.log" 2>&1
)

# 4. VERIFICATION: Check the results
if [ ! -d "$HOME/.config" ]; then
    echo "❌ FAIL: $HOME/.config is missing"
    echo "--- Setup Output ---"
    cat "$TEST_DIR/setup_output.log"
    exit 1
fi

# Check if .dots directory was "created" (or exists)
# Note: In our test, we're not actually cloning, but checking if the logic handles it.
assert_exists "$HOME/.dots"

# Check symlinks (based on setup.sh L220+)
assert_link "$HOME/.config/bat"
assert_link "$HOME/.config/tmux"
assert_link "$HOME/.zshrc"

echo ""
echo "🎉 ALL TESTS PASSED!"

# 5. CLEANUP
rm -rf "$TEST_DIR"
