// Content + command registry for the dots terminal.
//
// Every command returns an array of "blocks" that js/term.js knows how to
// render and stream. Block kinds:
//
//   { tool, args, out, collapse }  a tool call:  ⏺ Read(README.md) / ⎿ output
//   { text }                      assistant prose (inline markdown)
//   { list }                      bulleted lines (inline markdown)
//   { kv }                        aligned label/value rows
//   { code, copy }                a copyable command block
//   { box, rows }                 titled box with kv rows
//   { todo }                      ☒ / ☐ checklist
//   { cmds }                      clickable command menu
//   { screen }                    raw monospace lines (TUI mock), streamed
//   { rule }                      labelled divider
//   { note }                      dim footnote
//
// Inline markdown: **bold**, `code`, [label](url), [label](cmd:/docs tui).

window.DOTS = (function () {

  var VERSION = '2.2.0';
  var REPO    = 'https://github.com/CtrlUserKnown/dots';
  var SITE    = 'https://ctrluserknown.github.io/dots/';
  var INSTALL = 'curl -fsSL ' + SITE + 'install.sh | sh';

  // ── /about ──────────────────────────────────────────────────────────────────

  function about() {
    return [
      { tool: 'Read', args: 'README.md', out: [
        '**dots** — a fast, cross-platform dotfiles manager with an',
        'interactive TUI, written in Rust.',
        'Platforms: macOS (Homebrew), Linux (apt/dnf)',
      ], collapse: 0 },
      { text: '**dots** installs your tools, wires up symlinks GNU Stow–style, applies premade app configs, and keeps everything healthy — on macOS and Linux alike. One static binary, no runtime dependencies.' },
      { text: 'The repo is the *tool*. Your actual dotfiles live in their own repo; dots manages the symlinks between that repo and your `$HOME`, and can adopt files that are already there (with backups).' },
      { rule: 'design goals' },
      { list: [
        '**Single static binary** — pure-Rust TLS, no OpenSSL, no keychain, optimized for size',
        '**Idempotent** — run it twice and nothing breaks; anything it replaces is backed up first',
        '**Cross-platform first** — one dependency list, resolved per platform via brew / apt / dnf',
        '**Scriptable** — every screen in the TUI has a plain CLI equivalent',
        '**Extensible** — [Lua plugins](cmd:/plugins) add their own dashboard panes',
      ] },
      { rule: 'stack' },
      { kv: [
        ['language', 'Rust — Cargo workspace, 2 crates'],
        ['tui', 'ratatui + the shared `tui-core` crate'],
        ['crates', '`crates/dots` (binary) · `crates/tui-core` (chrome)'],
        ['platforms', 'macOS · Linux'],
        ['license', 'MIT'],
      ] },
      { cmds: [
        ['/features', 'what it does, feature by feature'],
        ['/install',  'one-line install'],
        ['/docs',     'full documentation'],
      ] },
    ];
  }

  // ── /install ────────────────────────────────────────────────────────────────

  function install() {
    return [
      { text: 'One line. The installer figures out the rest:' },
      { code: INSTALL, copy: true },
      { rule: 'what the installer does' },
      { todo: [
        ['x', 'Detect your OS and architecture'],
        ['x', 'Clone the repo to `~/.dots`'],
        ['x', 'Download a prebuilt binary — or `cargo build` if no release matches'],
        ['x', 'Put `dots` on your `PATH` (`~/.dots/bin/dots`)'],
        ['x', 'Run `dots init` to write the default config'],
        [' ', 'Run `dots` to open the TUI'],
      ] },
      { rule: 'flags' },
      { code: 'install.sh [--version <tag>] [--dir <path>]' },
      { kv: [
        ['--version', 'pin a release tag, e.g. `--version v2.2.0`'],
        ['--dir',     'install somewhere other than `~/.dots`'],
      ] },
      { rule: 'from source' },
      { code: 'git clone ' + REPO + ' && cargo build --release', copy: true },
      { note: 'Binary lands at `target/release/dots`. To remove everything: `uninstall.sh`.' },
      { cmds: [
        ['/tui',  'see the dashboard you get after installing'],
        ['/docs requirements', 'what you need first'],
      ] },
    ];
  }

  // ── /features ───────────────────────────────────────────────────────────────

  function features() {
    return [
      { text: 'Six things dots does, in the order you\'ll meet them:' },
      { list: [
        '**Interactive TUI** — a ratatui dashboard covering symlink health, installed tools, shell plugins, app configs, and network status. [See it](cmd:/tui)',
        '**Symlink management** — declare links with `dots link add`, then create or repair them idempotently. Adopts files already in `$HOME`, with automatic backups.',
        '**Cross-platform installs** — one dependency list in `packages.rs`, resolved per platform through Homebrew, `apt`, or `dnf`, in required / optional / dev tiers.',
        '**App configs** — every directory in your dotfiles repo (`nvim/`, `git/`, `bat/`, …) shows up in the Configs screen with a status badge; preview its files and (un)install it by linking into `$HOME`.',
        '**Premade configs** — starter configs for Ghostty, Neovim, and opencode compiled into the binary, applied on demand.',
        '**Portable profiles** — export your setup to `personal.json` and re-import it on another machine, from disk or straight from GitHub.',
      ] },
      { rule: 'and' },
      { list: [
        '**Lua plugins** — [add your own dashboard panes](cmd:/plugins) with a few lines of Lua',
        '**Self-updating** — built-in update check, one-command upgrade with SHA-256 verified release tarballs',
        '**Network pane** — live connectivity, latency, DNS, and VPN status via platform probes',
      ] },
      { cmds: [
        ['/cli', 'every subcommand'],
        ['/install', 'get it'],
      ] },
    ];
  }

  // ── /tui ────────────────────────────────────────────────────────────────────

  // The dashboard mock. Panes are drawn by width — padding measures visible
  // characters — so the box lines up no matter how the cells are coloured.
  var PW = 33;                                     // pane width, two per row

  function vis(s)    { return s.replace(/<[^>]*>/g, '').length; }
  function rep(c, n) { return n > 0 ? new Array(n + 1).join(c) : ''; }
  function dim(s)    { return '<span class="dim">' + s + '</span>'; }
  function g(s)      { return '<span class="c-green">' + s + '</span>'; }

  function pane(title, rows, w) {
    var head = '╭─ ' + title + ' ';
    var out  = [dim(head + rep('─', w - head.length - 1) + '╮')];

    rows.forEach(function (r) {
      out.push(dim('│') + ' ' + r + rep(' ', w - 4 - vis(r)) + ' ' + dim('│'));
    });
    out.push(dim('╰' + rep('─', w - 2) + '╯'));
    return out;
  }

  // Two panes, side by side. Both are padded to the same height first.
  function columns(left, right) {
    var n = Math.max(left.rows.length, right.rows.length);
    while (left.rows.length  < n) left.rows.push('');
    while (right.rows.length < n) right.rows.push('');

    var a = pane(left.title,  left.rows,  PW);
    var b = pane(right.title, right.rows, PW);
    return a.map(function (line, i) { return line + b[i]; });
  }

  function dashboard() {
    return [
      '<b class="c-cyan"> dots </b> ' + dim('dotfiles manager') +
        rep(' ', 24) + dim('q quit   ? help'),
    ].concat(
      columns(
        { title: 'Symlinks', rows: [
          g('●') + ' ghostty   ' + dim('~/.config/ghostty'),
          g('●') + ' nvim      ' + dim('~/.config/nvim'),
          g('●') + ' zsh       ' + dim('~/.zshrc'),
          '<span class="c-yellow">○</span> tmux      <span class="c-yellow">missing</span>',
          dim('4 links · 3 ok · 1 missing'),
        ] },
        { title: 'Tools', rows: [
          g('●') + ' eza       ' + dim('v1.1.0 ') + '  ' + g('ok'),
          g('●') + ' bat       ' + dim('v0.24.0') + '  ' + g('ok'),
          g('●') + ' fzf       ' + dim('v0.53.0') + '  ' + g('ok'),
          '<span class="c-red">✗</span> zoxide    ' + dim('—      ') + '  <span class="c-red">gone</span>',
          dim('7 tools · 6 installed'),
        ] }
      ),
      columns(
        { title: 'Configs', rows: [
          g('●') + ' nvim      ' + g('[ installed ]'),
          '<span class="c-yellow">◐</span> git       <span class="c-yellow">[  partial  ]</span>',
          dim('○ bat       [    none   ]'),
        ] },
        { title: 'Plugins', rows: [
          g('●') + ' zsh-autosuggestions   ' + g('ok'),
          g('●') + ' zsh-syntax-highlight  ' + g('ok'),
          g('●') + ' fzf-tab               ' + g('ok'),
        ] }
      ),
      pane('Network', [
        g('●') + ' online   ' + dim('12ms') + '   Wi-Fi   ' + dim('dns') +
          ' 1.1.1.1   ' + dim('vpn') + ' off',
      ], PW * 2),
      [dim(' ↑↓←→/hjkl') + ' move  ' + dim('enter') + ' open  ' + dim('1-3') +
        ' aliases/profile/theme  ' + dim('space') + ' settings']
    );
  }

  function tui() {
    return [
      { text: 'Running `dots` with no arguments opens the dashboard — a grid of five panes over your machine\'s state:' },
      { screen: dashboard() },
      { rule: 'screens' },
      { kv: [
        ['Health',   'behind Symlinks / Tools / Plugins — repair links, install missing tools'],
        ['Configs',  'browse configs found in your dotfiles repo, preview files, install or remove'],
        ['Aliases',  'built-in and user aliases; add, edit, remove'],
        ['Profile',  'export and import your personal configuration'],
        ['Theme',    'pick from 200+ built-in Ghostty themes'],
        ['Settings', 'update checks, greeting, developer mode — and the update popup'],
      ] },
      { rule: 'keys' },
      { kv: [
        ['hjkl / arrows', 'move focus'],
        ['enter',         'open the focused pane'],
        ['1 2 3',         'Aliases · Profile · Theme'],
        ['space',         'Settings (with update check)'],
        ['esc',           'back'],
        ['q / ctrl+c',    'quit'],
        ['mouse',         'wheel scrolls, left-click focuses and opens a pane'],
      ] },
      { note: 'Plugin panes join the same grid — see [/plugins](cmd:/plugins).' },
    ];
  }

  // ── /cli ────────────────────────────────────────────────────────────────────

  function cli() {
    return [
      { text: 'Everything the TUI does is scriptable:' },
      { kv: [
        ['dots',                        'open the interactive TUI'],
        ['dots health [--fix]',         'check and repair symlinks, tools, and plugins'],
        ['dots update',                 'check for and apply updates'],
        ['dots install <name>',         'install a single dependency'],
        ['dots install --all',          'install all missing **required** dependencies'],
        ['dots install --optional',     'install all missing **optional** dependencies'],
        ['dots aliases list|add|remove','manage shell aliases'],
        ['dots premade list|apply|remove <app>', 'bundled starter configs (ghostty, neovim, opencode)'],
        ['dots config list|view|install|remove <name>', 'app configs discovered in your dotfiles repo'],
        ['dots link add <src> <dst>',   'adopt a file/dir and symlink it (recorded in `links.toml`)'],
        ['dots link list|apply|remove <target>', 'inspect, create/repair, or remove declared links'],
        ['dots profile generate [path]','export your setup to `personal.json`'],
        ['dots profile import <path>',  'import a `personal.json` from disk'],
        ['dots profile import-git <user/repo/path.json>', 'import one straight from GitHub'],
        ['dots plugins list|new <name>|dir', 'manage Lua plugins'],
        ['dots init [--quiet]',         'initialize config (idempotent; the installer runs it)'],
        ['dots --version',              'print the version'],
      ] },
      { cmds: [['/docs cli', 'the same list with notes'], ['/tui', 'the interactive side']] },
    ];
  }

  // ── /plugins ────────────────────────────────────────────────────────────────

  function plugins() {
    return [
      { text: 'Drop `*.lua` files in `~/.dots/plugins/` to add your own panes to the dashboard — GitHub, AWS, anything a shell command can report. Plugins load at startup, run on the TUI thread, and each pane refreshes on its own interval. One that fails to load is reported by `dots plugins list` and skipped; it never crashes the TUI.' },
      { code: 'dots plugins new github   # scaffold, then edit', copy: true },
      { rule: 'ui' },
      { kv: [
        ['ui.pane{ … }',            'register a dashboard pane'],
        ['ui.layout{ columns = N }','set the dashboard column count (default 2)'],
      ] },
      { rule: 'ui.pane fields' },
      { kv: [
        ['render',   '`function() → lines` — a string or a table of strings *(the only one you always need)*'],
        ['id',       'stable identifier, shown by `dots plugins list`'],
        ['title',    'pane title (defaults to `id`)'],
        ['size',     'row-height weight — `2` makes the pane twice as tall'],
        ['span',     'columns spanned — `2` is full width in a 2-column grid'],
        ['refresh',  'seconds between `render()` calls (default 30)'],
        ['on_enter', '`function()` run when the pane is opened'],
      ] },
      { rule: 'dots helpers' },
      { kv: [
        ['dots.sh(cmd)', 'trimmed stdout of `sh -c cmd` (`""` on failure)'],
        ['dots.env(n)',  'an environment variable (`""` if unset)'],
        ['dots.dir()',   'the `~/.dots` path'],
      ] },
      { rule: 'example' },
      { screen: [
        '<span class="c-mauve">ui.pane</span>{',
        '  id = <span class="c-green">"github"</span>, title = <span class="c-green">"GitHub"</span>, span = <span class="c-yellow">2</span>, refresh = <span class="c-yellow">120</span>,',
        '  render = <span class="c-mauve">function</span>()',
        '    <span class="c-mauve">local</span> prs = dots.<span class="c-blue">sh</span>(<span class="c-green">"gh pr list --author @me --state open | wc -l"</span>)',
        '    <span class="c-mauve">if</span> prs == <span class="c-green">""</span> <span class="c-mauve">then return</span> { <span class="c-green">"gh not available"</span> } <span class="c-mauve">end</span>',
        '    <span class="c-mauve">return</span> { prs .. <span class="c-green">" PR(s) open"</span> }',
        '  <span class="c-mauve">end</span>,',
        '}',
      ] },
      { note: 'Ready-to-use examples live in [`examples/plugins/`](' + REPO + '/tree/main/examples/plugins) — `github.lua`, `aws.lua`.' },
    ];
  }

  // ── /theme ──────────────────────────────────────────────────────────────────

  var PALETTE = [
    ['bg',      '#1a1a1a'], ['fg',      '#d4d4d4'],
    ['black',   '#1a1a1a'], ['brblack', '#585b70'],
    ['red',     '#f38ba8'], ['green',   '#a6e3a1'],
    ['yellow',  '#f9e2af'], ['blue',    '#89b4fa'],
    ['magenta', '#cba6f7'], ['cyan',    '#89dceb'],
    ['white',   '#d4d4d4'], ['brwhite', '#ffffff'],
  ];

  function theme() {
    var swatches = PALETTE.map(function (p) {
      return '<span class="swatch"><i style="background:' + p[1] + '"></i>' +
             p[0] + ' <span class="dim">' + p[1] + '</span></span>';
    }).join('');

    return [
      { text: 'This site — and this session — wear **noir-cat**, the Ghostty theme that ships with dots: catppuccin-ish accents over a near-black `#1a1a1a`.' },
      { raw: '<div class="swatches">' + swatches + '</div>' },
      { rule: 'ghostty' },
      { screen: [
        'background = <span class="c-green">1a1a1a</span>',
        'foreground = <span class="c-green">d4d4d4</span>',
        'cursor-color = <span class="c-green">#ffffff</span>',
        'selection-background = <span class="c-green">313244</span>',
        '',
        'palette = <span class="c-yellow">1</span>=<span class="c-red">#f38ba8</span>   palette = <span class="c-yellow">4</span>=<span class="c-blue">#89b4fa</span>',
        'palette = <span class="c-yellow">2</span>=<span class="c-green">#a6e3a1</span>   palette = <span class="c-yellow">5</span>=<span class="c-mauve">#cba6f7</span>',
        'palette = <span class="c-yellow">3</span>=<span class="c-yellow">#f9e2af</span>   palette = <span class="c-yellow">6</span>=<span class="c-cyan">#89dceb</span>',
      ] },
      { text: 'Pick it — or any of 200+ built-in Ghostty themes — from the TUI\'s Theme screen (`3` on the dashboard).' },
      { note: 'Also bundled: **knew-pines** for Ghostty, **KnewPines** / Moon / Dawn for `bat`, and the **charModel** zsh prompt.' },
    ];
  }

  // ── /docs ───────────────────────────────────────────────────────────────────

  var TOPICS = {
    requirements: function () { return [
      { text: 'dots runs on macOS and Linux as a single static binary — no runtime dependencies, no OpenSSL, no keychain.' },
      { kv: [
        ['macOS', 'Homebrew for package installs'],
        ['Linux', '`apt` (Debian/Ubuntu) or `dnf` (Fedora/RHEL)'],
        ['git',   'used for the repo checkout and self-update'],
        ['shell', 'zsh, bash, or fish'],
        ['rust',  'only if you build from source (`cargo build --release`)'],
      ] },
    ]; },

    structure: function () { return [
      { text: 'A Cargo workspace with two crates:' },
      { kv: [
        ['crates/dots',      'the binary — CLI, TUI screens, installer, symlink/link engine, config, Lua plugin host, bundled premade `assets/`'],
        ['crates/tui-core',  'shared TUI chrome: header/footer/description bars, color theme, flash model'],
      ] },
      { rule: 'on disk' },
      { screen: [
        '<span class="c-blue">~/.dots/</span>',
        '├── <span class="c-blue">bin/</span>dots            <span class="dim">the binary on your PATH</span>',
        '├── settings.toml       <span class="dim">tool settings</span>',
        '├── links.toml          <span class="dim">declared symlinks</span>',
        '└── <span class="c-blue">plugins/</span>            <span class="dim">your Lua panes</span>',
        '<span class="c-blue">~/.personal/</span>            <span class="dim">machine-local layer</span>',
        '├── aliases.zsh         <span class="dim">sourced after built-in aliases</span>',
        '├── <span class="c-blue">apps/</span>',
        '└── config.toml         <span class="dim">overrides settings.toml</span>',
      ] },
    ]; },

    symlinks: function () { return [
      { text: 'Two ways to get files from your dotfiles repo into `$HOME`:' },
      { rule: 'explicit links' },
      { code: 'dots link add ~/dotfiles/nvim ~/.config/nvim', copy: true },
      { text: 'Recorded in `links.toml`. `dots link apply` creates or repairs every declared link; anything already at the target is adopted and backed up first.' },
      { rule: 'stow-style folding' },
      { text: 'A directory per app in your dotfiles repo is folded into `$HOME` the way GNU Stow would do it — `nvim/.config/nvim/init.lua` becomes `~/.config/nvim/init.lua`.' },
      { rule: 'statuses' },
      { kv: [
        ['<span class="c-green">ok</span>',           'target is a symlink pointing where it should'],
        ['<span class="c-yellow">missing</span>',     'nothing at the target yet'],
        ['<span class="c-red">broken</span>',         'symlink exists, source is gone'],
        ['<span class="c-yellow">wrong target</span>','symlink points somewhere else'],
      ] },
      { note: '`dots health --fix` walks every link and repairs what it can.' },
    ]; },

    deps: function () { return [
      { text: 'The dependency list lives in [`crates/dots/src/packages.rs`](' + REPO + '/blob/main/crates/dots/src/packages.rs), each entry mapped to its `brew` / `dnf` / `apt` name.' },
      { kv: [
        ['required', 'installed by `dots install --all` — git, eza, bat, fd, fzf, fastfetch, zoxide'],
        ['optional', 'installed by `dots install --optional` — neovim, herdr, btop, lazygit, yazi, carapace'],
        ['dev',      'installed on request with `dots install <name>` — go, lua, cmake, gcc, ripgrep, gh, docker, ffmpeg, …'],
      ] },
      { code: 'dots install --all && dots install --optional', copy: true },
    ]; },

    configs: function () { return [
      { text: 'Every directory in your dotfiles repo shows up in the Configs screen with an install badge — `[ installed ]`, `[ partial ]`, or `[ not installed ]`. Open one to list its files, preview contents, and install or remove it by (un)linking into `$HOME`.' },
      { code: 'dots config list\ndots config view nvim\ndots config install nvim' },
      { rule: 'premade' },
      { text: 'Starter configs for **Ghostty**, **Neovim**, and **opencode** are compiled into the binary and applied on demand. Existing files are backed up.' },
      { code: 'dots premade list\ndots premade apply ghostty' },
    ]; },

    profiles: function () { return [
      { text: 'A profile is your setup as data — tools, links, aliases, configs — so a new machine is one import away.' },
      { code: 'dots profile generate ~/personal.json          # export\ndots profile import ~/personal.json            # from disk\ndots profile import-git user/repo/personal.json # from GitHub' },
    ]; },

    settings: function () { return [
      { text: 'Tool settings live in `~/.dots/settings.toml` under a `[dots]` table:' },
      { kv: [
        ['update_check',     '`true` — check for updates periodically'],
        ['update_frequency', '`1440` — minutes between update checks'],
        ['greeting',         '`true` — show the greeting banner'],
        ['developer_mode',   '`false` — enable developer features'],
        ['theme',            '*(empty)* — selected theme'],
      ] },
      { text: '`~/.personal/config.toml` overrides any of it, per machine. Everything here is also togglable from the Settings screen (`space` on the dashboard).' },
    ]; },

    aliases: function () { return [
      { text: 'dots ships a set of shell aliases and manages yours alongside them.' },
      { code: 'dots aliases list\ndots aliases add gs "git status"\ndots aliases remove gs' },
      { text: 'User aliases land in `~/.personal/aliases.zsh`, sourced after the built-ins so yours win. The Aliases screen (`1` on the dashboard) does the same interactively.' },
    ]; },

    updating: function () { return [
      { text: 'dots checks for new releases in the background (`update_check`, every `update_frequency` minutes) and surfaces them in the Settings popup.' },
      { code: 'dots update', copy: true },
      { text: 'Updates download a release tarball from GitHub Releases and verify its SHA-256 before swapping the binary. If dots was installed through a package manager, it defers to that instead.' },
    ]; },

    dev: function () { return [
      { text: 'Working on dots itself:' },
      { code: 'cargo test --all\ncargo clippy --all -- -D warnings\ncargo fmt --all -- --check\nbash tests/integration/test_setup.sh' },
      { text: 'An isolated container environment lives in [`test-env/`](' + REPO + '/tree/main/test-env); the manual QA checklist is in [`docs/manual-test-checklist.md`](' + REPO + '/blob/main/docs/manual-test-checklist.md). CI runs on Linux and macOS.' },
    ]; },
  };

  var TOPIC_LIST = [
    ['requirements', 'platforms and prerequisites'],
    ['structure',    'crates, and what lands on disk'],
    ['symlinks',     'explicit links, stow folding, statuses'],
    ['deps',         'the required / optional / dev tiers'],
    ['configs',      'app configs and premade starters'],
    ['profiles',     'export and import your setup'],
    ['settings',     'settings.toml and the personal layer'],
    ['aliases',      'managing shell aliases'],
    ['updating',     'self-update and verification'],
    ['dev',          'tests, lints, CI'],
  ];

  function docs(arg) {
    var key = (arg || '').trim().toLowerCase();

    if (key && TOPICS[key]) {
      return [{ tool: 'Read', args: 'docs/' + key + '.md', out: ['(' + key + ')'], collapse: 0 }]
        .concat(TOPICS[key]());
    }

    if (key) {
      return [
        { err: 'No doc topic `' + key + '`.' },
        { cmds: TOPIC_LIST.map(function (t) { return ['/docs ' + t[0], t[1]]; }) },
      ];
    }

    return [
      { text: 'Documentation, by topic — pick one:' },
      { cmds: TOPIC_LIST.map(function (t) { return ['/docs ' + t[0], t[1]]; }) },
      { note: 'Related: [/cli](cmd:/cli) · [/tui](cmd:/tui) · [/plugins](cmd:/plugins) · [full README](' + REPO + '#readme)' },
    ];
  }

  // ── /changelog ──────────────────────────────────────────────────────────────

  var SECTION_COLOR = {
    Added: 'c-green', Changed: 'c-blue', Fixed: 'c-yellow', Removed: 'c-red',
  };

  function parseChangelog(text) {
    var entries = [], current = null, section = null;

    text.split('\n').forEach(function (line) {
      var v = line.match(/^## \[(.+?)\](?: - (.+))?/);
      if (v) {
        current = { version: v[1], date: v[2] || '', sections: [] };
        entries.push(current);
        section = null;
        return;
      }
      if (!current) return;

      var s = line.match(/^### (.+)/);
      if (s) {
        section = { name: s[1], items: [] };
        current.sections.push(section);
        return;
      }
      if (section && line.indexOf('- ') === 0) section.items.push(line.slice(2).trim());
    });

    // Keep only shipped versions: `[Unreleased]` is a staging area for the next
    // tag, and letting it through would render a rule reading "vUnreleased" and
    // report it as the latest release.
    return entries.filter(function (e) {
      return e.sections.length && !/^unreleased$/i.test(e.version);
    });
  }

  // Async: term.js awaits the returned promise before streaming.
  function changelog(arg) {
    var all = (arg || '').trim() === 'all';

    return fetch('CHANGELOG.md')
      .then(function (res) {
        if (!res.ok) throw new Error('HTTP ' + res.status);
        return res.text();
      })
      .then(function (text) {
        var entries = parseChangelog(text);
        if (!entries.length) throw new Error('empty');

        var shown = all ? entries : entries.slice(0, 4);
        var blocks = [{ tool: 'Read', args: 'CHANGELOG.md', out: [
          entries.length + ' releases · latest **v' + entries[0].version + '**',
        ], collapse: 0 }];

        shown.forEach(function (e) {
          blocks.push({ rule: 'v' + e.version + (e.date ? '  ·  ' + e.date : '') });
          e.sections.forEach(function (s) {
            var cls = SECTION_COLOR[s.name] || 'c-mauve';
            blocks.push({ raw: '<div class="cl-sec ' + cls + '">' + s.name.toLowerCase() + '</div>' });
            blocks.push({ list: s.items });
          });
        });

        if (!all) {
          blocks.push({ note: 'Showing ' + shown.length + ' of ' + entries.length +
            ' releases — [/changelog all](cmd:/changelog all) for everything.' });
        }
        return blocks;
      })
      .catch(function () {
        return [
          { err: 'Could not read `CHANGELOG.md` from this host.' },
          { note: '[Read it on GitHub](' + REPO + '/blob/main/CHANGELOG.md) instead.' },
        ];
      });
  }

  // ── /status ─────────────────────────────────────────────────────────────────

  function status() {
    var ua = navigator.userAgent;
    var os = /Mac/.test(ua) ? 'macOS' : /Linux|X11/.test(ua) ? 'Linux'
           : /Win/.test(ua) ? 'Windows *(dots is macOS/Linux only)*' : 'unknown';

    return [
      { box: 'dots status', rows: [
        ['version',  'v' + VERSION],
        ['repo',     '[CtrlUserKnown/dots](' + REPO + ')'],
        ['site',     '[ctrluserknown.github.io/dots](' + SITE + ')'],
        ['license',  'MIT'],
        ['language', 'Rust'],
        ['your os',  os],
        ['session',  'noir-cat · web mock'],
      ] },
      { note: 'This page is a mock of the real thing. [Install dots](cmd:/install) to get the actual TUI.' },
    ];
  }

  // ── neofetch ────────────────────────────────────────────────────────────────

  function neofetch() {
    return [
      { raw:
        '<div class="fetch">' +
          '<pre class="fetch-art c-mauve">' +
'  ██████╗  ██████╗ ████████╗███████╗\n' +
'  ██╔══██╗██╔═══██╗╚══██╔══╝██╔════╝\n' +
'  ██║  ██║██║   ██║   ██║   ███████╗\n' +
'  ██║  ██║██║   ██║   ██║   ╚════██║\n' +
'  ██████╔╝╚██████╔╝   ██║   ███████║\n' +
'  ╚═════╝  ╚═════╝    ╚═╝   ╚══════╝' +
          '</pre>' +
          '<div class="fetch-info">' +
            row('user', 'you@dots') +
            row('os', 'macOS / Linux') +
            row('shell', 'zsh · bash · fish') +
            row('wm', 'ratatui') +
            row('theme', 'noir-cat') +
            row('lang', 'Rust') +
            row('version', 'v' + VERSION) +
            row('license', 'MIT') +
          '</div>' +
        '</div>' },
      { note: 'Try [/about](cmd:/about), [/features](cmd:/features), or [/install](cmd:/install).' },
    ];

    function row(k, v) {
      return '<div class="fetch-row"><span class="c-cyan">' + k + '</span>' +
             '<span class="dim">:</span> <span>' + v + '</span></div>';
    }
  }

  // ── simulated shell output ──────────────────────────────────────────────────

  function health() {
    return [
      { screen: [
        '<span class="dim">checking symlinks…</span>',
        '  <span class="c-green">✓</span> ghostty      <span class="dim">~/.config/ghostty/config</span>',
        '  <span class="c-green">✓</span> nvim         <span class="dim">~/.config/nvim</span>',
        '  <span class="c-green">✓</span> zsh          <span class="dim">~/.zshrc</span>',
        '  <span class="c-yellow">!</span> tmux         <span class="c-yellow">missing</span> <span class="dim">— run with --fix</span>',
        '<span class="dim">checking tools…</span>',
        '  <span class="c-green">✓</span> git eza bat fd fzf fastfetch',
        '  <span class="c-red">✗</span> zoxide       <span class="c-red">not installed</span>',
        '<span class="dim">checking plugins…</span>',
        '  <span class="c-green">✓</span> 3 zsh plugins active',
        '',
        '<span class="c-yellow">2 issues</span> <span class="dim">· `dots health --fix` repairs links and installs what\'s missing</span>',
      ] },
    ];
  }

  function installRun() {
    return [
      { screen: [
        '<span class="dim">  ▸</span> detecting platform…            <span class="c-green">linux-x86_64</span>',
        '<span class="dim">  ▸</span> cloning CtrlUserKnown/dots…    <span class="c-green">~/.dots</span>',
        '<span class="dim">  ▸</span> fetching release v' + VERSION + '…       <span class="c-green">2.1 MB</span>',
        '<span class="dim">  ▸</span> verifying sha-256…             <span class="c-green">ok</span>',
        '<span class="dim">  ▸</span> linking ~/.dots/bin/dots…      <span class="c-green">on PATH</span>',
        '<span class="dim">  ▸</span> dots init…                     <span class="c-green">settings.toml written</span>',
        '',
        '  <span class="c-green">✓ dots installed</span> — dots v' + VERSION,
      ] },
      { note: 'That was a mock — run it for real: [/install](cmd:/install). Then type `dots`.' },
    ];
  }

  var LS = [
    '<span class="c-blue">bat/</span>', '<span class="c-blue">ghostty/</span>',
    '<span class="c-blue">git/</span>', '<span class="c-blue">nvim/</span>',
    '<span class="c-blue">opencode/</span>', '<span class="c-blue">zsh/</span>',
    'links.toml', 'settings.toml', 'README.md',
  ];

  // ── commands ────────────────────────────────────────────────────────────────

  var COMMANDS = [
    { name: '/about',     desc: 'what dots is and why it exists',  run: about },
    { name: '/install',   desc: 'the one-line install, step by step', run: install },
    { name: '/features',  desc: 'everything dots does',            run: features },
    { name: '/tui',       desc: 'the interactive dashboard',       run: tui },
    { name: '/docs',      desc: 'documentation by topic',          run: docs, arg: '[topic]' },
    { name: '/cli',       desc: 'every subcommand',                run: cli },
    { name: '/plugins',   desc: 'write a Lua dashboard pane',      run: plugins },
    { name: '/changelog', desc: 'release history',                 run: changelog, arg: '[all]' },
    { name: '/theme',     desc: 'the noir-cat palette',            run: theme },
    { name: '/status',    desc: 'version, links, your platform',   run: status },
    { name: '/hosts',     desc: 'remote hosts and services',        run: hostTable },
    { name: '/host',      desc: 'open a host page',                run: hostPage, arg: '<name>' },
    { name: '/github',    desc: 'open the repository',             run: function () {
        window.open(REPO, '_blank', 'noopener');
        return [{ ok: 'Opening [' + REPO.replace('https://', '') + '](' + REPO + ')…' }];
      } },
    { name: '/keys',      desc: 'keyboard shortcuts for this page', run: null },  // term.js
    { name: '/clear',     desc: 'clear the session',               run: null },   // term.js
    { name: '/help',      desc: 'list commands',                   run: null },   // term.js
  ];

  // Bare shell commands, matched in order. Real terminals answer these, so
  // this one does too.
  var SHELL = [
    { re: /^(ls|eza|ll)\b/,          run: function () { return [{ raw: '<div class="ls">' + LS.join('') + '</div>' }]; } },
    { re: /^pwd$/,                   run: function () { return [{ screen: ['~/.dots'] }]; } },
    { re: /^whoami$/,                run: function () { return [{ screen: ['you'] }]; } },
    { re: /^date$/,                  run: function () { return [{ screen: [new Date().toString()] }]; } },
    { re: /^uname/,                  run: function () {
        var mac = /Mac/.test(navigator.userAgent);
        return [{ screen: [mac ? 'Darwin arm64' : 'Linux x86_64'] }];
      } },
    { re: /^(neo|fast)fetch$/,       run: neofetch },
    { re: /^dots\s+health/,          run: health },
    { re: /^dots\s+(-v|--version)$/, run: function () { return [{ screen: ['dots v' + VERSION] }]; } },
    { re: /^dots\s+(-h|--help)$/,    run: cli },
    { re: /^dots\s+plugins/,         run: plugins },
    { re: /^dots\s+(link|config|premade|profile|install|update|aliases|init)/, run: cli },
    { re: /^dots$/,                  run: tui },
    { re: /^man\s+dots$/,            run: function () { return docs(); } },
    { re: /^(cat|bat)\s+README/i,    run: about },
    { re: /^(cat|bat)\s+CHANGELOG/i, run: function () { return changelog(); } },
    { re: /^curl\b.*install\.sh/,    run: installRun },
    { re: /^(brew|apt|apt-get|dnf|pacman|yum)\b/, run: function () {
        return [{ text: 'dots drives your package manager for you — one dependency list, resolved per platform. See [/docs deps](cmd:/docs deps).' }];
      } },
    { re: /^git\s+clone/,            run: function () {
        return [{ text: 'The installer clones it for you: [/install](cmd:/install).' }];
      } },
    { re: /^(vim|nvim|nano|emacs|vi)\b/, run: function () {
        return [{ text: 'No editor in here. dots does symlink `~/.config/nvim` for you, though — [/docs configs](cmd:/docs configs).' }];
      } },
    { re: /^sudo\b/,                 run: function () {
        return [{ text: 'No `sudo` needed — dots installs to `~/.dots` and never writes outside `$HOME` without backing up first.' }];
      } },
    { re: /^rm\s+-rf/,               run: function () {
        return [{ err: 'Not in my session. If you want dots gone: `uninstall.sh`.' }];
      } },
    { re: /^(echo)\s+(.*)$/,         run: function (m) { return [{ screen: [escapeHtml(m[2])] }]; } },
    { re: /^tree$/,                  run: function () { return TOPICS.structure(); } },
    { re: /^(fortune|cowsay)$/,      run: function () {
        return [{ text: 'A dotfiles repo you can\'t reinstall from is a screenshot.' }];
      } },
  ];

  function escapeHtml(s) {
    return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
  }

  // ── terminal helpers ───────────────────────────────────────────────────────────

  function dim(s)    { return '<span class="dim">' + s + '</span>'; }
  function green(s)  { return '<span class="c-green">' + s + '</span>'; }
  function cyan(s)   { return '<span class="c-cyan">' + s + '</span>'; }
  function yellow(s) { return '<span class="c-yellow">' + s + '</span>'; }

  // ── host data ─────────────────────────────────────────────────────────────────

  var HOSTS = [
    { name: 'docs',     host: 'docs@prod.userknown.com',         port: '22',   ping: '52',  tags: 'prod web',        desc: 'documentation server for the dots project.' },
    { name: 'about',    host: 'info@profile.knowlange.org',      port: '8081', ping: '102', tags: 'mange ip',        desc: 'profile and about page hosting.' },
    { name: 'git',      host: 'gh@dev.source.io',                port: '91',   ping: '4',   tags: 'dev',             desc: 'git repository hosting and code review.' },
    { name: 'changelog',host: 'logs@alt.changelog.gg',           port: '1738', ping: '33',  tags: 'infra',           desc: 'centralised log and changelog aggregation.' },
    { name: 'install',  host: 'install@get.dots.git',            port: '77',   ping: '90',  tags: 'get download',    desc: 'install script and release distribution.' },
  ];

  var HOST_MAP = {};
  HOSTS.forEach(function (h) { HOST_MAP[h.name] = h; });

  // Generate VA (visualizer animation) HTML
  function va(num) {
    var n = Math.min(10, Math.max(3, Math.round(num / 12)));
    var bars = [];
    for (var i = 0; i < n; i++) bars.push('<span class="va-bar"></span>');
    return '<span class="va">' + bars.join('') + '</span>';
  }

  // ── /hosts ────────────────────────────────────────────────────────────────────

  function hostTable() {
    var blocks = [
      { text: '**hosts** — remote machines and services managed by dots.' },
      { raw: '<div class="host-table blk fade">' +
        '<div class="host-row"><span class="h-name c-cyan">name</span><span class="h-host c-cyan">host</span><span class="h-port c-cyan">port</span><span class="h-ping c-cyan">ping</span><span class="h-tags c-cyan">tags</span></div>' +
        '<div class="host-row" style="color:var(--dim)">' + escapeHtml(new Array(54).join('─')) + '</div>' +
        HOSTS.map(function (h) {
          return '<div class="host-row">' +
            '<a class="h-name cmdlink" data-cmd="/host ' + h.name + '">' + escapeHtml(h.name) + '</a>' +
            '<span class="h-host">' + escapeHtml(h.host) + '</span>' +
            '<span class="h-port">' + escapeHtml(h.port) + '</span>' +
            '<span class="h-ping">' + escapeHtml(h.ping) + 'ms' + va(parseInt(h.ping)) + '</span>' +
            '<span class="h-tags">' + escapeHtml(h.tags) + '</span>' +
            '</div>';
        }).join('') +
        '</div>' },
      { note: 'Click a name to open its page. Each host has server info, logs, and VA ping status.' },
    ];
    return blocks;
  }

  // ── /host <name> ──────────────────────────────────────────────────────────────

  function hostPage(name) {
    var h = HOST_MAP[name];
    if (!h) {
      return [
        { err: 'No host `' + name + '`.' },
        { cmds: HOSTS.map(function (h) { return ['/host ' + h.name, h.desc]; }) },
      ];
    }

    return [
      { tool: 'Read', args: 'hosts/' + h.name + '.md', out: [
        '<span class="c-mauve">#</span> <span class="c-cyan">' + h.name + '</span>  ' + dim('— ' + h.desc),
        '',
        dim('| key    | value'),
        dim('|--------|-------'),
        dim('| host') + '   | ' + escapeHtml(h.host),
        dim('| port') + '   | ' + escapeHtml(h.port),
        dim('| ping') + '   | ' + escapeHtml(h.ping) + 'ms ' + va(parseInt(h.ping)),
        dim('| tags') + '   | ' + escapeHtml(h.tags),
      ], collapse: 0 },
      { rule: h.name + ' — ' + h.desc },
      { text: '**' + h.name + '** serves as a ' + h.desc + ' It is reachable at <code>' + escapeHtml(h.host) + '</code> on port <code>' + h.port + '</code>.' },
      { screen: [
        dim('# ' + h.name),
        '',
        dim('> ') + h.desc.charAt(0).toUpperCase() + h.desc.slice(1),
        '',
        dim('## connection'),
        '',
        dim('| key ') + '  | ' + dim('value'),
        dim('|---') + '   | ' + dim('---'),
        dim('| host') + '   | ' + escapeHtml(h.host),
        dim('| port') + '   | ' + '<span class="c-yellow">' + escapeHtml(h.port) + '</span>',
        dim('| ping') + '   | ' + '<span class="c-green">' + escapeHtml(h.ping) + 'ms</span> ' + va(parseInt(h.ping)),
        dim('| tags') + '   | ' + escapeHtml(h.tags),
        '',
        dim('## services'),
        '',
        dim('  • ') + '<span class="c-green">sshd</span>  ' + dim('secure shell daemon — port ' + h.port),
        dim('  • ') + '<span class="c-green">nginx</span> ' + dim('reverse proxy (443)'),
        dim('  • ') + '<span class="c-green">dots-agent</span>  ' + dim('dots health monitor'),
        '',
        dim('## status'),
        '',
        dim('  ') + '<span class="c-green">●</span> online   ' + dim('uptime: 99.97%'),
        dim('  ') + '<span class="c-green">●</span> load     ' + dim('0.42 0.38 0.35'),
        dim('  ') + '<span class="c-green">●</span> disk     ' + dim('64% · 256G / 400G'),
      ] },
      { cmds: [
        ['/hosts', 'back to host list'],
      ] },
    ];
  }

  return {
    VERSION: VERSION,
    REPO:    REPO,
    SITE:    SITE,
    INSTALL: INSTALL,
    COMMANDS: COMMANDS,
    SHELL:    SHELL,
    TOPIC_LIST: TOPIC_LIST,
    escapeHtml: escapeHtml,
    HOSTS: HOSTS,
    HOST_MAP: HOST_MAP,
  };

})();
