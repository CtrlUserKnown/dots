# Dots website

One page, one terminal. The whole site is a fake shell session styled after an
agent CLI: you type, it streams an answer back. No nav bar, no separate pages —
every "page" is a command.

## files

| File | What it is |
|---|---|
| `index.html` | Page shell: top bar, transcript, composer. Content comes from JS. |
| `css/style.css` | The whole theme, on noir-cat variables. |
| `js/content.js` | Command registry + all site copy, as content blocks. |
| `js/term.js` | The session engine: streaming, input, history, slash menu. |
| `robots.txt`, `sitemap.xml`, `site.webmanifest` | Metadata (single URL). |

`CHANGELOG.md` and `install.sh` are copied into this directory at deploy time by
`.github/workflows/deploy-site.yml`; `/changelog` fetches the former at runtime.

## adding content

Everything a visitor can read lives in `js/content.js`. A command returns an
array of blocks, and `js/term.js` renders and streams them — block kinds are
documented at the top of that file (`text`, `list`, `kv`, `code`, `box`, `todo`,
`cmds`, `screen`, `tool`, `rule`, `note`, `raw`).

To add a command, append to `COMMANDS`; it shows up in `/help` and in the slash
menu automatically. To add a bare shell command (`ls`, `dots health`, …), append
to `SHELL`. Doc topics are entries in `TOPICS` plus a row in `TOPIC_LIST`.

Inline markdown in block strings: `**bold**`, `` `code` ``, `[label](url)`, and
`[label](cmd:/docs tui)` for a link that runs a command.

## rules

- Writing stays concise and sounds like the repo's own voice.
- Facts come from the repo itself — README, CHANGELOG, `crates/dots/src`. Check
  them before changing copy; keybindings and pane lists drift.
- Theme is **noir-cat** (`background 1a1a1a`, `foreground d4d4d4`, catppuccin
  accents), defined once as CSS variables in `:root`.
- Responsive down to a 390px phone. Wide content (the TUI mock, trees, code)
  scrolls inside its own box; the page itself never scrolls sideways.
- Works in Safari, Firefox, and Chrome; no build step, no dependencies, no
  framework. Jekyll stays off via `.nojekyll`.
- Anything unreachable without JavaScript gets a `<noscript>` summary with the
  install command and links to the repo.
