#!/usr/bin/env python3
"""Shared curses utilities for dots.py and ssm.py."""

import curses
import subprocess
from pathlib import Path

DOTS_DIR = Path(__file__).resolve().parents[3]

# ── color pair IDs ────────────────────────────────────────────────────────────

COLOR_HEADER = 1
COLOR_SELECT = 2
COLOR_ERROR  = 3
COLOR_DIM    = 4


def init_colors() -> None:
    curses.use_default_colors()
    curses.init_pair(COLOR_HEADER, curses.COLOR_CYAN,   -1)
    curses.init_pair(COLOR_SELECT, curses.COLOR_GREEN,  -1)
    curses.init_pair(COLOR_ERROR,  curses.COLOR_RED,    -1)
    curses.init_pair(COLOR_DIM,    curses.COLOR_YELLOW, -1)


# ── drawing helpers ───────────────────────────────────────────────────────────

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


def draw_header(win, title: str, version: str = "") -> None:
    _, w = win.getmaxyx()
    attr = curses.color_pair(COLOR_HEADER) | curses.A_BOLD
    ver  = f" v{version} " if version else ""
    line = ["─"] * w
    for i, ch in enumerate(title):
        if 4 + i < w:
            line[4 + i] = ch
    if ver:
        vx = max(4 + len(title), w - len(ver) - 2)
        for i, ch in enumerate(ver):
            if vx + i < w:
                line[vx + i] = ch
    safe_addstr(win, 0, 0, "".join(line), attr)


def draw_footer(win, hint: str) -> None:
    h, w = win.getmaxyx()
    safe_addstr(win, h - 3, 0, "─" * w, curses.color_pair(COLOR_HEADER))
    safe_addstr(win, h - 2, 0, hint[:w - 1], curses.color_pair(COLOR_DIM))


def draw_desc(win, text: str, flash: tuple[str, int] | None = None) -> None:
    """Desc row at h-4. flash is (message, COLOR_* id) or None."""
    h, _ = win.getmaxyx()
    if flash and flash[0]:
        safe_addstr(win, h - 4, 2, flash[0],
                    curses.color_pair(flash[1]) | curses.A_BOLD)
    else:
        safe_addstr(win, h - 4, 2, "›", curses.color_pair(COLOR_DIM))
        safe_addstr(win, h - 4, 4, text,
                    curses.color_pair(COLOR_HEADER) | curses.A_BOLD)


def clamp(val: int, lo: int, hi: int) -> int:
    return max(lo, min(hi, val))


# ── update check ──────────────────────────────────────────────────────────────

def check_upstream(dots_dir: Path) -> tuple[int, str]:
    """Fetch upstream and return (commits_behind, upstream_version). -1 = error."""
    if not (dots_dir / ".git").exists():
        return -1, ""
    try:
        subprocess.run(
            ["git", "-C", str(dots_dir), "fetch", "--depth", "1", "--tags", "origin"],
            capture_output=True, timeout=15,
        )
        r = subprocess.run(
            ["git", "-C", str(dots_dir), "rev-list", "--count", "HEAD..origin/HEAD"],
            capture_output=True, text=True, timeout=5,
        )
        behind = int(r.stdout.strip() or "0")
        ver    = ""
        if behind:
            rv = subprocess.run(
                ["git", "-C", str(dots_dir), "describe", "--tags", "--abbrev=0", "origin/HEAD"],
                capture_output=True, text=True, timeout=5,
            )
            ver = rv.stdout.strip().lstrip("v")
        return behind, ver
    except Exception:
        return -1, ""
