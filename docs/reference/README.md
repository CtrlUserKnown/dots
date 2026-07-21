# Reference material

Third-party code kept purely as a design reference — **not compiled, not a dependency.**

- `lazygit-gui.go` — the core GUI/event-loop from [lazygit's fork of gocui](https://github.com/jesseduffield/gocui).
  Kept as the north-star for the ratatui-based `tui-core` framework (panels, focus,
  tabs, mouse, scrollbars, background-task spinner) behind the `dots` TUI. The `ssm`
  tool now lives in its own repo (`~/development/ssm`) with a vendored copy of the
  same chrome.
