#!/usr/bin/env python3
"""SSH Session Manager — curses TUI for storing and launching SSH connections."""

import curses
import json
import os
import shutil
import subprocess
import sys
from pathlib import Path

from shared import (
    DOTS_DIR, COLOR_HEADER, COLOR_SELECT, COLOR_ERROR, COLOR_DIM,
    init_colors, safe_addstr, draw_header as _draw_header, draw_footer, draw_desc,
    clamp, check_upstream,
)

SESSIONS_FILE = Path.home() / ".config" / "ssm" / "sessions.json"
CONFIG_FILE   = Path.home() / ".config" / "ssm" / "config.json"
VERSION       = os.environ.get("DOTS_VERSION", "")

SSM_KNOWN_HOSTS = Path.home() / ".config" / "ssm" / "known_hosts"


def draw_header(win, title: str) -> None:
    _draw_header(win, title, VERSION)


# ── keyring ───────────────────────────────────────────────────────────────────

try:
    import keyring as _keyring_mod
    _KEYRING = True
except ImportError:
    _KEYRING = False

_KR_SERVICE = "dots-ssm"


def _kr_store(name: str, password: str) -> bool:
    if not _KEYRING or not password:
        return False
    try:
        _keyring_mod.set_password(_KR_SERVICE, name, password)
        return True
    except Exception:
        return False


def _kr_load(name: str) -> str:
    if not _KEYRING:
        return ""
    try:
        return _keyring_mod.get_password(_KR_SERVICE, name) or ""
    except Exception:
        return ""


def _kr_delete(name: str) -> None:
    if not _KEYRING:
        return
    try:
        _keyring_mod.delete_password(_KR_SERVICE, name)
    except Exception:
        pass


# ── config ────────────────────────────────────────────────────────────────────

def load_config() -> dict:
    if not CONFIG_FILE.exists():
        return {"use_herdr": True}
    try:
        with open(CONFIG_FILE) as f:
            return json.load(f)
    except (json.JSONDecodeError, OSError):
        return {"use_herdr": True}


def save_config(cfg: dict) -> None:
    CONFIG_FILE.parent.mkdir(parents=True, exist_ok=True)
    with open(CONFIG_FILE, "w") as f:
        json.dump(cfg, f, indent=2)


def toggle_herdr(cfg: dict) -> dict:
    cfg["use_herdr"] = not cfg.get("use_herdr", True)
    save_config(cfg)
    return cfg


# ── storage ───────────────────────────────────────────────────────────────────

def _save_sessions_raw(sessions: list) -> None:
    """Write sessions to disk, stripping the password field."""
    SESSIONS_FILE.parent.mkdir(parents=True, exist_ok=True)
    clean = [{k: v for k, v in s.items() if k != "password"} for s in sessions]
    with open(SESSIONS_FILE, "w") as f:
        json.dump(clean, f, indent=2)


def load_sessions() -> list:
    if not SESSIONS_FILE.exists():
        return []
    try:
        with open(SESSIONS_FILE) as f:
            data = json.load(f)
    except (json.JSONDecodeError, OSError):
        return []

    migrated = False
    sessions = []
    for s in data:
        s = dict(s)
        plaintext_pw = s.pop("password", "")
        if plaintext_pw and _KEYRING:
            _kr_store(s["name"], plaintext_pw)
            migrated = True
        s["password"] = _kr_load(s["name"]) if _KEYRING else plaintext_pw
        sessions.append(s)

    if migrated:
        _save_sessions_raw(sessions)

    return sessions


def sessions_mtime() -> float:
    try:
        return SESSIONS_FILE.stat().st_mtime
    except OSError:
        return 0.0


def save_sessions(sessions: list) -> None:
    if _KEYRING:
        for s in sessions:
            pw = s.get("password", "")
            if pw:
                _kr_store(s["name"], pw)
        _save_sessions_raw(sessions)
    else:
        SESSIONS_FILE.parent.mkdir(parents=True, exist_ok=True)
        with open(SESSIONS_FILE, "w") as f:
            json.dump(sessions, f, indent=2)


# ── update check ──────────────────────────────────────────────────────────────

def do_pull() -> bool:
    """Fast-forward pull + symlink repair. Returns True on success."""
    try:
        r = subprocess.run(
            ["git", "-C", str(DOTS_DIR), "pull", "--ff-only"],
            capture_output=True, text=True, timeout=30,
        )
        if r.returncode == 0:
            subprocess.run(
                ["python3", str(DOTS_DIR / "src/zsh/zsh/dots.py"), "--repair-symlinks"],
                capture_output=True, timeout=15,
            )
            return True
        return False
    except Exception:
        return False


def run_update_view(stdscr) -> None:
    state      = "checking"
    behind     = 0
    up_ver     = ""

    while True:
        stdscr.erase()
        draw_header(stdscr, " update ")

        if state == "checking":
            safe_addstr(stdscr, 2, 2, "Fetching upstream…",
                        curses.color_pair(COLOR_DIM))
            draw_footer(stdscr, "")
            stdscr.refresh()
            dev = (DOTS_DIR / ".developer").exists()
            behind, up_ver = check_upstream(DOTS_DIR, dev_mode=dev)
            if behind == -1:
                state = "error"
            elif behind == 0:
                state = "uptodate"
            else:
                state = "available"

        elif state == "uptodate":
            safe_addstr(stdscr, 2, 2, "✓  Already up to date.",
                        curses.color_pair(COLOR_SELECT) | curses.A_BOLD)
            draw_desc(stdscr, "")
            draw_footer(stdscr, " q back ")
            stdscr.refresh()
            if stdscr.getch() != curses.KEY_RESIZE:
                return

        elif state == "available":
            if dev:
                lines = [f"  {behind} new commit(s) on origin (latest: {up_ver})"]
            else:
                lines = [f"  {behind} new commit(s) available."]
                if up_ver:
                    lines.append(f"  New version: v{up_ver}")
            lines += ["", "  Press p to pull, or q to skip."]
            for i, line in enumerate(lines):
                safe_addstr(stdscr, 2 + i, 0, line)
            draw_desc(stdscr, "")
            draw_footer(stdscr, " p pull  q back ")
            stdscr.refresh()
            key = stdscr.getch()
            if key in (ord("q"), ord("Q"), 27):
                return
            elif key == ord("p"):
                safe_addstr(stdscr, 2 + len(lines) + 1, 2, "Pulling…",
                            curses.color_pair(COLOR_DIM))
                stdscr.refresh()
                state = "done" if do_pull() else "pull_error"

        elif state == "error":
            safe_addstr(stdscr, 2, 2,
                        "✗  Could not reach upstream (offline or not a git repo).",
                        curses.color_pair(COLOR_ERROR) | curses.A_BOLD)
            draw_desc(stdscr, "")
            draw_footer(stdscr, " q back ")
            stdscr.refresh()
            if stdscr.getch() != curses.KEY_RESIZE:
                return

        elif state == "pull_error":
            safe_addstr(stdscr, 2, 2, "✗  Pull failed (check git output).",
                        curses.color_pair(COLOR_ERROR) | curses.A_BOLD)
            draw_desc(stdscr, "")
            draw_footer(stdscr, " q back ")
            stdscr.refresh()
            if stdscr.getch() != curses.KEY_RESIZE:
                return

        elif state == "done":
            safe_addstr(stdscr, 2, 2,
                        "✓  Dotfiles updated — restart your shell to apply.",
                        curses.color_pair(COLOR_SELECT) | curses.A_BOLD)
            draw_desc(stdscr, "")
            draw_footer(stdscr, " q back ")
            stdscr.refresh()
            if stdscr.getch() != curses.KEY_RESIZE:
                return


# ── form ──────────────────────────────────────────────────────────────────────

FIELD_DEFS = [
    ("Name",     False),
    ("Host/IP",  False),
    ("User",     False),
    ("Password", True),
    ("Port",     False),
]

FIELD_DEFAULTS = ["", "", "root", "", "22"]


def run_form(stdscr, existing: dict | None = None) -> dict | None:
    """Add/edit form. Returns new session dict or None if cancelled."""
    title  = " edit session " if existing else " add session "
    values = (
        [
            existing.get("name", ""),
            existing.get("host", ""),
            existing.get("user", "root"),
            existing.get("password", ""),
            str(existing.get("port", 22)),
        ]
        if existing
        else FIELD_DEFAULTS[:]
    )

    current = 0
    flash: tuple[str, int] | None = None

    curses.curs_set(1)

    while True:
        stdscr.erase()
        h, w = stdscr.getmaxyx()

        draw_header(stdscr, title)

        bw = min(62, w - 4)
        bx = max(0, (w - bw) // 2)
        by = 2

        for i, (fname, is_pass) in enumerate(FIELD_DEFS):
            fy      = by + i * 2
            val     = values[i]
            display = "•" * len(val) if is_pass else val
            is_sel  = i == current
            l_attr  = (curses.color_pair(COLOR_SELECT) | curses.A_BOLD) if is_sel \
                      else curses.color_pair(COLOR_DIM)
            safe_addstr(stdscr, fy, bx + 2, f"{fname:<10}", l_attr)
            field_w = bw - 15
            safe_addstr(stdscr, fy, bx + 13,
                        "[" + f"{display:<{field_w}}"[:field_w] + "]")

        draw_desc(stdscr, "", flash)
        draw_footer(stdscr, " tab/↓ next  ↑ prev  enter save  esc cancel")
        flash = None

        is_pass     = FIELD_DEFS[current][1]
        display_val = "•" * len(values[current]) if is_pass else values[current]
        cx = bx + 14 + len(display_val)
        cy = by + current * 2
        try:
            stdscr.move(cy, min(cx, w - 2))
        except curses.error:
            pass

        stdscr.refresh()
        key = stdscr.getch()

        if key == 27:
            curses.curs_set(0)
            return None

        elif key in (9, curses.KEY_DOWN):
            current = (current + 1) % len(FIELD_DEFS)

        elif key == curses.KEY_UP:
            current = (current - 1) % len(FIELD_DEFS)

        elif key in (curses.KEY_ENTER, 10, 13):
            if current < len(FIELD_DEFS) - 1:
                current += 1
            else:
                name = values[0].strip()
                host = values[1].strip()
                if not name:
                    flash   = ("  ✗ Name is required", COLOR_ERROR)
                    current = 0
                    continue
                if not host:
                    flash   = ("  ✗ Host/IP is required", COLOR_ERROR)
                    current = 1
                    continue
                try:
                    port = int(values[4].strip() or "22")
                except ValueError:
                    flash   = ("  ✗ Port must be a number", COLOR_ERROR)
                    current = 4
                    continue
                curses.curs_set(0)
                return {
                    "name":     name,
                    "host":     host,
                    "user":     values[2].strip() or "root",
                    "password": values[3],
                    "port":     port,
                }

        elif key in (curses.KEY_BACKSPACE, 127, 8):
            values[current] = values[current][:-1]

        elif 32 <= key <= 126:
            values[current] += chr(key)


# ── main TUI ──────────────────────────────────────────────────────────────────

def run_help_view(stdscr) -> None:
    """Full-screen keybinding reference."""
    lines = [
        ("navigation",   ""),
        ("j / k  ↑↓",   "move up / down"),
        ("gg",           "jump to top"),
        ("G",            "jump to bottom"),
        ("^d / ^u",      "half-page down / up"),
        ("^f / ^b",      "full-page down / up"),
        ("[n]j / [n]G",  "jump n rows down / to row n"),
        ("",             ""),
        ("actions",      ""),
        ("enter",        "connect to selected session"),
        ("a",            "add new session"),
        ("e",            "edit selected session"),
        ("D",            "duplicate selected session"),
        ("d",            "delete selected session"),
        ("y",            "copy connection string to clipboard"),
        ("",             ""),
        ("other",        ""),
        ("h",            "toggle herdr mode on / off"),
        ("u",            "check for dotfiles updates"),
        ("?",            "show this help screen"),
        ("q  esc",       "quit / go back"),
    ]

    while True:
        stdscr.erase()
        h, w = stdscr.getmaxyx()
        draw_header(stdscr, " help ")

        row = 2
        for key_str, desc in lines:
            if row >= h - 3:
                break
            if not key_str and not desc:
                row += 1
                continue
            if not desc:
                safe_addstr(stdscr, row, 2, key_str,
                            curses.color_pair(COLOR_DIM) | curses.A_BOLD)
            else:
                safe_addstr(stdscr, row, 4, f"{key_str:<14}",
                            curses.color_pair(COLOR_HEADER) | curses.A_BOLD)
                safe_addstr(stdscr, row, 18, desc)
            row += 1

        draw_footer(stdscr, " q back ")
        stdscr.refresh()
        if stdscr.getch() != curses.KEY_RESIZE:
            return


def run_search(stdscr, sessions: list) -> list:
    """Incremental search — returns filtered session list (preserves original indices)."""
    curses.curs_set(1)
    query = ""

    while True:
        matches = [s for s in sessions
                   if query.lower() in s.get("name", "").lower()
                   or query.lower() in s.get("host", "").lower()]

        stdscr.erase()
        h, w = stdscr.getmaxyx()
        draw_header(stdscr, " search ")

        prompt = f"  / {query}"
        safe_addstr(stdscr, 2, 0, prompt, curses.A_BOLD)

        list_top = 4
        list_h   = h - 7
        safe_addstr(stdscr, 3, 0,
                    f"  {'NAME':<20} {'HOST/IP':<24} {'USER':<12} PORT",
                    curses.color_pair(COLOR_DIM) | curses.A_BOLD)

        if not matches:
            safe_addstr(stdscr, list_top + 1, 4, "no matches",
                        curses.color_pair(COLOR_DIM))
        else:
            for i, sess in enumerate(matches[:list_h]):
                name = sess.get("name", "")[:19]
                host = sess.get("host", "")[:23]
                user = sess.get("user", "root")[:11]
                port = str(sess.get("port", 22))[:5]
                safe_addstr(stdscr, list_top + i, 0,
                            f"  {name:<20} {host:<24} {user:<12} {port}")

        count_str = f"{len(matches)} match{'es' if len(matches) != 1 else ''}"
        draw_desc(stdscr, count_str)
        draw_footer(stdscr, " type to filter  enter confirm  esc cancel ")
        try:
            stdscr.move(2, min(len(prompt), w - 1))
        except curses.error:
            pass
        stdscr.refresh()

        key = stdscr.getch()
        if key == 27:
            curses.curs_set(0)
            return []
        elif key in (curses.KEY_ENTER, 10, 13):
            curses.curs_set(0)
            return matches
        elif key in (curses.KEY_BACKSPACE, 127, 8):
            query = query[:-1]
        elif 32 <= key <= 126:
            query += chr(key)


def _yank(text: str) -> bool:
    """Copy text to system clipboard. Returns True on success."""
    try:
        subprocess.run(["pbcopy"], input=text.encode(), check=True, timeout=3)
        return True
    except Exception:
        return False


def run_tui(stdscr) -> tuple | None:
    """Main list view. Returns ('connect', session) or None."""
    init_colors()
    curses.curs_set(0)
    stdscr.keypad(True)

    sessions   = load_sessions()
    cfg        = load_config()
    last_mtime = sessions_mtime()
    idx        = 0
    flash: tuple[str, int] | None = None
    count_buf  = ""
    pending_g  = False
    filter_active = False  # True when showing search-filtered subset
    visible: list = []     # filtered view; empty means show all

    def active_sessions() -> list:
        return visible if filter_active else sessions

    while True:
        # autoreload when sessions.json is modified externally
        mt = sessions_mtime()
        if mt != last_mtime:
            sessions   = load_sessions()
            last_mtime = mt
            if filter_active:
                filter_active = False
                visible       = []
            idx   = clamp(idx, 0, max(0, len(active_sessions()) - 1))
            flash = ("  ↺ Sessions reloaded", COLOR_SELECT)

        cur = active_sessions()

        stdscr.erase()
        h, w = stdscr.getmaxyx()

        herdr_label = "[herdr ON]" if cfg.get("use_herdr", True) else "[herdr OFF]"
        herdr_attr  = curses.color_pair(COLOR_SELECT) if cfg.get("use_herdr", True) \
                      else curses.color_pair(COLOR_ERROR)
        title = " ssh sessions [filtered] " if filter_active else " ssh sessions "
        draw_header(stdscr, title)
        _, w = stdscr.getmaxyx()
        safe_addstr(stdscr, 0, w - len(herdr_label) - 2, herdr_label, herdr_attr | curses.A_BOLD)

        safe_addstr(stdscr, 2, 0,
                    f"  {'NAME':<20} {'HOST/IP':<24} {'USER':<12} PORT",
                    curses.color_pair(COLOR_DIM) | curses.A_BOLD)
        safe_addstr(stdscr, 3, 0, "  " + "─" * min(w - 3, 64),
                    curses.color_pair(COLOR_DIM))

        list_top = 4
        list_h   = h - 8

        if not cur:
            msg = "No sessions — press 'a' to add one" if not filter_active \
                  else "No matches — press esc or / to search again"
            safe_addstr(stdscr, list_top + 2, max(0, (w - len(msg)) // 2), msg,
                        curses.color_pair(COLOR_DIM))
        else:
            idx    = clamp(idx, 0, len(cur) - 1)
            scroll = max(0, idx - list_h + 1)
            for i, sess in enumerate(cur[scroll: scroll + list_h]):
                real_i = i + scroll
                y      = list_top + i
                name   = sess.get("name", "")[:19]
                host   = sess.get("host", "")[:23]
                user   = sess.get("user", "root")[:11]
                port   = str(sess.get("port", 22))[:5]
                row    = f"  {name:<20} {host:<24} {user:<12} {port}"
                if real_i == idx:
                    safe_addstr(stdscr, y, 0, "▶",
                                curses.color_pair(COLOR_SELECT) | curses.A_BOLD)
                    safe_addstr(stdscr, y, 1, row[1:], curses.A_BOLD)
                else:
                    safe_addstr(stdscr, y, 0, row)

        # desc / flash
        if flash:
            draw_desc(stdscr, "", flash)
        elif pending_g:
            draw_desc(stdscr, "g…")
        elif count_buf:
            draw_desc(stdscr, f"[{count_buf}]")
        elif cur:
            sel  = cur[idx]
            desc = f"{sel.get('user','root')}@{sel['host']}:{sel.get('port',22)}"
            draw_desc(stdscr, desc)
        else:
            draw_desc(stdscr, "")

        draw_footer(stdscr,
                    " j/k↑↓ nav  gg/G top/bot  ^d/^u/^f/^b scroll"
                    "  enter connect  a add  e edit  D dup  d del"
                    "  y yank  / search  h herdr  u update  ? help  q quit")
        stdscr.refresh()
        flash = None

        key = stdscr.getch()
        ch  = chr(key) if 32 <= key <= 126 else ""

        if ch.isdigit() and (ch != "0" or count_buf) and not pending_g:
            count_buf += ch
            continue

        count     = int(count_buf) if count_buf else 1
        had_count = bool(count_buf)
        count_buf = ""

        if key == ord("g"):
            if pending_g:
                idx       = 0
                pending_g = False
            else:
                pending_g = True
            continue

        pending_g = False

        if key in (ord("q"), ord("Q"), 27):
            if filter_active:
                filter_active = False
                visible       = []
                idx           = 0
            else:
                return None

        elif key == ord("/"):
            result = run_search(stdscr, sessions)
            if result:
                visible       = result
                filter_active = True
                idx           = 0
            elif filter_active:
                filter_active = False
                visible       = []

        elif key in (ord("j"), curses.KEY_DOWN):
            if cur:
                idx = clamp(idx + count, 0, len(cur) - 1)

        elif key in (ord("k"), curses.KEY_UP):
            if cur:
                idx = clamp(idx - count, 0, len(cur) - 1)

        elif key == ord("G"):
            if cur:
                idx = clamp(count - 1 if had_count else len(cur) - 1,
                            0, len(cur) - 1)

        elif key == 4:    # Ctrl+d — half page down
            if cur:
                idx = clamp(idx + max(1, list_h // 2), 0, len(cur) - 1)

        elif key == 21:   # Ctrl+u — half page up
            if cur:
                idx = clamp(idx - max(1, list_h // 2), 0, len(cur) - 1)

        elif key == 6:    # Ctrl+f — full page down
            if cur:
                idx = clamp(idx + list_h, 0, len(cur) - 1)

        elif key == 2:    # Ctrl+b — full page up
            if cur:
                idx = clamp(idx - list_h, 0, len(cur) - 1)

        elif key in (curses.KEY_ENTER, 10, 13):
            if cur:
                return ("connect", cur[idx], cfg)

        elif key == ord("a"):
            result = run_form(stdscr)
            if result:
                if any(s["name"] == result["name"] for s in sessions):
                    flash = (f"  ✗ Name '{result['name']}' already exists", COLOR_ERROR)
                else:
                    sessions.append(result)
                    save_sessions(sessions)
                    last_mtime = sessions_mtime()
                    idx        = len(sessions) - 1
                    filter_active = False
                    visible       = []
                    flash      = (f"  ✓ Added '{result['name']}'", COLOR_SELECT)

        elif key == ord("e"):
            if cur:
                old_name = cur[idx].get("name", "")
                real_idx = sessions.index(cur[idx]) if cur[idx] in sessions else -1
                result = run_form(stdscr, cur[idx])
                if result:
                    dup = any(
                        s["name"] == result["name"] and s is not cur[idx]
                        for s in sessions
                    )
                    if dup:
                        flash = (f"  ✗ Name '{result['name']}' already exists", COLOR_ERROR)
                    else:
                        if old_name and old_name != result["name"]:
                            _kr_delete(old_name)
                        if real_idx >= 0:
                            sessions[real_idx] = result
                        if filter_active:
                            visible[idx] = result
                        save_sessions(sessions)
                        last_mtime = sessions_mtime()
                        flash      = (f"  ✓ Updated '{result['name']}'", COLOR_SELECT)

        elif key == ord("D"):
            if cur:
                src  = cur[idx]
                base = src.get("name", "copy")
                new_name = base + "-copy"
                n = 2
                while any(s["name"] == new_name for s in sessions):
                    new_name = f"{base}-copy{n}"
                    n += 1
                dup = dict(src)
                dup["name"] = new_name
                sessions.append(dup)
                if _KEYRING and src.get("password"):
                    _kr_store(new_name, src["password"])
                save_sessions(sessions)
                last_mtime    = sessions_mtime()
                filter_active = False
                visible       = []
                idx           = len(sessions) - 1
                flash         = (f"  ✓ Duplicated as '{new_name}'", COLOR_SELECT)

        elif key == ord("d"):
            if cur:
                name    = cur[idx].get("name", "session")
                h2, w2  = stdscr.getmaxyx()
                safe_addstr(stdscr, h2 - 4, 2,
                            f"  Delete '{name}'? (y/n) "[:w2 - 4],
                            curses.color_pair(COLOR_ERROR) | curses.A_BOLD)
                stdscr.refresh()
                c = stdscr.getch()
                if c in (ord("y"), ord("Y")):
                    _kr_delete(name)
                    real_sess = cur[idx]
                    if real_sess in sessions:
                        sessions.remove(real_sess)
                    if filter_active and real_sess in visible:
                        visible.remove(real_sess)
                    idx        = clamp(idx, 0, max(0, len(active_sessions()) - 1))
                    save_sessions(sessions)
                    last_mtime = sessions_mtime()
                    flash      = (f"  ✓ Deleted '{name}'", COLOR_SELECT)

        elif key == ord("y"):
            if cur:
                sel   = cur[idx]
                text  = f"{sel.get('user','root')}@{sel['host']}:{sel.get('port',22)}"
                if _yank(text):
                    flash = (f"  ✓ Copied {text}", COLOR_SELECT)
                else:
                    flash = ("  ✗ pbcopy not available", COLOR_ERROR)

        elif key == ord("h"):
            cfg   = toggle_herdr(cfg)
            state = "ON" if cfg.get("use_herdr", True) else "OFF"
            flash = (f"  herdr {state}", COLOR_SELECT if cfg["use_herdr"] else COLOR_ERROR)

        elif key == ord("?"):
            run_help_view(stdscr)

        elif key == ord("u"):
            run_update_view(stdscr)


# ── connect ───────────────────────────────────────────────────────────────────

def do_connect(session: dict, cfg: dict) -> None:
    host      = session["host"]
    user      = session.get("user", "root")
    port      = int(session.get("port", 22))
    password  = session.get("password", "")
    use_herdr = cfg.get("use_herdr", True)

    print(f"→ connecting to {user}@{host}:{port}")
    try:
        if use_herdr:
            if not shutil.which("herdr"):
                print("Error: 'herdr' is not installed or not in PATH.")
                print("Tip: install herdr or toggle off herdr mode (press 'h' in main menu).")
                input("\nPress Enter to return…")
                return
            # herdr --remote handles its own SSH transport; key auth required
            url = f"ssh://{user}@{host}" if port == 22 else f"ssh://{user}@{host}:{port}"
            result = subprocess.run(["herdr", "--remote", url])
        else:
            SSM_KNOWN_HOSTS.parent.mkdir(parents=True, exist_ok=True)
            ssh_opts   = [
                "-p", str(port),
                "-o", "StrictHostKeyChecking=accept-new",
                "-o", f"UserKnownHostsFile={SSM_KNOWN_HOSTS}",
                "-o", "ConnectTimeout=5",
            ]
            ssh_target = [f"{user}@{host}"]
            env = os.environ.copy()
            if password:
                pw_opts = ["-o", "PubkeyAuthentication=no",
                           "-o", "PreferredAuthentications=password"]
                if shutil.which("sshpass"):
                    env["SSHPASS"] = password
                    result = subprocess.run(
                        ["sshpass", "-e", "ssh"] + ssh_opts + pw_opts + ssh_target, env=env)
                else:
                    print(
                        "Tip: install sshpass (`brew install hudochenkov/sshpass/sshpass`) "
                        "to use stored passwords automatically."
                    )
                    result = subprocess.run(["ssh"] + ssh_opts + pw_opts + ssh_target)
            else:
                result = subprocess.run(["ssh"] + ssh_opts + ssh_target)
    except KeyboardInterrupt:
        return
    except EOFError:
        return
    except Exception as e:
        print(f"\nError running SSH connection command: {e}")
        try:
            input("\nPress Enter to return…")
        except EOFError:
            pass
        return
    if result.returncode != 0:
        input(f"\nConnection failed (exit {result.returncode}). Press Enter to return…")


# ── cli helpers ───────────────────────────────────────────────────────────────

def _parse_hostspec(spec: str) -> dict:
    """Parse user@host[:port] into a minimal session dict."""
    user = "root"
    port = 22
    if "@" in spec:
        user, hostpart = spec.split("@", 1)
    else:
        hostpart = spec
    if ":" in hostpart:
        host, port_str = hostpart.rsplit(":", 1)
        try:
            port = int(port_str)
        except ValueError:
            print(f"ssm: invalid port in '{spec}'", file=sys.stderr)
            sys.exit(1)
    else:
        host = hostpart
    return {"host": host, "user": user, "port": port, "password": ""}


def _cli_list() -> None:
    sessions = load_sessions()
    cfg      = load_config()
    mode     = "herdr ON" if cfg.get("use_herdr", True) else "herdr OFF"
    if not sessions:
        print(f"No sessions saved.  [{mode}]")
        return
    print(f"  {'NAME':<20} {'HOST/IP':<24} {'USER':<12} PORT   [{mode}]")
    print("  " + "─" * 64)
    for s in sessions:
        print(f"  {s.get('name',''):<20} {s.get('host',''):<24}"
              f" {s.get('user','root'):<12} {s.get('port', 22)}")


def _cli_connect_name(name: str) -> None:
    sessions = load_sessions()
    cfg      = load_config()
    match    = next((s for s in sessions if s["name"] == name), None)
    if not match:
        # Fuzzy fallback: prefix match
        matches = [s for s in sessions if s["name"].startswith(name)]
        if len(matches) == 1:
            match = matches[0]
        elif len(matches) > 1:
            names = ", ".join(s["name"] for s in matches)
            print(f"ssm: '{name}' is ambiguous — matches: {names}", file=sys.stderr)
            sys.exit(1)
        else:
            print(f"ssm: no session named '{name}'", file=sys.stderr)
            sys.exit(1)
    do_connect(match, cfg)


def _cli_connect_direct(spec: str) -> None:
    cfg     = load_config()
    session = _parse_hostspec(spec)
    do_connect(session, cfg)


def _print_help() -> None:
    print("usage:")
    print("  ssm                       open TUI session manager")
    print("  ssm <name>                connect to saved session by name")
    print("  ssm -c user@host[:port]   connect directly to host")
    print("  ssm -l                    list saved sessions")
    print("  ssm -h                    show this help")


# ── entry point ───────────────────────────────────────────────────────────────

def main() -> None:
    while True:
        try:
            result = curses.wrapper(run_tui)
        except KeyboardInterrupt:
            sys.exit(0)

        if not result or result[0] != "connect":
            break

        do_connect(result[1], result[2])


if __name__ == "__main__":
    _args = sys.argv[1:]
    if not _args:
        main()
    elif _args[0] in ("-l", "--list"):
        _cli_list()
    elif _args[0] in ("-c", "--connect"):
        if len(_args) < 2:
            print("ssm: -c requires user@host[:port]", file=sys.stderr)
            sys.exit(1)
        _cli_connect_direct(_args[1])
    elif _args[0] in ("-h", "--help"):
        _print_help()
    elif _args[0].startswith("-"):
        print(f"ssm: unknown flag '{_args[0]}'  (try -h)", file=sys.stderr)
        sys.exit(1)
    else:
        _cli_connect_name(_args[0])
