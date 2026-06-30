#!/usr/bin/env python3
"""dots — dotfiles manager TUI."""

import curses
import datetime
import json
import os
import re
import shutil
import subprocess
import sys
from pathlib import Path
from typing import Any, NamedTuple, Optional

DOTS_DIR     = Path(__file__).resolve().parents[3]
SETTINGS_FILE = DOTS_DIR / ".settings"
VERSION       = os.environ.get("DOTS_VERSION", "")

VERSION_STAMP = Path.home() / ".config" / "zsh" / ".version_stamp"
CHECK_ACTION  = "__check_updates__"

UPDATE_FREQ_OPTIONS = [
    (60,    "Every hour"),
    (360,   "Every 6 hours"),
    (720,   "Every 12 hours"),
    (1440,  "Daily"),
    (4320,  "Every 3 days"),
    (10080, "Weekly"),
]

# ── data structures ───────────────────────────────────────────────────────────

class Dep(NamedTuple):
    bin:      str        # binary checked with shutil.which (empty = cask/no-cli)
    brew:     str        # homebrew formula
    dnf:      str        # fedora dnf package
    apt:      str        # debian/ubuntu apt package
    desc:     str
    category: str        # "required" | "optional" | "dev"
    tap:      str  = ""  # brew tap to enable first
    cask:     bool = False


DEPS: list[Dep] = [
    # ── required ────────────────────────────────────────────────────────────
    Dep("git",       "git",       "git",       "git",       "version control",                    "required"),
    Dep("eza",       "eza",       "eza",       "eza",       "modern ls replacement",               "required"),
    Dep("bat",       "bat",       "bat",       "bat",       "fzf previewer & syntax highlighter",  "required"),
    Dep("fd",        "fd",        "fd-find",   "fd-find",   "fast file finder (fzf command)",      "required"),
    Dep("fzf",       "fzf",       "fzf",       "fzf",       "fuzzy finder",                        "required"),
    Dep("fastfetch", "fastfetch", "fastfetch", "fastfetch", "system info at shell start",          "required"),
    Dep("zoxide",    "zoxide",    "zoxide",    "zoxide",    "smarter cd",                          "required"),
    Dep("nvim",      "neovim",    "neovim",    "neovim",    "text editor ($EDITOR)",               "required"),
    # ── optional ────────────────────────────────────────────────────────────
    Dep("herdr",     "herdr",     "",          "",          "terminal multiplexer (mux alias)",    "optional", "charmbracelet/tap"),
    Dep("btop",      "btop",      "btop",      "btop",      "system monitor (b alias)",            "optional"),
    Dep("lazygit",   "lazygit",   "lazygit",   "lazygit",   "git TUI (lz alias)",                 "optional"),
    Dep("yazi",      "yazi",      "",          "",          "file manager (y alias)",              "optional"),
    Dep("carapace",  "carapace",  "carapace",  "",          "shell completions engine",            "optional"),
    # ── dev ─────────────────────────────────────────────────────────────────
    # runtime / languages
    Dep("java",      "openjdk@21","java-21-openjdk","default-jdk","Java Development Kit (LTS)", "dev"),
    Dep("go",        "go",        "golang",    "golang",    "Go programming language",             "dev"),
    Dep("crystal",   "crystal",   "",          "",          "Crystal programming language",        "dev"),
    Dep("lua",       "lua",       "lua",       "lua",       "Lua scripting language",              "dev"),
    Dep("mvn",       "maven",     "maven",     "maven",     "Java / Maven build tool",             "dev"),
    Dep("gradle",    "gradle",    "gradle",    "gradle",    "Gradle build tool",                   "dev"),
    Dep("dotnet",    "dotnet",    "",          "",          ".NET SDK",                            "dev"),
    # build tools
    Dep("cmake",     "cmake",     "cmake",     "cmake",     "cross-platform build system",         "dev"),
    Dep("gcc",       "gcc",       "gcc",       "gcc",       "GNU C/C++ compiler",                  "dev"),
    Dep("rg",        "ripgrep",   "ripgrep",   "ripgrep",   "fast grep replacement",               "dev"),
    Dep("shellcheck","shellcheck","ShellCheck","shellcheck","shell script linter",                  "dev"),
    Dep("gh",        "gh",        "gh",        "gh",        "GitHub CLI",                          "dev"),
    Dep("glow",      "glow",      "",          "",          "markdown renderer",                   "dev"),
    Dep("typst",     "typst",     "",          "",          "markup typesetting system",            "dev"),
    Dep("stow",      "stow",      "stow",      "stow",      "symlink farm manager",                "dev"),
    # devops / infra
    Dep("docker",    "docker",    "",          "",          "container runtime",                   "dev"),
    Dep("psql",      "postgresql@14","","",    "PostgreSQL database",                             "dev"),
    Dep("nmap",      "nmap",      "nmap",      "nmap",      "network scanner",                     "dev"),
    # media / utilities
    Dep("ffmpeg",    "ffmpeg",    "ffmpeg",    "ffmpeg",    "audio/video converter",               "dev"),
    Dep("yt-dlp",    "yt-dlp",    "",          "",          "video downloader",                    "dev"),
    Dep("gemini",    "gemini-cli","",          "",          "Google Gemini CLI",                   "dev"),
]

# Symlinks required for the config to work
SYMLINKS = [
    (Path.home() / ".config/bat",       DOTS_DIR / "src/bat"),
    (Path.home() / ".config/fastfetch", DOTS_DIR / "src/fastfetch"),
    (Path.home() / ".config/ghostty",   DOTS_DIR / "src/ghostty"),
    (Path.home() / ".config/herdr",     DOTS_DIR / "src/herdr"),
    (Path.home() / ".config/zsh",       DOTS_DIR / "src/zsh/zsh"),
    (Path.home() / ".zshrc",            DOTS_DIR / "src/zsh/.zshrc"),
]

# Shell plugins checked by file/dir existence
PLUGINS = [
    ("zsh-autosuggestions",          "fish-like suggestions as you type", [
        Path("/opt/homebrew/share/zsh-autosuggestions/zsh-autosuggestions.zsh"),
        Path("/usr/share/zsh-autosuggestions/zsh-autosuggestions.zsh"),
    ]),
    ("zsh-syntax-highlighting",      "color-codes commands as you type", [
        Path("/opt/homebrew/share/zsh-syntax-highlighting/zsh-syntax-highlighting.zsh"),
        Path("/usr/share/zsh-syntax-highlighting/zsh-syntax-highlighting.zsh"),
    ]),
    ("zsh-history-substring-search", "search history by typing a fragment", [
        Path.home() / ".config/zsh/plugins/zsh-history-substring-search",
        Path("/opt/homebrew/share/zsh-history-substring-search/zsh-history-substring-search.zsh"),
    ]),
    ("fzf-tab",                      "fuzzy-search tab completions", [
        Path.home() / ".config/zsh/plugins/fzf-tab",
    ]),
]

# ── menu action sentinels ─────────────────────────────────────────────────────

HEALTH_ACTION   = "__health__"
THEME_ACTION    = "__theme__"
SETTINGS_ACTION = "__settings__"
DEV_ACTION      = "__dev__"
EDIT_ACTION     = "__edit__"
ALIAS_ACTION    = "__alias__"
PERSONAL_ACTION = "__personal__"
GIT_ACTION      = "__git__"
ALIASES_ACTION  = "__aliases__"
RELOAD_ACTION   = "__reload__"
LOGS_ACTION     = "__logs__"
RESET_ACTION    = "__reset__"
SEPARATOR       = "__sep__"

# Config files exposed in dev mode → Edit Configs
EDIT_CONFIGS = [
    ("Terminal / ZSH",  DOTS_DIR / "src/zsh/",            "shell config, prompt, plugins & keybindings"),
    ("Ghostty",         DOTS_DIR / "src/ghostty/config",  "terminal emulator appearance & behavior"),
    ("Aliases",         DOTS_DIR / "src/zsh/zsh/.aliases","custom command shortcuts"),
]

MIN_H, MIN_W = 14, 50

# ── settings ──────────────────────────────────────────────────────────────────

_DEFAULT_SETTINGS = {
    "update_check": True,
    "greeting":     True,
    "update_frequency": 1440,
}


def load_settings() -> dict:
    try:
        return {**_DEFAULT_SETTINGS, **json.loads(SETTINGS_FILE.read_text())}
    except Exception:
        return dict(_DEFAULT_SETTINGS)


def save_settings(s: dict) -> None:
    try:
        SETTINGS_FILE.write_text(json.dumps(s, indent=2))
    except Exception:
        pass


def is_dev_mode() -> bool:
    return (DOTS_DIR / ".developer").exists()


def set_dev_mode(on: bool) -> None:
    marker = DOTS_DIR / ".developer"
    if on:
        marker.touch()
    elif marker.exists():
        marker.unlink()


def build_menu(dev: bool) -> list[tuple]:
    base = [
        ("Health",       HEALTH_ACTION,   "check symlinks, tools & plugins"),
        ("Theme",        THEME_ACTION,    "pick a terminal color theme"),
        ("Aliases",      ALIASES_ACTION,  "view custom shell aliases"),
        ("Settings",     SETTINGS_ACTION, "configure dots preferences"),
        ("Developer",    DEV_ACTION,      "enable developer mode for advanced options"),
    ]
    if not dev:
        return base
    dev_items = [
        ("Developer",    DEV_ACTION,      "disable developer mode"),
        ("──",           SEPARATOR,       ""),
        ("Edit Configs", EDIT_ACTION,     "open config files directly in $EDITOR"),
        ("Add Alias",    ALIAS_ACTION,    "create a new shell alias interactively"),
        ("Dev Packages",  PERSONAL_ACTION, "manage developer package list"),
        ("Git",          GIT_ACTION,      "status, log, pull & push dotfiles repo"),
        ("Reload",       RELOAD_ACTION,   "instructions to reload your shell config"),
        ("View Logs",    LOGS_ACTION,     "show update checker history"),
        ("Reset",        RESET_ACTION,    "wipe all dots settings and start fresh"),
    ]
    return [
        ("Health",   HEALTH_ACTION,   "check symlinks, tools & plugins"),
        ("Theme",    THEME_ACTION,    "pick a terminal color theme"),
        ("Aliases",  ALIASES_ACTION,  "view custom shell aliases"),
        ("Settings", SETTINGS_ACTION, "configure dots preferences"),
    ] + dev_items

# ── colors ────────────────────────────────────────────────────────────────────

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


def draw_header(win, title: str) -> None:
    _, w = win.getmaxyx()
    attr = curses.color_pair(COLOR_HEADER) | curses.A_BOLD
    safe_addstr(win, 0, 0, "─" * w, attr)
    safe_addstr(win, 0, max(0, (w - len(title)) // 2), title, attr)
    if VERSION:
        ver = f" v{VERSION} "
        safe_addstr(win, 0, max(0, w - len(ver) - 1), ver, attr)


def draw_footer(win, hint: str) -> None:
    h, w = win.getmaxyx()
    safe_addstr(win, h - 3, 0, "─" * w, curses.color_pair(COLOR_HEADER))
    safe_addstr(win, h - 2, 0, hint[:w - 1], curses.color_pair(COLOR_DIM))


def draw_desc(win, text: str, flash: str = "") -> None:
    h, _ = win.getmaxyx()
    if flash:
        safe_addstr(win, h - 4, 2, flash,
                    curses.color_pair(COLOR_SELECT) | curses.A_BOLD)
    else:
        safe_addstr(win, h - 4, 2, "›", curses.color_pair(COLOR_DIM))
        safe_addstr(win, h - 4, 4, text,
                    curses.color_pair(COLOR_HEADER) | curses.A_BOLD)


def show_message(stdscr, title: str, lines: list[str]) -> None:
    """Blocking modal message — press any key to dismiss."""
    while True:
        stdscr.erase()
        h, w = stdscr.getmaxyx()
        draw_header(stdscr, title)
        for i, line in enumerate(lines):
            safe_addstr(stdscr, 2 + i, 2, line)
        draw_footer(stdscr, " press any key to continue ")
        stdscr.refresh()
        if stdscr.getch() != curses.KEY_RESIZE:
            return


def show_output(stdscr, title: str, text: str) -> None:
    """Scrollable read-only output view."""
    lines  = text.splitlines()
    offset = 0
    while True:
        stdscr.erase()
        h, w = stdscr.getmaxyx()
        draw_header(stdscr, title)
        visible = h - 6
        for i, line in enumerate(lines[offset: offset + visible]):
            safe_addstr(stdscr, 2 + i, 2, line)
        draw_footer(stdscr, " j/k scroll  q back ")
        stdscr.refresh()
        key = stdscr.getch()
        if key in (ord("q"), ord("Q"), 27):
            return
        elif key in (ord("j"), curses.KEY_DOWN):
            offset = min(offset + 1, max(0, len(lines) - visible))
        elif key in (ord("k"), curses.KEY_UP):
            offset = max(0, offset - 1)


# ── editor / launcher ─────────────────────────────────────────────────────────

def resolve_editor() -> Optional[str]:
    cand = os.environ.get("EDITOR", "")
    if cand and shutil.which(cand):
        return cand
    for fb in ("nvim", "vim", "nano"):
        if shutil.which(fb):
            return fb
    return None


def is_remote() -> bool:
    if os.environ.get("SSH_CLIENT") or os.environ.get("SSH_TTY"):
        return True
    if not os.environ.get("TERM_PROGRAM") and not os.environ.get("DISPLAY"):
        return True
    return False


def open_file(path: str) -> None:
    if is_remote() and shutil.which("herdr"):
        editor = resolve_editor() or "nvim"
        os.execvp("herdr", ["herdr", editor, path])
        return
    editor = resolve_editor()
    if not editor:
        print("dots: no editor found — set $EDITOR or install nvim/vim/nano",
              file=sys.stderr)
        sys.exit(1)
    os.execvp(editor, [editor, path])


# ── symlink helpers ───────────────────────────────────────────────────────────

def check_symlink(link: Path, target: Path) -> str:
    if not link.is_symlink() and not link.exists(): return "MISSING"
    if not link.is_symlink():                        return "NOT A LINK"
    if not link.exists():                            return "BROKEN"
    if link.resolve() == target.resolve():           return "OK"
    return "WRONG TARGET"


def read_version_header(path: Path) -> str:
    """Return the dotfiles version string from a file header comment, or '' if not found."""
    try:
        for line in path.read_text(errors="ignore").splitlines()[:10]:
            m = re.match(r"#\s*dotfiles\s+v(\S+)", line)
            if m:
                return m.group(1)
    except OSError:
        pass
    return ""


def _backup_path(path: Path) -> Path:
    stamp = datetime.date.today().strftime("%Y%m%d")
    base  = path.parent / f"{path.name}.bak.{stamp}"
    if not base.exists():
        return base
    i = 1
    while True:
        candidate = path.parent / f"{path.name}.bak.{stamp}.{i}"
        if not candidate.exists():
            return candidate
        i += 1


def repair_symlink(link: Path, target: Path) -> bool:
    if link.exists() and not link.is_symlink():
        if link.is_dir():
            try:
                shutil.move(str(link), str(_backup_path(link)))
            except OSError:
                return False
        else:
            # For regular files check for a dotfiles version header.
            # If the version matches the repo file, it is already current — skip.
            repo_ver  = read_version_header(target)
            file_ver  = read_version_header(link)
            if repo_ver and file_ver == repo_ver:
                return True
            try:
                shutil.move(str(link), str(_backup_path(link)))
            except OSError:
                return False
    if link.is_symlink():
        link.unlink()
    link.symlink_to(target)
    return True


def repair_all() -> None:
    ok = repaired = skipped = 0
    for link, target in SYMLINKS:
        status = check_symlink(link, target)
        label  = "~/" + str(link.relative_to(Path.home()))
        if status == "OK":
            print(f"  ✓  {label}"); ok += 1
        else:
            if repair_symlink(link, target):
                print(f"  →  {label}  ({status} → repaired)"); repaired += 1
            else:
                print(f"  ✗  {label}  (skipped — file in the way)"); skipped += 1
    print(f"\n  {ok} OK, {repaired} repaired, {skipped} skipped")
    if skipped:
        sys.exit(1)


# ── dependency helpers ────────────────────────────────────────────────────────

def detect_pkg_manager() -> Optional[str]:
    if shutil.which("brew"): return "brew"
    if shutil.which("dnf"):  return "dnf"
    if shutil.which("apt"):  return "apt"
    return None


_brew_casks: Optional[set] = None


def get_brew_casks() -> set:
    global _brew_casks
    if _brew_casks is None:
        if shutil.which("brew"):
            r = subprocess.run(["brew", "list", "--cask"],
                               capture_output=True, text=True)
            _brew_casks = set(r.stdout.split()) if r.returncode == 0 else set()
        else:
            _brew_casks = set()
    return _brew_casks


def check_dep(dep: Dep) -> bool:
    if dep.cask:
        return dep.brew in get_brew_casks()
    if not dep.bin:
        return False
    alt = {"fd": ["fd", "fdfind"], "bat": ["bat", "batcat"]}
    return any(shutil.which(b) for b in alt.get(dep.bin, [dep.bin]))


def check_plugin(paths: list[Path]) -> bool:
    return any(p.exists() for p in paths)


def install_deps_cli(deps: list[Dep]) -> None:
    # DOTS_SKIP_BINS=nvim,fd  — comma-separated binaries to skip (used by setup.sh)
    skip = {b.strip() for b in os.environ.get("DOTS_SKIP_BINS", "").split(",") if b.strip()}
    if skip:
        deps = [d for d in deps if d.bin not in skip and d.brew not in skip]
    if not deps:
        print("  ✓ All dependencies already installed.")
        return
    pm = detect_pkg_manager()
    if not pm:
        print("  ⚠  No supported package manager found (brew / dnf / apt).")
        return
    for dep in deps:
        label = dep.brew or dep.bin
        print(f"\n  Installing {label}...")
        if pm == "brew":
            if dep.tap:
                subprocess.run(["brew", "tap", dep.tap], check=False)
            if dep.brew:
                cmd = ["brew", "install"]
                if dep.cask:
                    cmd.append("--cask")
                subprocess.run(cmd + [dep.brew], check=False)
            else:
                print(f"  ⚠  {label}: no brew formula — install manually")
        elif pm == "dnf":
            if dep.dnf:
                subprocess.run(["sudo", "dnf", "install", "-y", dep.dnf], check=False)
            else:
                print(f"  ⚠  {label}: not in dnf repos — install manually")
        elif pm == "apt":
            if dep.apt:
                subprocess.run(["sudo", "apt", "install", "-y", dep.apt], check=False)
            else:
                print(f"  ⚠  {label}: not in apt repos — install manually")
    print("\n  Done.")


def check_deps_cli() -> None:
    pm = detect_pkg_manager()
    print(f"\n  Package manager: {pm or 'none detected'}\n")
    any_missing = False
    for dep in DEPS:
        ok     = check_dep(dep)
        tag    = f"[{dep.category[:3]}]"
        sym    = "✓" if ok else "✗"
        status = "installed" if ok else "MISSING"
        print(f"  {sym}  {dep.bin or dep.brew:<14} {dep.desc:<38} {tag}  {status}")
        if not ok:
            any_missing = True
    print()
    if any_missing:
        sys.exit(1)


# ── theme helpers ─────────────────────────────────────────────────────────────

def list_ghostty_themes() -> list[str]:
    if not shutil.which("ghostty"):
        return []
    r = subprocess.run(["ghostty", "+list-themes"], capture_output=True, text=True)
    themes = []
    for line in r.stdout.strip().splitlines():
        line = line.strip()
        if " (" in line:
            line = line[:line.rfind(" (")]
        if line:
            themes.append(line)
    return themes


def get_current_theme() -> str:
    config = DOTS_DIR / "src/ghostty/config"
    try:
        for line in config.read_text().splitlines():
            m = re.match(r'^\s*theme\s*=\s*"?([^"]+)"?\s*$', line)
            if m:
                return m.group(1).strip()
    except Exception:
        pass
    return ""


def set_ghostty_theme(name: str) -> bool:
    config = DOTS_DIR / "src/ghostty/config"
    try:
        text = config.read_text()
        text = re.sub(r'^theme\s*=.*$', f'theme = "{name}"',
                      text, flags=re.MULTILINE)
        config.write_text(text)
        s = load_settings()
        s["theme"] = name
        save_settings(s)
        return True
    except Exception:
        return False


# ── update check helpers ──────────────────────────────────────────────────────


def check_upstream() -> tuple[int, str]:
    """Fetch upstream and return (commits_behind, upstream_version). -1 = error."""
    if not (DOTS_DIR / ".git").exists():
        return -1, ""
    try:
        subprocess.run(
            ["git", "-C", str(DOTS_DIR), "fetch", "--depth", "1", "--tags", "origin"],
            capture_output=True, timeout=15,
        )
        r = subprocess.run(
            ["git", "-C", str(DOTS_DIR), "rev-list", "--count", "HEAD..origin/HEAD"],
            capture_output=True, text=True, timeout=5,
        )
        behind = int(r.stdout.strip() or "0")
        ver = ""
        if behind:
            rv = subprocess.run(
                ["git", "-C", str(DOTS_DIR), "describe", "--tags", "--abbrev=0", "origin/HEAD"],
                capture_output=True, text=True, timeout=5,
            )
            ver = rv.stdout.strip().lstrip("v")
        return behind, ver
    except Exception:
        return -1, ""


def do_pull() -> tuple[bool, str]:
    """Fast-forward pull + symlink repair. Returns (ok, new_version_or_msg)."""
    try:
        r = subprocess.run(
            ["git", "-C", str(DOTS_DIR), "pull", "--ff-only"],
            capture_output=True, text=True, timeout=30,
        )
        if r.returncode != 0:
            return False, r.stderr.strip() or "pull failed"
        subprocess.run(
            ["python3", str(DOTS_DIR / "src/zsh/zsh/dots.py"), "--repair-symlinks"],
            capture_output=True, timeout=15,
        )
        rv = subprocess.run(
            ["git", "-C", str(DOTS_DIR), "describe", "--tags", "--abbrev=0"],
            capture_output=True, text=True, timeout=5,
        )
        new_ver = rv.stdout.strip().lstrip("v")
        if new_ver:
            try:
                VERSION_STAMP.write_text(new_ver)
            except Exception:
                pass
        return True, new_ver
    except subprocess.TimeoutExpired:
        return False, "timed out"
    except Exception as e:
        return False, str(e)


# ── health view ───────────────────────────────────────────────────────────────

def run_health_view(stdscr) -> None:
    flash = ""
    idx   = 0  # navigable item index

    while True:
        stdscr.erase()
        h, w = stdscr.getmaxyx()
        draw_header(stdscr, " health ")

        # Build flat display list: ("section"|"item", label, ok, desc)
        display: list[tuple] = []
        nav_descs: list[str] = []  # description for each navigable item

        display.append(("section", "symlinks", True, ""))
        for link, target in SYMLINKS:
            ok     = check_symlink(link, target) == "OK"
            label  = "~/" + str(link.relative_to(Path.home()))
            try:
                tstr = "~/" + str(target.relative_to(Path.home()))
            except ValueError:
                tstr = str(target)
            display.append(("item", label, ok, f"→ {tstr}"))
            nav_descs.append(f"→ {tstr}")

        display.append(("section", "tools", True, ""))
        for dep in DEPS:
            if dep.category not in ("required", "optional"):
                continue
            ok  = check_dep(dep)
            tag = f"[{dep.category[:3]}]"
            display.append(("item", f"{dep.bin or dep.brew:<14} {tag}", ok, dep.desc))
            nav_descs.append(dep.desc)

        display.append(("section", "plugins", True, ""))
        for name, desc, paths in PLUGINS:
            ok = check_plugin(paths)
            display.append(("item", name, ok, desc))
            nav_descs.append(desc)

        # Count navigable items
        nav_total = sum(1 for kind, *_ in display if kind == "item")
        idx = max(0, min(idx, nav_total - 1))

        visible_rows = h - 6
        # Build scroll offset to keep idx in view
        # Map idx → display row
        nav_i = -1
        idx_display_row = 0
        for di, (kind, *_) in enumerate(display):
            if kind == "item":
                nav_i += 1
                if nav_i == idx:
                    idx_display_row = di
                    break

        scroll = max(0, idx_display_row - visible_rows // 2)
        scroll = min(scroll, max(0, len(display) - visible_rows))

        # Render visible slice
        nav_i = -1
        for di, (kind, label, ok, desc) in enumerate(display):
            row = 2 + di - scroll
            if row < 2 or row >= 2 + visible_rows:
                if kind == "item":
                    nav_i += 1
                continue

            if kind == "section":
                safe_addstr(stdscr, row, 2, label,
                            curses.color_pair(COLOR_DIM) | curses.A_BOLD)
            else:
                nav_i += 1
                cursor = "▶" if nav_i == idx else " "
                sym    = "✓" if ok else "✗"
                sym_color = COLOR_SELECT if ok else COLOR_ERROR
                safe_addstr(stdscr, row, 0, cursor,
                            curses.color_pair(sym_color) | curses.A_BOLD)
                safe_addstr(stdscr, row, 2, sym,
                            curses.color_pair(sym_color) | curses.A_BOLD)
                safe_addstr(stdscr, row, 4, label,
                            0 if ok else curses.A_BOLD)

        desc_text = nav_descs[idx] if nav_descs else ""
        draw_desc(stdscr, desc_text, flash)

        any_broken = any(check_symlink(l, t) != "OK" for l, t in SYMLINKS)
        any_missing_tools = any(
            not check_dep(d) for d in DEPS if d.category in ("required", "optional")
        )
        hints = []
        if any_broken:
            hints.append("r repair symlinks")
        if any_missing_tools:
            hints.append("i install missing tools")
        hints.append("q back")
        draw_footer(stdscr, " j/k navigate  " + "  ".join(hints))
        stdscr.refresh()
        flash = ""

        key = stdscr.getch()
        if key in (ord("q"), ord("Q"), 27):
            return
        elif key in (ord("j"), curses.KEY_DOWN):
            idx = min(idx + 1, nav_total - 1)
        elif key in (ord("k"), curses.KEY_UP):
            idx = max(0, idx - 1)
        elif key == ord("r") and any_broken:
            rep = skp = 0
            for link, target in SYMLINKS:
                if check_symlink(link, target) != "OK":
                    if repair_symlink(link, target): rep += 1
                    else: skp += 1
            parts = []
            if rep: parts.append(f"{rep} repaired")
            if skp: parts.append(f"{skp} skipped")
            flash = "  ".join(parts)
        elif key == ord("i") and any_missing_tools:
            missing = [d for d in DEPS
                       if d.category in ("required", "optional") and not check_dep(d)]
            return ("install", missing)  # handled by main loop


# ── theme view ────────────────────────────────────────────────────────────────

def run_theme_view(stdscr) -> None:
    flash    = ""
    themes   = list_ghostty_themes()
    current  = get_current_theme()

    if not themes:
        show_message(stdscr, " theme ", [
            "  Ghostty is not installed or not in PATH.",
            "  Install Ghostty to use the theme picker.",
        ])
        return

    idx    = next((i for i, t in enumerate(themes) if t == current), 0)
    offset = 0

    while True:
        stdscr.erase()
        h, w = stdscr.getmaxyx()
        draw_header(stdscr, " theme ")

        visible = h - 6
        offset  = max(0, min(offset, len(themes) - visible))
        if idx < offset:
            offset = idx
        elif idx >= offset + visible:
            offset = idx - visible + 1

        for i, name in enumerate(themes[offset: offset + visible]):
            ti  = i + offset
            row = 2 + i
            is_cur     = name == current
            is_sel     = ti == idx
            label      = f" {name} {'(active)' if is_cur else ''}"
            if is_sel:
                safe_addstr(stdscr, row, 0, "▶",
                            curses.color_pair(COLOR_SELECT) | curses.A_BOLD)
                safe_addstr(stdscr, row, 2, label, curses.A_BOLD)
            else:
                attr = curses.color_pair(COLOR_DIM) if is_cur else 0
                safe_addstr(stdscr, row, 3, label, attr)

        draw_desc(stdscr, f"enter to apply  current: {current}", flash)
        draw_footer(stdscr, " j/k navigate  enter apply  q back")
        stdscr.refresh()
        flash = ""

        key = stdscr.getch()
        if key in (ord("q"), ord("Q"), 27):
            return
        elif key in (ord("j"), curses.KEY_DOWN):
            idx = min(idx + 1, len(themes) - 1)
        elif key in (ord("k"), curses.KEY_UP):
            idx = max(0, idx - 1)
        elif key in (curses.KEY_ENTER, 10, 13):
            name = themes[idx]
            if set_ghostty_theme(name):
                current = name
                flash   = f"  ✓ Theme set to {name} — restart Ghostty to apply"
            else:
                flash = "  ✗ Could not write to ghostty config"


# ── check for updates view ────────────────────────────────────────────────────

def run_check_updates_view(stdscr) -> None:
    state  = "checking"
    msg    = ""
    behind = 0
    up_ver = ""

    while True:
        stdscr.erase()
        draw_header(stdscr, " check for updates ")

        if state == "checking":
            safe_addstr(stdscr, 2, 2, "Checking for updates…",
                        curses.color_pair(COLOR_DIM))
            draw_footer(stdscr, "")
            stdscr.refresh()
            behind, up_ver = check_upstream()
            if behind == -1:
                state = "error"
                msg   = "Could not check for updates (offline or not a git repo)."
            elif behind == 0:
                state = "uptodate"
            else:
                state = "available"

        elif state == "uptodate":
            safe_addstr(stdscr, 2, 2, "✓  No updates found. You're up to date!",
                        curses.color_pair(COLOR_SELECT) | curses.A_BOLD)
            if VERSION:
                safe_addstr(stdscr, 3, 2, f"  Current version: v{VERSION}")
            draw_footer(stdscr, "  q back ")
            stdscr.refresh()
            if stdscr.getch() not in (curses.KEY_RESIZE,):
                return

        elif state == "available":
            lines = [f"  Update available: v{VERSION} → v{up_ver}" if VERSION
                     else f"  Update available (v{up_ver})"]
            lines += ["", "  Press 'y' to update, any other key to skip."]
            for i, line in enumerate(lines):
                safe_addstr(stdscr, 2 + i, 0, line)
            draw_footer(stdscr, "  y update  q back ")
            stdscr.refresh()
            key = stdscr.getch()
            if key in (ord("q"), ord("Q"), 27):
                return
            elif key in (ord("y"), ord("Y")):
                state = "pulling"

        elif state == "pulling":
            safe_addstr(stdscr, 2, 2, "Applying update…",
                        curses.color_pair(COLOR_DIM))
            stdscr.refresh()
            ok, new_ver = do_pull()
            if ok:
                state = "done"
            else:
                state = "error"
                msg = f"Pull failed: {new_ver}"

        elif state == "done":
            safe_addstr(stdscr, 2, 2,
                        "✓  Dotfiles updated! Restart your shell to apply.",
                        curses.color_pair(COLOR_SELECT) | curses.A_BOLD)
            draw_footer(stdscr, "  q back ")
            stdscr.refresh()
            if stdscr.getch() not in (curses.KEY_RESIZE,):
                return

        elif state == "error":
            safe_addstr(stdscr, 2, 2, f"✗  {msg}",
                        curses.color_pair(COLOR_ERROR) | curses.A_BOLD)
            draw_footer(stdscr, "  q back ")
            stdscr.refresh()
            if stdscr.getch() not in (curses.KEY_RESIZE,):
                return


# ── settings view ─────────────────────────────────────────────────────────────

def run_settings_view(stdscr) -> None:
    s   = load_settings()
    idx = 0

    def _freq_label():
        val = s.get("update_frequency", 1440)
        for mins, label in UPDATE_FREQ_OPTIONS:
            if mins == val:
                return label
        return f"{val} min"

    def _next_freq():
        val = s.get("update_frequency", 1440)
        for i, (mins, _) in enumerate(UPDATE_FREQ_OPTIONS):
            if mins == val:
                return UPDATE_FREQ_OPTIONS[(i + 1) % len(UPDATE_FREQ_OPTIONS)][0]
        return UPDATE_FREQ_OPTIONS[0][0]

    # (label, key, type, options)
    fields = [
        ("Check for updates", CHECK_ACTION,        "action", None),
        ("Auto-updates",      "update_check",       "bool",   None),
        ("Check interval",    "update_frequency",   "choice", _freq_label),
        ("Shell greeting",    "greeting",           "bool",   None),
    ]

    while True:
        stdscr.erase()
        h, w = stdscr.getmaxyx()
        draw_header(stdscr, " settings ")

        for i, (label, key, kind, opt_fn) in enumerate(fields):
            is_sel = i == idx
            cursor = "▶" if is_sel else " "
            if kind == "bool":
                val = s.get(key, True)
                display = "ON" if val else "OFF"
                val_attr = curses.color_pair(COLOR_SELECT) if val else curses.color_pair(COLOR_ERROR)
            elif kind == "choice":
                display = opt_fn() if opt_fn else ""
                val_attr = curses.color_pair(COLOR_HEADER)
            else:
                display = ""
                val_attr = 0

            safe_addstr(stdscr, 2 + i, 0, cursor,
                        curses.color_pair(COLOR_SELECT) | curses.A_BOLD if is_sel else 0)
            safe_addstr(stdscr, 2 + i, 2, f"{label:<20}",
                        curses.A_BOLD if is_sel else 0)
            safe_addstr(stdscr, 2 + i, 22, display, val_attr)

        descs = {
            CHECK_ACTION:       "manually check for dotfiles updates right now",
            "update_check":     "automatically check for updates in the background",
            "update_frequency": "how often to check for updates",
            "greeting":         "show fastfetch system info when opening a terminal",
        }
        draw_desc(stdscr, descs.get(fields[idx][1], ""))
        draw_footer(stdscr, " j/k navigate  space/enter activate  q save & back")
        stdscr.refresh()

        key = stdscr.getch()
        if key in (ord("q"), ord("Q"), 27):
            save_settings(s)
            return
        elif key in (ord("j"), curses.KEY_DOWN):
            idx = (idx + 1) % len(fields)
        elif key in (ord("k"), curses.KEY_UP):
            idx = (idx - 1) % len(fields)
        elif key in (ord(" "), curses.KEY_ENTER, 10, 13):
            _, fkey, fkind, _ = fields[idx]
            if fkind == "bool":
                s[fkey] = not s.get(fkey, True)
            elif fkind == "choice":
                s[fkey] = _next_freq()
            elif fkind == "action" and fkey == CHECK_ACTION:
                run_check_updates_view(stdscr)


# ── dev packages view ─────────────────────────────────────────────────────────

def run_personal_view(stdscr):
    flash  = ""
    idx    = 0
    offset = 0
    pkgs   = [d for d in DEPS if d.category == "dev"]

    while True:
        stdscr.erase()
        h, w = stdscr.getmaxyx()
        draw_header(stdscr, " dev packages ")

        missing = []
        visible = h - 6
        offset  = max(0, min(offset, len(pkgs) - visible))
        if idx < offset:
            offset = idx
        elif idx >= offset + visible:
            offset = idx - visible + 1

        for i, dep in enumerate(pkgs[offset: offset + visible]):
            ti     = i + offset
            ok     = check_dep(dep)
            cursor = "▶" if ti == idx else " "
            sym    = "✓" if ok else "✗"
            sc     = COLOR_SELECT if ok else COLOR_ERROR
            label  = dep.brew or dep.bin
            suffix = "(cask)" if dep.cask else ""
            safe_addstr(stdscr, 2 + i, 0, cursor,
                        curses.color_pair(sc) | curses.A_BOLD)
            safe_addstr(stdscr, 2 + i, 2, sym, curses.color_pair(sc) | curses.A_BOLD)
            safe_addstr(stdscr, 2 + i, 4, f"{label:<20}", curses.A_BOLD if not ok else 0)
            safe_addstr(stdscr, 2 + i, 24, f"{dep.desc:<30}", curses.color_pair(COLOR_DIM))
            if suffix:
                safe_addstr(stdscr, 2 + i, 54, suffix, curses.color_pair(COLOR_DIM))

        missing = [d for d in pkgs if not check_dep(d)]
        draw_desc(stdscr, pkgs[idx].desc if pkgs else "", flash)
        hint = f" j/k navigate  i install {len(missing)} missing  q back" if missing \
               else " j/k navigate  all installed  q back"
        draw_footer(stdscr, hint)
        stdscr.refresh()
        flash = ""

        key = stdscr.getch()
        if key in (ord("q"), ord("Q"), 27):
            return None
        elif key in (ord("j"), curses.KEY_DOWN):
            idx = min(idx + 1, len(pkgs) - 1)
        elif key in (ord("k"), curses.KEY_UP):
            idx = max(0, idx - 1)
        elif key == ord("i") and missing:
            return ("install", missing)


# ── edit configs view (dev) ───────────────────────────────────────────────────

def run_edit_configs_view(stdscr):
    idx = 0
    while True:
        stdscr.erase()
        h, w = stdscr.getmaxyx()
        draw_header(stdscr, " edit configs ")
        label_w = max(len(l) for l, *_ in EDIT_CONFIGS) + 2
        for i, (label, _, desc) in enumerate(EDIT_CONFIGS):
            cursor = "▶" if i == idx else " "
            if i == idx:
                safe_addstr(stdscr, 2 + i, 0, cursor,
                            curses.color_pair(COLOR_SELECT) | curses.A_BOLD)
                safe_addstr(stdscr, 2 + i, 2, f" {label:<{label_w}}", curses.A_BOLD)
            else:
                safe_addstr(stdscr, 2 + i, 0, f"   {label:<{label_w}}")
        draw_desc(stdscr, EDIT_CONFIGS[idx][2])
        draw_footer(stdscr, " j/k navigate  enter open  q back")
        stdscr.refresh()

        key = stdscr.getch()
        if key in (ord("q"), ord("Q"), 27):
            return None
        elif key in (ord("j"), curses.KEY_DOWN):
            idx = (idx + 1) % len(EDIT_CONFIGS)
        elif key in (ord("k"), curses.KEY_UP):
            idx = (idx - 1) % len(EDIT_CONFIGS)
        elif key in (curses.KEY_ENTER, 10, 13):
            return ("open", str(EDIT_CONFIGS[idx][1]))


# ── add alias view (dev) ──────────────────────────────────────────────────────

def _read_field(stdscr, prompt: str, row: int) -> str:
    h, w = stdscr.getmaxyx()
    safe_addstr(stdscr, row, 2, prompt, curses.color_pair(COLOR_DIM))
    safe_addstr(stdscr, row, 2 + len(prompt), " " * (w - len(prompt) - 4))
    stdscr.refresh()
    curses.echo()
    curses.curs_set(1)
    val = stdscr.getstr(row, 2 + len(prompt), w - len(prompt) - 6)
    curses.noecho()
    curses.curs_set(0)
    return val.decode("utf-8", errors="replace").strip()


def run_add_alias_view(stdscr) -> None:
    flash = ""
    while True:
        stdscr.erase()
        h, w = stdscr.getmaxyx()
        draw_header(stdscr, " add alias ")
        safe_addstr(stdscr, 2, 2, "Add a new alias to ~/.config/zsh/.aliases")
        safe_addstr(stdscr, 4, 2, "Press enter after each field. Leave name blank to cancel.")

        if flash:
            draw_desc(stdscr, "", flash)
        draw_footer(stdscr, " fill in fields below ")
        stdscr.refresh()

        name = _read_field(stdscr, "alias name:    ", 6)
        if not name:
            return
        command = _read_field(stdscr, "command:       ", 7)
        if not command:
            return
        desc    = _read_field(stdscr, "description:   ", 8)

        aliases_path = DOTS_DIR / "src/zsh/zsh/.aliases"
        try:
            line = f'alias {name}="{command}"'
            if desc:
                line += f"  # {desc}"
            with aliases_path.open("a") as f:
                f.write(f"\n{line}\n")
            flash = f"  ✓ Added: {line}"
            show_message(stdscr, " add alias ", [
                f"  Added: alias {name}=\"{command}\"",
                "",
                "  Run `reload` in your shell to apply the new alias.",
            ])
            return
        except Exception as e:
            flash = f"  ✗ Failed: {e}"


# ── git view (dev) ────────────────────────────────────────────────────────────

def run_git_view(stdscr):
    items = [
        ("Status",  "status", "show working tree status"),
        ("Log",     "log",    "recent commits on the dotfiles repo"),
        ("Pull",    "pull",   "pull latest changes (fast-forward only)"),
        ("Push",    "push",   "push local commits to remote"),
    ]
    idx = 0
    while True:
        stdscr.erase()
        h, w = stdscr.getmaxyx()
        draw_header(stdscr, " git ")
        label_w = max(len(l) for l, *_ in items) + 2
        for i, (label, _, desc) in enumerate(items):
            if i == idx:
                safe_addstr(stdscr, 2 + i, 0, "▶",
                            curses.color_pair(COLOR_SELECT) | curses.A_BOLD)
                safe_addstr(stdscr, 2 + i, 2, f" {label:<{label_w}}", curses.A_BOLD)
            else:
                safe_addstr(stdscr, 2 + i, 0, f"   {label:<{label_w}}")
        draw_desc(stdscr, items[idx][2])
        draw_footer(stdscr, " j/k navigate  enter select  q back")
        stdscr.refresh()

        key = stdscr.getch()
        if key in (ord("q"), ord("Q"), 27):
            return None
        elif key in (ord("j"), curses.KEY_DOWN):
            idx = (idx + 1) % len(items)
        elif key in (ord("k"), curses.KEY_UP):
            idx = (idx - 1) % len(items)
        elif key in (curses.KEY_ENTER, 10, 13):
            _, op, _ = items[idx]
            if op == "status":
                r = subprocess.run(["git", "-C", str(DOTS_DIR), "status"],
                                   capture_output=True, text=True)
                show_output(stdscr, " git status ", r.stdout or r.stderr)
            elif op == "log":
                r = subprocess.run(
                    ["git", "-C", str(DOTS_DIR), "log", "--oneline", "-20"],
                    capture_output=True, text=True)
                show_output(stdscr, " git log ", r.stdout or r.stderr)
            elif op in ("pull", "push"):
                return ("git", op)  # exit curses, run in terminal


# ── logs view (dev) ───────────────────────────────────────────────────────────

def run_logs_view(stdscr) -> None:
    lines = ["  Update checker state:", ""]
    stamp = Path.home() / ".config/zsh/.update_stamp"
    vstamp = Path.home() / ".config/zsh/.version_stamp"
    if stamp.exists():
        import time
        ts = int(stamp.read_text().strip() or 0)
        lines.append(f"  Last checked:  {time.strftime('%Y-%m-%d %H:%M:%S', time.localtime(ts))}")
    else:
        lines.append("  Last checked:  never")
    if vstamp.exists():
        lines.append(f"  Known version: {vstamp.read_text().strip()}")
    lines += ["", f"  Dotfiles dir:  {DOTS_DIR}", f"  Settings file: {SETTINGS_FILE}"]
    s = load_settings()
    lines += ["", "  Settings:", *[f"    {k}: {v}" for k, v in s.items()]]
    show_output(stdscr, " logs ", "\n".join(lines))


# ── aliases view ─────────────────────────────────────────────────────────────

def parse_aliases() -> list[tuple]:
    """Returns list of ("section", name) or ("alias", name, desc) tuples."""
    aliases_path = DOTS_DIR / "src/zsh/zsh/.aliases"
    result: list[tuple] = []
    try:
        for line in aliases_path.read_text().splitlines():
            m = re.match(r'^#\s*---\s*alias:(\S+?)\s*---', line)
            if m:
                result.append(("section", m.group(1)))
                continue
            m = re.match(r'^alias\s+([a-zA-Z0-9_-]+)=.+#\s*(.+)', line)
            if m:
                result.append(("alias", m.group(1), m.group(2).strip()))
    except Exception:
        pass
    return result


def run_aliases_view(stdscr) -> None:
    display  = parse_aliases()
    nav_items = [(i, d[1], d[2]) for i, d in enumerate(display) if d[0] == "alias"]

    if not nav_items:
        show_message(stdscr, " aliases ", ["  No aliases found or .aliases file missing."])
        return

    idx    = 0
    offset = 0

    while True:
        stdscr.erase()
        h, w = stdscr.getmaxyx()
        draw_header(stdscr, " aliases ")

        visible  = h - 6
        sel_di   = nav_items[idx][0]
        if sel_di < offset:
            offset = sel_di
        elif sel_di >= offset + visible:
            offset = sel_di - visible + 1
        offset = max(0, min(offset, max(0, len(display) - visible)))

        nav_i = -1
        for di, item in enumerate(display):
            row = 2 + di - offset
            if item[0] == "alias":
                nav_i += 1
            if row < 2 or row >= 2 + visible:
                continue
            if item[0] == "section":
                safe_addstr(stdscr, row, 2, item[1],
                            curses.color_pair(COLOR_DIM) | curses.A_BOLD)
            elif item[0] == "alias":
                _, name, desc = item
                is_sel = nav_i == idx
                safe_addstr(stdscr, row, 0, "▶" if is_sel else " ",
                            curses.color_pair(COLOR_SELECT) | curses.A_BOLD if is_sel else 0)
                safe_addstr(stdscr, row, 2, f"{name:<14}",
                            curses.color_pair(COLOR_HEADER) | curses.A_BOLD if is_sel else curses.color_pair(COLOR_HEADER))
                safe_addstr(stdscr, row, 16, desc,
                            curses.A_BOLD if is_sel else 0)

        draw_desc(stdscr, nav_items[idx][2])
        draw_footer(stdscr, " j/k navigate  q back ")
        stdscr.refresh()

        key = stdscr.getch()
        if key in (ord("q"), ord("Q"), 27):
            return
        elif key in (ord("j"), curses.KEY_DOWN):
            idx = min(idx + 1, len(nav_items) - 1)
        elif key in (ord("k"), curses.KEY_UP):
            idx = max(0, idx - 1)


# ── main TUI ──────────────────────────────────────────────────────────────────

def run_tui(stdscr):
    init_colors()
    curses.curs_set(0)
    stdscr.keypad(True)

    h, w = stdscr.getmaxyx()
    if h < MIN_H or w < MIN_W:
        safe_addstr(stdscr, 0, 0,
                    f"Terminal too small — need at least {MIN_W}×{MIN_H}")
        stdscr.getch()
        return None

    idx = 0

    while True:
        dev   = is_dev_mode()
        menu  = build_menu(dev)

        # Skip separator items during navigation
        nav_indices = [i for i, (_, path, _) in enumerate(menu) if path != SEPARATOR]
        if idx not in nav_indices:
            idx = nav_indices[0] if nav_indices else 0

        stdscr.erase()
        h, w = stdscr.getmaxyx()
        draw_header(stdscr, " dots ")

        label_w = max(len(label) for label, *_ in menu) + 2
        for i, (label, path, _desc) in enumerate(menu):
            y = 2 + i
            if path == SEPARATOR:
                safe_addstr(stdscr, y, 2, "─" * min(20, w - 4),
                            curses.color_pair(COLOR_DIM))
                continue
            if i == idx:
                safe_addstr(stdscr, y, 0, "▶",
                            curses.color_pair(COLOR_SELECT) | curses.A_BOLD)
                safe_addstr(stdscr, y, 2, f" {label:<{label_w}}", curses.A_BOLD)
                if dev and label == "Developer":
                    safe_addstr(stdscr, y, 2 + label_w + 2, "[dev mode ON]",
                                curses.color_pair(COLOR_SELECT))
            else:
                safe_addstr(stdscr, y, 0, f"   {label:<{label_w}}")
                if dev and label == "Developer":
                    safe_addstr(stdscr, y, 3 + label_w + 2, "[dev mode ON]",
                                curses.color_pair(COLOR_DIM))

        draw_desc(stdscr, menu[idx][2])
        draw_footer(stdscr, " j/k navigate  enter select  q quit")
        stdscr.refresh()

        key = stdscr.getch()
        if key in (ord("q"), ord("Q"), 27):
            return None

        pos_in_nav = nav_indices.index(idx) if idx in nav_indices else 0
        if key in (ord("j"), curses.KEY_DOWN):
            idx = nav_indices[(pos_in_nav + 1) % len(nav_indices)]
        elif key in (ord("k"), curses.KEY_UP):
            idx = nav_indices[(pos_in_nav - 1) % len(nav_indices)]
        elif key in (curses.KEY_ENTER, 10, 13):
            _, path, _ = menu[idx]

            if path == HEALTH_ACTION:
                result = run_health_view(stdscr)
                if result is not None:
                    return result
            elif path == THEME_ACTION:
                run_theme_view(stdscr)
            elif path == ALIASES_ACTION:
                run_aliases_view(stdscr)
            elif path == SETTINGS_ACTION:
                run_settings_view(stdscr)
            elif path == DEV_ACTION:
                set_dev_mode(not dev)
                idx = 0
            elif path == EDIT_ACTION:
                result = run_edit_configs_view(stdscr)
                if result is not None:
                    return result
            elif path == ALIAS_ACTION:
                run_add_alias_view(stdscr)
            elif path == PERSONAL_ACTION:
                result = run_personal_view(stdscr)
                if result is not None:
                    return result
            elif path == GIT_ACTION:
                result = run_git_view(stdscr)
                if result is not None:
                    return result
            elif path == RELOAD_ACTION:
                show_message(stdscr, " reload ", [
                    "  Run the following command in your shell to apply changes:",
                    "",
                    "    reload",
                    "",
                    "  (or open a new terminal window)",
                ])
            elif path == LOGS_ACTION:
                run_logs_view(stdscr)
            elif path == RESET_ACTION:
                show_message(stdscr, " reset ", [
                    "  This will delete your dots settings file.",
                    "  Your theme and update preferences will be cleared.",
                    "",
                    "  Press 'y' to confirm, any other key to cancel.",
                ])
                confirm = stdscr.getch()
                if confirm in (ord("y"), ord("Y")):
                    try:
                        SETTINGS_FILE.unlink(missing_ok=True)
                    except Exception:
                        pass
                    show_message(stdscr, " reset ", ["  Settings reset. Run `reload` to apply."])


# ── git operations (outside curses) ──────────────────────────────────────────

def run_git_op(op: str) -> None:
    cmd = ["git", "-C", str(DOTS_DIR)]
    if op == "pull":
        cmd += ["pull", "--ff-only"]
    elif op == "push":
        cmd += ["push"]
    subprocess.run(cmd)


# ── entry point ───────────────────────────────────────────────────────────────

def main() -> None:
    while True:
        try:
            result = curses.wrapper(run_tui)
        except KeyboardInterrupt:
            break

        if result is None:
            break

        action, data = result

        if action == "open":
            open_file(data)
            break  # exec — never returns here
        elif action == "install":
            print()
            install_deps_cli(data)
            print("\n  Press Enter to return to dots...")
            try:
                input()
            except EOFError:
                break
        elif action == "git":
            print()
            run_git_op(data)
            print("\n  Press Enter to return to dots...")
            try:
                input()
            except EOFError:
                break


if __name__ == "__main__":
    if "--repair-symlinks" in sys.argv:
        repair_all()
    elif "--check-deps" in sys.argv:
        check_deps_cli()
    elif "--install-deps" in sys.argv:
        missing = [d for d in DEPS if d.category == "required" and not check_dep(d)]
        install_deps_cli(missing)
    elif "--install-optional" in sys.argv:
        missing = [d for d in DEPS if d.category == "optional" and not check_dep(d)]
        install_deps_cli(missing)
    elif "--install-dev" in sys.argv:
        missing = [d for d in DEPS if d.category == "dev" and not check_dep(d)]
        install_deps_cli(missing)
    elif "--health" in sys.argv:
        check_deps_cli()
    elif "--dev-mode" in sys.argv:
        val = sys.argv[sys.argv.index("--dev-mode") + 1] if \
              sys.argv.index("--dev-mode") + 1 < len(sys.argv) else ""
        set_dev_mode(val.lower() in ("on", "1", "true"))
        print(f"  Developer mode: {'ON' if is_dev_mode() else 'OFF'}")
    elif "--set" in sys.argv:
        i = sys.argv.index("--set")
        if i + 1 < len(sys.argv) and "=" in sys.argv[i + 1]:
            k, v = sys.argv[i + 1].split("=", 1)
            s = load_settings()
            s[k] = v
            save_settings(s)
            print(f"  Set {k} = {v}")
    elif "--theme" in sys.argv:
        i = sys.argv.index("--theme")
        if i + 1 < len(sys.argv):
            name = sys.argv[i + 1]
            if set_ghostty_theme(name):
                print(f"  Theme set to: {name}")
            else:
                print("  Failed to update ghostty config", file=sys.stderr)
                sys.exit(1)
    else:
        main()
