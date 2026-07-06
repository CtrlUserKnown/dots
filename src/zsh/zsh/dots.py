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

from shared import (
    DOTS_DIR, COLOR_HEADER, COLOR_SELECT, COLOR_ERROR, COLOR_DIM,
    init_colors, safe_addstr, draw_header as _draw_header, draw_footer, draw_desc,
    clamp, check_upstream,
)

SETTINGS_FILE = DOTS_DIR / ".settings"
VERSION       = os.environ.get("DOTS_VERSION", "")


def draw_header(win, title: str) -> None:
    _draw_header(win, title, VERSION)


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
    Dep("nvim",      "neovim",    "neovim",    "neovim",    "text editor ($EDITOR)",               "optional"),
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

def get_symlinks() -> list[tuple]:
    """Core symlinks always required, plus tool-conditional ones."""
    links = [
        (Path.home() / ".config/bat",       DOTS_DIR / "src/bat"),
        (Path.home() / ".config/fastfetch", DOTS_DIR / "src/fastfetch"),
        (Path.home() / ".config/zsh",       DOTS_DIR / "src/zsh/zsh"),
        (Path.home() / ".zshrc",            DOTS_DIR / "src/zsh/.zshrc"),
    ]
    if shutil.which("ghostty"):
        links.append((Path.home() / ".config/ghostty", DOTS_DIR / "src/ghostty"))
    if shutil.which("herdr"):
        links.append((Path.home() / ".config/herdr", DOTS_DIR / "src/herdr"))
    return links

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
SSM_ACTION      = "__ssm__"
PROFILE_ACTION  = "__profile__"
SEPARATOR       = "__sep__"

SSM_CONFIG_FILE = Path.home() / ".config" / "ssm" / "config.json"


def load_ssm_config() -> dict:
    try:
        with open(SSM_CONFIG_FILE) as f:
            return json.load(f)
    except Exception:
        return {"use_herdr": True}


def save_ssm_config(cfg: dict) -> None:
    SSM_CONFIG_FILE.parent.mkdir(parents=True, exist_ok=True)
    with open(SSM_CONFIG_FILE, "w") as f:
        json.dump(cfg, f, indent=2)

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
        ("SSM",          SSM_ACTION,      "SSH session manager — install & configure herdr"),
        ("Profile",      PROFILE_ACTION,  "generate or import a personal config file"),
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
        ("SSM",      SSM_ACTION,      "SSH session manager — install & configure herdr"),
        ("Profile",  PROFILE_ACTION,  "generate or import a personal config file"),
        ("Settings", SETTINGS_ACTION, "configure dots preferences"),
    ] + dev_items


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
    for link, target in get_symlinks():
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
    flash: tuple | None = None
    idx   = 0  # navigable item index

    while True:
        stdscr.erase()
        h, w = stdscr.getmaxyx()
        draw_header(stdscr, " health ")

        # Build flat display list: ("section"|"item", label, ok, desc)
        display: list[tuple] = []
        nav_descs: list[str] = []
        nav_types: list[tuple] = []  # ("symlink", link, target) | ("dep", dep) | ("plugin", paths)

        display.append(("section", "symlinks", True, ""))
        for link, target in get_symlinks():
            ok     = check_symlink(link, target) == "OK"
            label  = "~/" + str(link.relative_to(Path.home()))
            try:
                tstr = "~/" + str(target.relative_to(Path.home()))
            except ValueError:
                tstr = str(target)
            display.append(("item", label, ok, f"→ {tstr}"))
            nav_descs.append(f"→ {tstr}")
            nav_types.append(("symlink", link, target))

        display.append(("section", "tools", True, ""))
        for dep in DEPS:
            if dep.category not in ("required", "optional"):
                continue
            ok  = check_dep(dep)
            tag = f"[{dep.category[:3]}]"
            display.append(("item", f"{dep.bin or dep.brew:<14} {tag}", ok, dep.desc))
            nav_descs.append(dep.desc)
            nav_types.append(("dep", dep))

        display.append(("section", "plugins", True, ""))
        for name, desc, paths in PLUGINS:
            ok = check_plugin(paths)
            display.append(("item", name, ok, desc))
            nav_descs.append(desc)
            nav_types.append(("plugin", paths))

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

        any_broken = any(check_symlink(l, t) != "OK" for l, t in get_symlinks())
        any_missing_tools = any(
            not check_dep(d) for d in DEPS if d.category in ("required", "optional")
        )

        # determine whether the selected item can be fixed with enter
        curr_type = nav_types[idx] if idx < len(nav_types) else None
        can_enter_fix = False
        if curr_type:
            if curr_type[0] == "symlink":
                can_enter_fix = check_symlink(curr_type[1], curr_type[2]) != "OK"
            elif curr_type[0] == "dep":
                can_enter_fix = not check_dep(curr_type[1])

        hints = []
        if can_enter_fix:
            hints.append("enter fix")
        if any_broken:
            hints.append("r repair all")
        if any_missing_tools:
            hints.append("i install all")
        hints.append("q back")
        draw_footer(stdscr, " j/k navigate  " + "  ".join(hints))
        stdscr.refresh()
        flash = None

        key = stdscr.getch()
        if key in (ord("q"), ord("Q"), 27):
            return
        elif key in (ord("j"), curses.KEY_DOWN):
            idx = min(idx + 1, nav_total - 1)
        elif key in (ord("k"), curses.KEY_UP):
            idx = max(0, idx - 1)
        elif key in (curses.KEY_ENTER, 10, 13) and curr_type:
            if curr_type[0] == "symlink":
                link, target = curr_type[1], curr_type[2]
                if check_symlink(link, target) != "OK":
                    ok = repair_symlink(link, target)
                    flash = ("  ✓ Repaired" if ok else "  ✗ Repair failed", COLOR_SELECT if ok else COLOR_ERROR)
                else:
                    flash = ("  already OK", COLOR_DIM)
            elif curr_type[0] == "dep":
                dep = curr_type[1]
                if not check_dep(dep):
                    return ("install", [dep])
                else:
                    flash = ("  already installed", COLOR_DIM)
            elif curr_type[0] == "plugin":
                flash = ("  install plugin manually — see docs", COLOR_DIM)
        elif key == ord("r") and any_broken:
            rep = skp = 0
            for link, target in get_symlinks():
                if check_symlink(link, target) != "OK":
                    if repair_symlink(link, target): rep += 1
                    else: skp += 1
            parts = []
            if rep: parts.append(f"{rep} repaired")
            if skp: parts.append(f"{skp} skipped")
            flash = ("  ".join(parts), COLOR_SELECT)
        elif key == ord("i") and any_missing_tools:
            missing = [d for d in DEPS
                       if d.category in ("required", "optional") and not check_dep(d)]
            return ("install", missing)  # handled by main loop


# ── theme view ────────────────────────────────────────────────────────────────

def run_theme_view(stdscr) -> None:
    flash: tuple | None = None
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
        flash = None

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
                flash   = (f"  ✓ Theme set to {name} — restart Ghostty to apply", COLOR_SELECT)
            else:
                flash = ("  ✗ Could not write to ghostty config", COLOR_ERROR)


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
            dev = is_dev_mode()
            behind, up_ver = check_upstream(DOTS_DIR, dev_mode=dev)
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
            if dev and VERSION:
                safe_addstr(stdscr, 3, 2, f"  Current commit: {VERSION} (dev mode)")
            elif VERSION:
                safe_addstr(stdscr, 3, 2, f"  Current version: v{VERSION}")
            draw_footer(stdscr, "  q back ")
            stdscr.refresh()
            if stdscr.getch() not in (curses.KEY_RESIZE,):
                return

        elif state == "available":
            if dev:
                lines = [f"  {behind} new commit(s) on origin (latest: {up_ver})"]
            else:
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
    flash: tuple | None = None
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
        flash = None

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
    flash: tuple | None = None
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
            flash = (f"  ✓ Added: {line}", COLOR_SELECT)
            show_message(stdscr, " add alias ", [
                f"  Added: alias {name}=\"{command}\"",
                "",
                "  Run `reload` in your shell to apply the new alias.",
            ])
            return
        except Exception as e:
            flash = (f"  ✗ Failed: {e}", COLOR_ERROR)


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


# ── personal config ───────────────────────────────────────────────────────────

PERSONAL_CONFIG_FILE    = Path.home() / ".config" / "dots" / "personal.json"
_PERSONAL_CONFIG_VERSION = "1"


def _collect_personal_config() -> dict:
    s     = load_settings()
    theme = get_current_theme()
    opt   = [d.brew or d.bin for d in DEPS if d.category == "optional" and check_dep(d)]
    dev   = [d.brew or d.bin for d in DEPS if d.category == "dev"      and check_dep(d)]
    return {
        "version":      _PERSONAL_CONFIG_VERSION,
        "generated":    datetime.datetime.now().isoformat(timespec="seconds"),
        "dots_version": VERSION or "unknown",
        "settings":     {k: v for k, v in s.items() if k in _DEFAULT_SETTINGS},
        "theme":        theme,
        "packages":     {"optional": opt, "dev": dev},
    }


def generate_personal_config(dest: Optional[Path] = None) -> Path:
    dest = dest or PERSONAL_CONFIG_FILE
    dest.parent.mkdir(parents=True, exist_ok=True)
    dest.write_text(json.dumps(_collect_personal_config(), indent=2))
    return dest


def _validate_personal_config(data: dict) -> Optional[str]:
    if not isinstance(data, dict):
        return "not a JSON object"
    if data.get("version") != _PERSONAL_CONFIG_VERSION:
        return f"unsupported version '{data.get('version')}' (expected '{_PERSONAL_CONFIG_VERSION}')"
    pkgs = data.get("packages", {})
    if not isinstance(pkgs, dict):
        return "'packages' must be an object"
    for cat in ("optional", "dev"):
        if not isinstance(pkgs.get(cat, []), list):
            return f"'packages.{cat}' must be a list"
    return None


def apply_personal_config(data: dict) -> list:
    current  = load_settings()
    incoming = data.get("settings", {})
    for k in _DEFAULT_SETTINGS:
        if k in incoming:
            current[k] = incoming[k]
    save_settings(current)

    theme = data.get("theme", "")
    if theme:
        set_ghostty_theme(theme)

    pkg_map: dict[str, Dep] = {(d.brew or d.bin): d for d in DEPS}
    pkgs = data.get("packages", {})
    return [
        pkg_map[name] for name in pkgs.get("optional", []) + pkgs.get("dev", [])
        if name in pkg_map and not check_dep(pkg_map[name])
    ]


def _fetch_github_raw(spec: str) -> str:
    import urllib.request
    parts = spec.split("/", 2)
    if len(parts) < 3:
        raise ValueError(f"expected user/repo/path, got '{spec}'")
    user, repo, path = parts
    for branch in ("main", "master"):
        url = f"https://raw.githubusercontent.com/{user}/{repo}/{branch}/{path}"
        try:
            with urllib.request.urlopen(url, timeout=10) as r:  # noqa: S310
                return r.read().decode()
        except Exception:
            continue
    raise ValueError(f"could not fetch '{spec}' from GitHub (tried main and master)")


# ── aliases view ─────────────────────────────────────────────────────────────

def parse_aliases() -> list[tuple]:
    """Returns list of ("section", name) or ("alias", name, desc, value) tuples."""
    aliases_path = DOTS_DIR / "src/zsh/zsh/.aliases"
    result: list[tuple] = []
    try:
        for line in aliases_path.read_text().splitlines():
            m = re.match(r'^#\s*---\s*alias:(\S+?)\s*---', line)
            if m:
                result.append(("section", m.group(1)))
                continue
            m = re.match(r'^alias\s+([a-zA-Z0-9_-]+)=(.+?)(?:\s{2,}#\s*(.+))?$', line)
            if m and (m.group(2) or m.group(3)):
                name  = m.group(1)
                val   = (m.group(2) or "").strip().strip('"').strip("'")
                desc  = (m.group(3) or "").strip()
                result.append(("alias", name, desc, val))
    except Exception:
        pass
    return result


def _yank_text(text: str) -> bool:
    try:
        subprocess.run(["pbcopy"], input=text.encode(), check=True, timeout=3)
        return True
    except Exception:
        return False


def run_aliases_view(stdscr) -> None:
    display   = parse_aliases()
    # nav_items: (display_index, name, desc, value)
    nav_items = [(i, d[1], d[2], d[3] if len(d) > 3 else "") for i, d in enumerate(display) if d[0] == "alias"]

    if not nav_items:
        show_message(stdscr, " aliases ", ["  No aliases found or .aliases file missing."])
        return

    idx    = 0
    offset = 0
    flash: tuple | None = None

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
                name, desc = item[1], item[2]
                is_sel = nav_i == idx
                safe_addstr(stdscr, row, 0, "▶" if is_sel else " ",
                            curses.color_pair(COLOR_SELECT) | curses.A_BOLD if is_sel else 0)
                safe_addstr(stdscr, row, 2, f"{name:<14}",
                            curses.color_pair(COLOR_HEADER) | curses.A_BOLD if is_sel else curses.color_pair(COLOR_HEADER))
                safe_addstr(stdscr, row, 16, desc,
                            curses.A_BOLD if is_sel else 0)

        val = nav_items[idx][3]
        draw_desc(stdscr, nav_items[idx][2], flash)
        flash = None
        draw_footer(stdscr, " j/k navigate  enter copy  q back ")
        stdscr.refresh()

        key = stdscr.getch()
        if key in (ord("q"), ord("Q"), 27):
            return
        elif key in (ord("j"), curses.KEY_DOWN):
            idx = min(idx + 1, len(nav_items) - 1)
        elif key in (ord("k"), curses.KEY_UP):
            idx = max(0, idx - 1)
        elif key in (curses.KEY_ENTER, 10, 13):
            val = nav_items[idx][3]
            if val and _yank_text(val):
                flash = (f"  ✓ Copied: {val[:40]}", COLOR_SELECT)
            else:
                flash = ("  ✗ pbcopy not available", COLOR_ERROR)


# ── ssm view ─────────────────────────────────────────────────────────────────

def run_ssm_view(stdscr):
    herdr_dep = next(d for d in DEPS if d.bin == "herdr")
    flash: tuple | None = None

    while True:
        installed = check_dep(herdr_dep)
        cfg       = load_ssm_config()
        use_herdr = cfg.get("use_herdr", True)

        stdscr.erase()
        h, w = stdscr.getmaxyx()
        draw_header(stdscr, " ssm ")

        # herdr row
        sym  = "✓" if installed else "✗"
        sc   = COLOR_SELECT if installed else COLOR_ERROR
        safe_addstr(stdscr, 2, 2, sym,     curses.color_pair(sc) | curses.A_BOLD)
        safe_addstr(stdscr, 2, 4, "herdr", curses.A_BOLD)
        safe_addstr(stdscr, 2, 12,
                    "installed" if installed else "not installed",
                    curses.color_pair(sc))

        # herdr mode row (only meaningful when installed)
        if installed:
            mode_str  = "ON" if use_herdr else "OFF"
            mode_attr = curses.color_pair(COLOR_SELECT if use_herdr else COLOR_ERROR)
            safe_addstr(stdscr, 3, 4, "mode",    curses.A_BOLD)
            safe_addstr(stdscr, 3, 12, mode_str, mode_attr | curses.A_BOLD)

        # info lines
        safe_addstr(stdscr, 5, 4,
                    "herdr --remote connects sessions through herdr's SSH transport.",
                    curses.color_pair(COLOR_DIM))
        safe_addstr(stdscr, 6, 4,
                    "Key-based SSH auth is required (password sessions not supported).",
                    curses.color_pair(COLOR_DIM))
        safe_addstr(stdscr, 7, 4,
                    "Run  ssh-copy-id user@host  to authorise your key on a server.",
                    curses.color_pair(COLOR_DIM))

        draw_desc(stdscr, "", flash)
        flash = None

        hints = []
        if not installed:
            hints.append("i install herdr")
        else:
            hints.append("h toggle mode")
            hints.append("s open ssm")
        hints.append("q back")
        draw_footer(stdscr, "  " + "  ".join(hints))
        stdscr.refresh()

        key = stdscr.getch()
        if key in (ord("q"), ord("Q"), 27):
            return None
        elif key == ord("i") and not installed:
            return ("install", [herdr_dep])
        elif key == ord("h") and installed:
            cfg["use_herdr"] = not use_herdr
            save_ssm_config(cfg)
            flash = (f"  herdr mode {'ON' if not use_herdr else 'OFF'}", COLOR_SELECT)
        elif key == ord("s") and installed:
            return ("launch_ssm", None)


# ── profile view ─────────────────────────────────────────────────────────────

def _prompt_line(stdscr, row: int, prompt: str, initial: str = "") -> Optional[str]:
    """Inline single-line text input. Returns stripped string or None on esc."""
    curses.curs_set(1)
    value = initial
    _, w = stdscr.getmaxyx()
    while True:
        safe_addstr(stdscr, row, 0, " " * max(0, w - 1))
        line = f"  {prompt}: {value}"
        safe_addstr(stdscr, row, 0, line, curses.A_BOLD)
        try:
            stdscr.move(row, min(len(line), w - 1))
        except curses.error:
            pass
        stdscr.refresh()
        key = stdscr.getch()
        if key == 27:
            curses.curs_set(0)
            return None
        elif key in (curses.KEY_ENTER, 10, 13):
            curses.curs_set(0)
            return value.strip()
        elif key in (curses.KEY_BACKSPACE, 127, 8):
            value = value[:-1]
        elif 32 <= key <= 126:
            value += chr(key)


def run_profile_view(stdscr):
    flash: tuple | None = None

    while True:
        exists = PERSONAL_CONFIG_FILE.exists()
        stdscr.erase()
        h, w = stdscr.getmaxyx()
        draw_header(stdscr, " profile ")

        sym  = "✓" if exists else "✗"
        sc   = COLOR_SELECT if exists else COLOR_DIM
        safe_addstr(stdscr, 2, 2, sym, curses.color_pair(sc) | curses.A_BOLD)
        cfg_label = str(PERSONAL_CONFIG_FILE).replace(str(Path.home()), "~")
        safe_addstr(stdscr, 2, 4, cfg_label, curses.color_pair(COLOR_DIM))

        if exists:
            try:
                data = json.loads(PERSONAL_CONFIG_FILE.read_text())
                gen  = data.get("generated", "")[:19]
                dver = data.get("dots_version", "?")
                pkgs = data.get("packages", {})
                n    = len(pkgs.get("optional", [])) + len(pkgs.get("dev", []))
                safe_addstr(stdscr, 3, 6,
                            f"generated {gen}  ·  {n} packages  ·  dots {dver}",
                            curses.color_pair(COLOR_DIM))
            except Exception:
                safe_addstr(stdscr, 3, 6, "(unreadable)", curses.color_pair(COLOR_ERROR))

        safe_addstr(stdscr, 5, 2,
                    "g  generate / update config from current system",
                    curses.color_pair(COLOR_DIM))
        safe_addstr(stdscr, 6, 2,
                    "i  import from a local file",
                    curses.color_pair(COLOR_DIM))
        safe_addstr(stdscr, 7, 2,
                    "G  import from GitHub  (user/repo/path/to/file.json)",
                    curses.color_pair(COLOR_DIM))

        draw_desc(stdscr, "", flash)
        flash = None
        draw_footer(stdscr, "  g generate  i import file  G import GitHub  q back")
        stdscr.refresh()

        key = stdscr.getch()
        if key in (ord("q"), ord("Q"), 27):
            return None

        elif key == ord("g"):
            path = generate_personal_config()
            flash = (f"  ✓ Saved to {str(path).replace(str(Path.home()), '~')}", COLOR_SELECT)

        elif key == ord("i"):
            stdscr.erase()
            draw_header(stdscr, " import file ")
            safe_addstr(stdscr, 2, 2, "Path to personal.json:", curses.color_pair(COLOR_DIM))
            draw_footer(stdscr, "  enter confirm  esc cancel")
            stdscr.refresh()
            default = str(PERSONAL_CONFIG_FILE).replace(str(Path.home()), "~")
            path_str = _prompt_line(stdscr, 4, "path", default)
            if path_str:
                path_str = path_str.replace("~", str(Path.home()), 1)
                try:
                    data = json.loads(Path(path_str).read_text())
                    err  = _validate_personal_config(data)
                    if err:
                        flash = (f"  ✗ {err}", COLOR_ERROR)
                    else:
                        to_install = apply_personal_config(data)
                        if to_install:
                            return ("install", to_install)
                        flash = ("  ✓ Config applied", COLOR_SELECT)
                except FileNotFoundError:
                    flash = (f"  ✗ File not found: {path_str}", COLOR_ERROR)
                except json.JSONDecodeError as e:
                    flash = (f"  ✗ JSON error: {e}", COLOR_ERROR)

        elif key == ord("G"):
            stdscr.erase()
            draw_header(stdscr, " import from GitHub ")
            safe_addstr(stdscr, 2, 2, "Format: user/repo/path/to/file.json",
                        curses.color_pair(COLOR_DIM))
            draw_footer(stdscr, "  enter confirm  esc cancel")
            stdscr.refresh()
            spec = _prompt_line(stdscr, 4, "spec")
            if spec:
                stdscr.erase()
                draw_header(stdscr, " import from GitHub ")
                safe_addstr(stdscr, 3, 2, f"Fetching {spec}…", curses.color_pair(COLOR_DIM))
                stdscr.refresh()
                try:
                    raw  = _fetch_github_raw(spec)
                    data = json.loads(raw)
                    err  = _validate_personal_config(data)
                    if err:
                        flash = (f"  ✗ {err}", COLOR_ERROR)
                    else:
                        to_install = apply_personal_config(data)
                        if to_install:
                            return ("install", to_install)
                        flash = ("  ✓ Config applied from GitHub", COLOR_SELECT)
                except ValueError as e:
                    flash = (f"  ✗ {e}", COLOR_ERROR)
                except json.JSONDecodeError as e:
                    flash = (f"  ✗ JSON error: {e}", COLOR_ERROR)


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
        nav_num = 0
        for i, (label, path, _desc) in enumerate(menu):
            y = 2 + i
            if path == SEPARATOR:
                safe_addstr(stdscr, y, 2, "─" * min(20, w - 4),
                            curses.color_pair(COLOR_DIM))
                continue
            nav_num += 1
            num_str = str(nav_num) if nav_num <= 9 else " "
            if i == idx:
                safe_addstr(stdscr, y, 0, "▶",
                            curses.color_pair(COLOR_SELECT) | curses.A_BOLD)
                safe_addstr(stdscr, y, 2, f"{num_str} {label:<{label_w}}", curses.A_BOLD)
                if dev and label == "Developer":
                    safe_addstr(stdscr, y, 4 + label_w + 2, "[dev mode ON]",
                                curses.color_pair(COLOR_SELECT))
            else:
                safe_addstr(stdscr, y, 0, f"  {num_str} {label:<{label_w}}")
                if dev and label == "Developer":
                    safe_addstr(stdscr, y, 4 + label_w + 2, "[dev mode ON]",
                                curses.color_pair(COLOR_DIM))

        draw_desc(stdscr, menu[idx][2])
        draw_footer(stdscr, " 1-9 jump  j/k navigate  enter select  q quit")
        stdscr.refresh()

        key = stdscr.getch()
        if key in (ord("q"), ord("Q"), 27):
            return None

        pos_in_nav = nav_indices.index(idx) if idx in nav_indices else 0
        if ord("1") <= key <= ord("9"):
            n = key - ord("1")  # 0-based
            if n < len(nav_indices):
                idx = nav_indices[n]
        elif key in (ord("j"), curses.KEY_DOWN):
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
            elif path == SSM_ACTION:
                result = run_ssm_view(stdscr)
                if result is not None:
                    return result
            elif path == PROFILE_ACTION:
                result = run_profile_view(stdscr)
                if result is not None:
                    return result
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
        elif action == "launch_ssm":
            ssm_py = DOTS_DIR / "src/zsh/zsh/ssm.py"
            os.execvp("python3", ["python3", "-B", str(ssm_py)])


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
        _VALID_KEYS = set(_DEFAULT_SETTINGS) | {"theme"}
        if i + 1 >= len(sys.argv) or "=" not in sys.argv[i + 1]:
            print("error: --set requires key=value", file=sys.stderr)
            sys.exit(1)
        k, v = sys.argv[i + 1].split("=", 1)
        if k not in _VALID_KEYS:
            print(f"error: unknown setting '{k}'. Valid keys: {', '.join(sorted(_VALID_KEYS))}", file=sys.stderr)
            sys.exit(1)
        _BOOL_MAP = {"true": True, "false": False, "1": True, "0": False}
        def _coerce(val: str):
            if val.lower() in _BOOL_MAP:
                return _BOOL_MAP[val.lower()]
            try:
                return int(val)
            except ValueError:
                return val
        s = load_settings()
        s[k] = _coerce(v)
        save_settings(s)
        print(f"  Set {k} = {s[k]}")
    elif "--theme" in sys.argv:
        i = sys.argv.index("--theme")
        if i + 1 < len(sys.argv):
            name = sys.argv[i + 1]
            if set_ghostty_theme(name):
                print(f"  Theme set to: {name}")
            else:
                print("  Failed to update ghostty config", file=sys.stderr)
                sys.exit(1)
    elif "--setting-file-generate" in sys.argv or "-SFG" in sys.argv:
        args   = sys.argv[1:]
        flag_i = next((i for i, a in enumerate(args) if a in ("--setting-file-generate", "-SFG")), -1)
        dest   = None
        if flag_i >= 0 and flag_i + 1 < len(args) and not args[flag_i + 1].startswith("-"):
            dest = Path(args[flag_i + 1]).expanduser()
        path = generate_personal_config(dest)
        print(f"  ✓ Personal config saved to {path}")
    elif "--import" in sys.argv or "-i" in sys.argv:
        args   = sys.argv[1:]
        flag_i = next((i for i, a in enumerate(args) if a in ("--import", "-i")), -1)
        is_git = any(a in ("--git", "-g") for a in args)
        if is_git:
            git_i = next(i for i, a in enumerate(args) if a in ("--git", "-g"))
            if git_i + 1 >= len(args):
                print("error: --git requires user/repo/path/to/file.json", file=sys.stderr)
                sys.exit(1)
            spec = args[git_i + 1]
            print(f"  Fetching {spec}…")
            try:
                raw  = _fetch_github_raw(spec)
                data = json.loads(raw)
            except (ValueError, json.JSONDecodeError) as e:
                print(f"  ✗ {e}", file=sys.stderr)
                sys.exit(1)
        else:
            if flag_i < 0 or flag_i + 1 >= len(args):
                print("error: --import requires a path or --git spec", file=sys.stderr)
                sys.exit(1)
            path_str = args[flag_i + 1]
            try:
                data = json.loads(Path(path_str).expanduser().read_text())
            except FileNotFoundError:
                print(f"  ✗ File not found: {path_str}", file=sys.stderr)
                sys.exit(1)
            except json.JSONDecodeError as e:
                print(f"  ✗ JSON error: {e}", file=sys.stderr)
                sys.exit(1)
        err = _validate_personal_config(data)
        if err:
            print(f"  ✗ Invalid config: {err}", file=sys.stderr)
            sys.exit(1)
        to_install = apply_personal_config(data)
        print("  ✓ Settings and theme applied.")
        if to_install:
            names = ", ".join(d.brew or d.bin for d in to_install)
            print(f"\n  Packages from profile not yet installed: {names}")
            print()
            install_deps_cli(to_install)
        else:
            print("  ✓ All profile packages already installed.")
    else:
        main()
