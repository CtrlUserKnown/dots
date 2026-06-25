#!/usr/bin/env python3
"""SSH Session Manager — curses TUI for storing and launching SSH connections."""

import curses
import json
import os
import shutil
import subprocess
import sys
from pathlib import Path

SESSIONS_FILE = Path.home() / ".config" / "ssm" / "sessions.json"


# ── storage ──────────────────────────────────────────────────────────────────

def load_sessions() -> list:
    if not SESSIONS_FILE.exists():
        return []
    try:
        with open(SESSIONS_FILE) as f:
            return json.load(f)
    except (json.JSONDecodeError, OSError):
        return []


def save_sessions(sessions: list) -> None:
    SESSIONS_FILE.parent.mkdir(parents=True, exist_ok=True)
    with open(SESSIONS_FILE, "w") as f:
        json.dump(sessions, f, indent=2)


# ── curses helpers ────────────────────────────────────────────────────────────

COLOR_HEADER  = 1
COLOR_SELECT  = 2
COLOR_ERROR   = 3
COLOR_DIM     = 4
COLOR_SUCCESS = 5
COLOR_ACCENT  = 6


def init_colors() -> None:
    curses.use_default_colors()
    curses.init_pair(COLOR_HEADER,  curses.COLOR_CYAN,    -1)
    curses.init_pair(COLOR_SELECT,  curses.COLOR_GREEN,   -1)
    curses.init_pair(COLOR_ERROR,   curses.COLOR_RED,     -1)
    curses.init_pair(COLOR_DIM,     curses.COLOR_YELLOW,  -1)
    curses.init_pair(COLOR_SUCCESS, curses.COLOR_GREEN,   -1)
    curses.init_pair(COLOR_ACCENT,  curses.COLOR_MAGENTA, -1)


def clamp(val: int, lo: int, hi: int) -> int:
    return max(lo, min(hi, val))


def safe_addstr(win, y: int, x: int, text: str, attr: int = 0) -> None:
    h, w = win.getmaxyx()
    if y < 0 or y >= h or x < 0 or x >= w:
        return
    max_len = w - x - 1
    if max_len <= 0:
        return
    try:
        win.addstr(y, x, text[:max_len], attr)
    except curses.error:
        pass


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
    """Show add/edit form. Returns new session dict or None if cancelled."""
    title = "Edit Session" if existing else "Add Session"
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
    error = ""

    curses.curs_set(1)

    while True:
        stdscr.erase()
        h, w = stdscr.getmaxyx()

        bw = min(62, w - 4)
        bh = 14
        by = max(0, (h - bh) // 2)
        bx = max(0, (w - bw) // 2)

        # border top
        hdr = f"─ {title} "
        safe_addstr(stdscr, by, bx, hdr + "─" * max(0, bw - len(hdr)),
                    curses.color_pair(COLOR_HEADER) | curses.A_BOLD)

        # fields
        for i, (fname, is_pass) in enumerate(FIELD_DEFS):
            fy = by + 2 + i * 2
            val = values[i]
            display = "•" * len(val) if is_pass else val
            label_attr = curses.color_pair(COLOR_SELECT) | curses.A_BOLD if i == current \
                         else curses.color_pair(COLOR_DIM)
            safe_addstr(stdscr, fy, bx + 2, f"{fname:<10}", label_attr)
            field_w = bw - 15
            safe_addstr(stdscr, fy, bx + 13, "[" + f"{display:<{field_w}}"[:field_w] + "]")

        # error line
        if error:
            safe_addstr(stdscr, by + bh - 4, bx + 2, error[:bw - 4],
                        curses.color_pair(COLOR_ERROR))

        # border bottom
        safe_addstr(stdscr, by + bh - 3, bx, "─" * bw,
                    curses.color_pair(COLOR_HEADER))
        safe_addstr(stdscr, by + bh - 2, bx + 1,
                    "tab/↓ next  ↑ prev  enter save  esc cancel"[:bw - 2],
                    curses.color_pair(COLOR_DIM))

        # move cursor
        is_pass = FIELD_DEFS[current][1]
        display_val = "•" * len(values[current]) if is_pass else values[current]
        cx = bx + 14 + len(display_val)
        cy = by + 2 + current * 2
        try:
            stdscr.move(cy, min(cx, w - 2))
        except curses.error:
            pass

        stdscr.refresh()
        key = stdscr.getch()

        if key == 27:  # ESC
            curses.curs_set(0)
            return None

        elif key in (9, curses.KEY_DOWN):  # Tab / Down
            current = (current + 1) % len(FIELD_DEFS)
            error = ""

        elif key == curses.KEY_UP:
            current = (current - 1) % len(FIELD_DEFS)
            error = ""

        elif key in (curses.KEY_ENTER, 10, 13):
            if current < len(FIELD_DEFS) - 1:
                current += 1
                error = ""
            else:
                name = values[0].strip()
                host = values[1].strip()
                if not name:
                    error = "Name is required"
                    current = 0
                    continue
                if not host:
                    error = "Host/IP is required"
                    current = 1
                    continue
                try:
                    port = int(values[4].strip() or "22")
                except ValueError:
                    error = "Port must be a number"
                    current = 4
                    continue
                curses.curs_set(0)
                return {
                    "name": name,
                    "host": host,
                    "user": values[2].strip() or "root",
                    "password": values[3],
                    "port": port,
                }

        elif key in (curses.KEY_BACKSPACE, 127, 8):
            values[current] = values[current][:-1]

        elif 32 <= key <= 126:
            values[current] += chr(key)


# ── main TUI ──────────────────────────────────────────────────────────────────

def run_tui(stdscr) -> tuple | None:
    """Main list view. Returns ('connect', session) or None."""
    init_colors()
    curses.curs_set(0)
    stdscr.keypad(True)

    sessions = load_sessions()
    idx       = 0
    flash     = ("", 0)   # (message, color_pair)
    count_buf = ""         # accumulated digit prefix (e.g. "5" before j/k/G)
    pending_g = False      # True after a lone 'g' press, waiting for 'gg'

    while True:
        stdscr.erase()
        h, w = stdscr.getmaxyx()

        # ── header ──
        title = " SSH Session Manager "
        safe_addstr(stdscr, 0, 0, "─" * w, curses.color_pair(COLOR_HEADER) | curses.A_BOLD)
        safe_addstr(stdscr, 0, max(0, (w - len(title)) // 2), title,
                    curses.color_pair(COLOR_HEADER) | curses.A_BOLD)

        # ── column headings ──
        safe_addstr(stdscr, 2, 0, f"  {'NAME':<20} {'HOST/IP':<24} {'USER':<12} PORT",
                    curses.color_pair(COLOR_DIM) | curses.A_BOLD)
        safe_addstr(stdscr, 3, 0, "  " + "─" * min(w - 3, 64),
                    curses.color_pair(COLOR_DIM))

        # ── session rows ──
        list_top = 4
        list_h   = h - 7

        if not sessions:
            msg = "No sessions yet — press 'a' to add one"
            safe_addstr(stdscr, list_top + 2, max(0, (w - len(msg)) // 2), msg,
                        curses.color_pair(COLOR_DIM))
        else:
            idx    = clamp(idx, 0, len(sessions) - 1)
            scroll = max(0, idx - list_h + 1)
            for i, sess in enumerate(sessions[scroll: scroll + list_h]):
                real_i = i + scroll
                y      = list_top + i
                name   = sess.get("name", "")[:19]
                host   = sess.get("host", "")[:23]
                user   = sess.get("user", "root")[:11]
                port   = str(sess.get("port", 22))[:5]
                row    = f"  {name:<20} {host:<24} {user:<12} {port}"
                if real_i == idx:
                    safe_addstr(stdscr, y, 0, "▶", curses.color_pair(COLOR_SELECT) | curses.A_BOLD)
                    safe_addstr(stdscr, y, 1, row[1:], curses.A_BOLD)
                else:
                    safe_addstr(stdscr, y, 0, row)

        # ── footer ──
        msg_text, msg_color = flash
        safe_addstr(stdscr, h - 3, 0, "─" * w, curses.color_pair(COLOR_HEADER))
        if msg_text:
            safe_addstr(stdscr, h - 2, 2, msg_text[:w - 4], curses.color_pair(msg_color))
        elif pending_g:
            safe_addstr(stdscr, h - 2, 2, "g...", curses.color_pair(COLOR_ACCENT) | curses.A_BOLD)
        elif count_buf:
            safe_addstr(stdscr, h - 2, 2, f"[{count_buf}]", curses.color_pair(COLOR_ACCENT) | curses.A_BOLD)
        else:
            keys = " j/k↑↓ nav  gg/G top/bot  ^d/^u ^f/^b scroll  enter connect  a add  e edit  d del  q quit"
            safe_addstr(stdscr, h - 2, 0, keys[:w - 1], curses.color_pair(COLOR_DIM))

        stdscr.refresh()
        flash = ("", 0)  # clear after one draw

        # ── input ──
        key = stdscr.getch()
        ch  = chr(key) if 32 <= key <= 126 else ""

        # accumulate count digits (leading 0 only valid after another digit)
        if ch.isdigit() and (ch != "0" or count_buf) and not pending_g:
            count_buf += ch
            continue  # redraw to show count, wait for motion key

        count     = int(count_buf) if count_buf else 1
        had_count = bool(count_buf)
        count_buf = ""

        # ── g / gg ──
        if key == ord("g"):
            if pending_g:    # second g → jump to top
                idx       = 0
                pending_g = False
            else:            # first g → wait
                pending_g = True
            continue         # always redraw without falling through

        # any non-g key cancels a pending g
        pending_g = False

        # ── navigation ──
        if key in (ord("q"), ord("Q"), 27):
            return None

        elif key in (ord("j"), curses.KEY_DOWN):
            if sessions:
                idx = clamp(idx + count, 0, len(sessions) - 1)

        elif key in (ord("k"), curses.KEY_UP):
            if sessions:
                idx = clamp(idx - count, 0, len(sessions) - 1)

        elif key == ord("G"):
            if sessions:
                # {n}G → go to nth entry (1-based); bare G → last
                idx = clamp(count - 1 if had_count else len(sessions) - 1,
                            0, len(sessions) - 1)

        elif key == 4:   # Ctrl+d — half page down
            if sessions:
                idx = clamp(idx + max(1, list_h // 2), 0, len(sessions) - 1)

        elif key == 21:  # Ctrl+u — half page up
            if sessions:
                idx = clamp(idx - max(1, list_h // 2), 0, len(sessions) - 1)

        elif key == 6:   # Ctrl+f — full page down
            if sessions:
                idx = clamp(idx + list_h, 0, len(sessions) - 1)

        elif key == 2:   # Ctrl+b — full page up
            if sessions:
                idx = clamp(idx - list_h, 0, len(sessions) - 1)

        # ── actions ──
        elif key in (curses.KEY_ENTER, 10, 13):
            if sessions:
                return ("connect", sessions[idx])

        elif key == ord("a"):
            result = run_form(stdscr)
            if result:
                if any(s["name"] == result["name"] for s in sessions):
                    flash = (f"Name '{result['name']}' already exists", COLOR_ERROR)
                else:
                    sessions.append(result)
                    save_sessions(sessions)
                    idx   = len(sessions) - 1
                    flash = (f"Added '{result['name']}'", COLOR_SUCCESS)

        elif key == ord("e"):
            if sessions:
                result = run_form(stdscr, sessions[idx])
                if result:
                    dup = any(
                        s["name"] == result["name"] and i != idx
                        for i, s in enumerate(sessions)
                    )
                    if dup:
                        flash = (f"Name '{result['name']}' already exists", COLOR_ERROR)
                    else:
                        sessions[idx] = result
                        save_sessions(sessions)
                        flash = (f"Updated '{result['name']}'", COLOR_SUCCESS)

        elif key == ord("d"):
            if sessions:
                name    = sessions[idx].get("name", "session")
                confirm = f" Delete '{name}'? (y/n) "
                safe_addstr(stdscr, h - 2, 2, confirm[:w - 4],
                            curses.color_pair(COLOR_ERROR) | curses.A_BOLD)
                stdscr.refresh()
                c = stdscr.getch()
                if c in (ord("y"), ord("Y")):
                    sessions.pop(idx)
                    idx   = clamp(idx, 0, max(0, len(sessions) - 1))
                    save_sessions(sessions)
                    flash = (f"Deleted '{name}'", COLOR_DIM)


# ── connect ───────────────────────────────────────────────────────────────────

def do_connect(session: dict) -> None:
    host     = session["host"]
    user     = session.get("user", "root")
    port     = str(session.get("port", 22))
    password = session.get("password", "")

    ssh_opts = [
        "-p", port,
        "-o", "StrictHostKeyChecking=no",
        "-o", "UserKnownHostsFile=/dev/null",
        "-o", "ConnectTimeout=5",
    ]
    ssh_target = [f"{user}@{host}"]

    env = os.environ.copy()
    if password:
        pw_opts = ["-o", "PubkeyAuthentication=no", "-o", "PreferredAuthentications=password"]
        if shutil.which("sshpass"):
            cmd = ["sshpass", "-e", "ssh"] + ssh_opts + pw_opts + ssh_target
            env["SSHPASS"] = password
        else:
            print(
                "Tip: install sshpass (`brew install hudochenkov/sshpass/sshpass`) "
                "to use stored passwords automatically."
            )
            cmd = ["ssh"] + ssh_opts + pw_opts + ssh_target
    else:
        cmd = ["ssh"] + ssh_opts + ssh_target

    print(f"→ connecting to {user}@{host}:{port}")
    try:
        result = subprocess.run(cmd, env=env)
    except KeyboardInterrupt:
        return
    if result.returncode != 0:
        input(f"\nConnection failed (exit {result.returncode}). Press Enter to return…")


# ── entry point ───────────────────────────────────────────────────────────────

def main() -> None:
    while True:
        try:
            result = curses.wrapper(run_tui)
        except KeyboardInterrupt:
            sys.exit(0)

        if not result or result[0] != "connect":
            break

        do_connect(result[1])


if __name__ == "__main__":
    main()
