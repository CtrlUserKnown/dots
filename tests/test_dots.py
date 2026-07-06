#!/usr/bin/env python3
"""
CI test suite for dots.py, ssm.py, and shared.py.
Run from repo root: python3 tests/test_dots.py
"""

import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

REPO_ROOT     = Path(__file__).resolve().parents[1]
ZSHZSH        = REPO_ROOT / "src/zsh/zsh"
SETTINGS_FILE = REPO_ROOT / ".settings"
sys.path.insert(0, str(ZSHZSH))

PASS_COUNT = 0
FAIL_COUNT = 0


def ok(label: str) -> None:
    global PASS_COUNT
    PASS_COUNT += 1
    print(f"  ✅ {label}")


def fail(label: str, detail: str = "") -> None:
    global FAIL_COUNT
    FAIL_COUNT += 1
    msg = f"  ❌ {label}"
    if detail:
        msg += f"\n     {detail}"
    print(msg)


def section(title: str) -> None:
    print(f"\n── {title} {'─' * (50 - len(title))}")


# ── helpers ────────────────────────────────────────────────────────────────────

def run_dots_set(arg: str) -> subprocess.CompletedProcess:
    return subprocess.run(
        [sys.executable, str(ZSHZSH / "dots.py"), "--set", arg],
        capture_output=True, text=True,
    )


def make_behind_repo() -> tuple[Path, Path]:
    """Return (local_path, tmpdir) where local is 1 commit behind a tagged origin."""
    tmp = Path(tempfile.mkdtemp())
    bare  = tmp / "origin.git"
    local = tmp / "local"
    ahead = tmp / "ahead"

    subprocess.run(["git", "init", "--bare", str(bare)], capture_output=True)
    subprocess.run(["git", "clone", str(bare), str(local)], capture_output=True)

    for repo in (local,):
        subprocess.run(["git", "-C", str(repo), "config", "user.email", "ci@test.com"], capture_output=True)
        subprocess.run(["git", "-C", str(repo), "config", "user.name",  "CI"],          capture_output=True)

    (local / "a.txt").write_text("a")
    subprocess.run(["git", "-C", str(local), "add", "."], capture_output=True)
    subprocess.run(["git", "-C", str(local), "commit", "-m", "init"], capture_output=True)
    subprocess.run(["git", "-C", str(local), "push", "origin", "main"], capture_output=True)

    # advance origin via a second clone
    subprocess.run(["git", "clone", str(bare), str(ahead)], capture_output=True)
    subprocess.run(["git", "-C", str(ahead), "config", "user.email", "ci@test.com"], capture_output=True)
    subprocess.run(["git", "-C", str(ahead), "config", "user.name",  "CI"],          capture_output=True)
    (ahead / "b.txt").write_text("b")
    subprocess.run(["git", "-C", str(ahead), "add", "."], capture_output=True)
    subprocess.run(["git", "-C", str(ahead), "commit", "-m", "next"], capture_output=True)
    subprocess.run(["git", "-C", str(ahead), "tag", "v9.9.9"], capture_output=True)
    subprocess.run(["git", "-C", str(ahead), "push", "origin", "main", "--tags"], capture_output=True)

    subprocess.run(
        ["git", "-C", str(local), "remote", "set-head", "origin", "main"],
        capture_output=True,
    )
    return local, tmp


# ── dots.py --set validation ───────────────────────────────────────────────────

section("dots.py --set: argument validation")

settings_backup = SETTINGS_FILE.read_text() if SETTINGS_FILE.exists() else None
try:
    r = run_dots_set("greeting")
    if r.returncode == 1 and "key=value" in r.stderr:
        ok("missing '=' exits 1 with clear error")
    else:
        fail("missing '=' should exit 1", f"rc={r.returncode} stderr={r.stderr!r}")

    r = run_dots_set("bogus_key=42")
    if r.returncode == 1 and "unknown setting" in r.stderr and "bogus_key" in r.stderr:
        ok("unknown key exits 1, names the bad key")
    else:
        fail("unknown key should exit 1", f"rc={r.returncode} stderr={r.stderr!r}")

    r = run_dots_set("greeting=false")
    settings = json.loads(SETTINGS_FILE.read_text())
    if r.returncode == 0 and settings.get("greeting") is False:
        ok("greeting=false -> JSON false (bool coercion)")
    else:
        fail("greeting=false should write JSON false", f"rc={r.returncode} settings={settings}")

    r = run_dots_set("update_frequency=720")
    settings = json.loads(SETTINGS_FILE.read_text())
    if r.returncode == 0 and settings.get("update_frequency") == 720:
        ok("update_frequency=720 -> integer 720")
    else:
        fail("update_frequency=720 should write int", f"settings={settings}")

    r = run_dots_set("update_check=TRUE")
    settings = json.loads(SETTINGS_FILE.read_text())
    if r.returncode == 0 and settings.get("update_check") is True:
        ok("update_check=TRUE (uppercase) -> JSON true")
    else:
        fail("case-insensitive coercion failed", f"settings={settings}")

    r = run_dots_set("update_check=0")
    settings = json.loads(SETTINGS_FILE.read_text())
    if r.returncode == 0 and settings.get("update_check") is False:
        ok("update_check=0 -> JSON false")
    else:
        fail("numeric 0 coercion failed", f"settings={settings}")

finally:
    if settings_backup is not None:
        SETTINGS_FILE.write_text(settings_backup)
    else:
        SETTINGS_FILE.unlink(missing_ok=True)


# ── check_upstream: mode branching ────────────────────────────────────────────

section("shared.py check_upstream: dev vs normal mode")

from shared import check_upstream

local, tmpdir = make_behind_repo()
try:
    behind, label = check_upstream(local, dev_mode=False)
    if behind == 1 and label == "9.9.9":
        ok("normal mode: behind=1, label is semver tag")
    else:
        fail("normal mode label wrong", f"behind={behind}, label={label!r}")

    behind, label = check_upstream(local, dev_mode=True)
    if behind == 1 and len(label) >= 7 and label.isalnum():
        ok(f"dev mode: behind=1, label is short SHA ({label!r})")
    else:
        fail("dev mode label should be short SHA", f"behind={behind}, label={label!r}")
finally:
    shutil.rmtree(str(tmpdir), ignore_errors=True)

# up-to-date repo returns (0, '') in both modes
behind_n, label_n = check_upstream(REPO_ROOT, dev_mode=False)
behind_d, label_d = check_upstream(REPO_ROOT, dev_mode=True)
if behind_n == 0 and label_n == "" and behind_d == 0 and label_d == "":
    ok("up-to-date repo: both modes return (0, '')")
else:
    fail("up-to-date repo check failed",
         f"normal=({behind_n},{label_n!r}) dev=({behind_d},{label_d!r})")


# ── ssm.py: password storage ──────────────────────────────────────────────────

section("ssm.py: password storage")

import ssm

try:
    import keyring as _kr
    _KEYRING_AVAIL = True
except Exception:
    _KEYRING_AVAIL = False

real_sf     = Path.home() / ".config/ssm/sessions.json"
sf_backup   = real_sf.read_text() if real_sf.exists() else None

try:
    real_sf.parent.mkdir(parents=True, exist_ok=True)

    ssm.save_sessions([{"name": "ci-test", "host": "10.0.0.1",
                        "user": "admin", "password": "s3cr3t", "port": 22}])
    raw = json.loads(real_sf.read_text())
    if "password" not in raw[0]:
        ok("save_sessions strips password from JSON")
    else:
        fail("save_sessions left password in JSON")

    loaded = ssm.load_sessions()
    if loaded[0].get("password") == "s3cr3t":
        ok("load_sessions restores password in memory")
    elif not _KEYRING_AVAIL:
        ok("load_sessions (no keyring backend): graceful fallback")
    else:
        fail("load_sessions password mismatch", f"got {loaded[0].get('password')!r}")

    # migration: plaintext password in JSON -> stripped + stored in keyring
    real_sf.write_text(json.dumps([{"name": "ci-migrate", "host": "10.0.0.2",
                                    "user": "u", "password": "migrated", "port": 22}]))
    ssm.load_sessions()
    raw2 = json.loads(real_sf.read_text())
    if "password" not in raw2[0]:
        ok("load_sessions migrates plaintext -> strips from JSON")
    else:
        fail("load_sessions did not strip migrated password")

finally:
    if _KEYRING_AVAIL:
        for name in ("ci-test", "ci-migrate"):
            try:
                _kr.delete_password("dots-ssm", name)
            except Exception:
                pass
    if sf_backup is not None:
        real_sf.write_text(sf_backup)
    else:
        real_sf.unlink(missing_ok=True)


# ── ssm.py: SSH hardening ─────────────────────────────────────────────────────

section("ssm.py: SSH hardening options")

import inspect
src = inspect.getsource(ssm)

if "StrictHostKeyChecking=accept-new" in src:
    ok("do_connect uses StrictHostKeyChecking=accept-new")
else:
    fail("StrictHostKeyChecking=no found -- host key bypass present")

if "UserKnownHostsFile=/dev/null" not in src:
    ok("do_connect does not discard known_hosts (/dev/null absent)")
else:
    fail("UserKnownHostsFile=/dev/null still present")

if "SSM_KNOWN_HOSTS" in src:
    ok("do_connect routes known_hosts to SSM_KNOWN_HOSTS path")
else:
    fail("SSM_KNOWN_HOSTS not referenced")


# ── update check: dev vs normal mode wiring ───────────────────────────────────

section("dots.py / ssm.py: update check dev_mode wiring")

import ast

for module_name, filepath in [("dots", ZSHZSH / "dots.py"), ("ssm", ZSHZSH / "ssm.py")]:
    src_text = filepath.read_text()
    if "dev_mode=dev" in src_text or "dev_mode=" in src_text:
        ok(f"{module_name}.py passes dev_mode to check_upstream")
    else:
        fail(f"{module_name}.py does not pass dev_mode")

# confirm shared.py has both fetch variants
shared_src = (ZSHZSH / "shared.py").read_text()
if "dev_mode" in shared_src and "--tags" in shared_src and "rev-parse" in shared_src:
    ok("shared.py check_upstream has both fetch paths (tags / SHA)")
else:
    fail("shared.py missing dev or normal mode fetch path")


# ── summary ───────────────────────────────────────────────────────────────────

print(f"\n{'─' * 55}")
total = PASS_COUNT + FAIL_COUNT
print(f"Results: {PASS_COUNT}/{total} passed")

if FAIL_COUNT:
    sys.exit(1)
