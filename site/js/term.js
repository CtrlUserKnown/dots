// The dots terminal: a streaming, keyboard-driven session that renders the
// content blocks defined in js/content.js.

(function () {

  var D = window.DOTS;

  var stream = document.getElementById('stream');
  var input  = document.getElementById('input');
  var popup  = document.getElementById('popup');
  var chips  = document.getElementById('chips');
  var modeEl = document.getElementById('mode');
  var ctxEl  = document.getElementById('ctx');

  var history = [];      // submitted lines, newest last
  var histIdx = -1;      // -1 = editing a fresh line
  var draft   = '';      // the line being edited when history nav started
  var busy    = false;   // a turn is streaming
  var run     = null;    // { cancelled } token for the streaming turn
  var sel     = 0;       // highlighted row in the slash popup
  var matches = [];      // current slash-popup matches
  var lastOut = null;    // last collapsed tool output, for ctrl+r
  var ctxLeft = 98;

  var MODES = [
    { label: '⏵⏵ auto-accept edits on', cls: 'c-green' },
    { label: '⏸ plan mode on',          cls: 'c-blue'  },
    { label: '⏵ normal mode',           cls: 'dim'     },
  ];
  var mode = 0;

  var VERBS = ['Reticulating', 'Symlinking', 'Stowing', 'Resolving', 'Unfolding',
               'Linting', 'Adopting', 'Puzzling', 'Compiling', 'Sniffing'];
  var SPARKS = ['✻', '✽', '✳', '✶', '✻', '✢'];

  // ── inline markdown ─────────────────────────────────────────────────────────

  function md(text) {
    return D.escapeHtml(text)
      .replace(/`([^`]+)`/g, '<code>$1</code>')
      .replace(/\*\*([^*]+)\*\*/g, '<strong>$1</strong>')
      .replace(/\*([^*]+)\*/g, '<em>$1</em>')
      .replace(/\[([^\]]+)\]\(cmd:([^)]+)\)/g, '<a class="cmdlink" data-cmd="$2">$1</a>')
      .replace(/\[([^\]]+)\]\(([^)]+)\)/g,
               '<a href="$2" target="_blank" rel="noopener noreferrer">$1</a>');
  }

  // Content strings are authored with markdown *and* the occasional raw span
  // (for TUI colouring), so escaping happens above but pre-coloured markup has
  // to survive: put spans back after escaping.
  function mdRich(text) {
    return md(text)
      .replace(/&lt;(\/?)(span|b|i)(\s[^&]*?)?&gt;/g, function (_, slash, tag, attrs) {
        return '<' + slash + tag + (attrs ? attrs.replace(/&quot;/g, '"') : '') + '>';
      });
  }

  // ── timing helpers ──────────────────────────────────────────────────────────

  function wait(ms) {
    return new Promise(function (resolve) { setTimeout(resolve, ms); });
  }

  function cancelled() { return !run || run.cancelled; }

  function scroll() {
    stream.scrollTop = stream.scrollHeight;
  }

  function add(html, cls) {
    var el = document.createElement('div');
    el.className = cls || '';
    el.innerHTML = html;
    stream.appendChild(el);
    scroll();
    return el;
  }

  // Reveal an element's text word by word, leaving markup intact.
  async function typeInto(el, html, speed) {
    el.innerHTML = html;

    var nodes = [], walker = document.createTreeWalker(el, NodeFilter.SHOW_TEXT);
    while (walker.nextNode()) nodes.push(walker.currentNode);

    var full = nodes.map(function (n) { return n.nodeValue; });
    nodes.forEach(function (n) { n.nodeValue = ''; });

    for (var i = 0; i < nodes.length; i++) {
      var words = full[i].split(/(\s+)/);
      for (var w = 0; w < words.length; w++) {
        if (cancelled()) { nodes[i].nodeValue = full[i]; break; }
        nodes[i].nodeValue += words[w];
        if (words[w].trim()) { scroll(); await wait(speed); }
      }
      if (cancelled()) {
        for (var j = i + 1; j < nodes.length; j++) nodes[j].nodeValue = full[j];
        break;
      }
    }
    scroll();
  }

  // ── block renderers ─────────────────────────────────────────────────────────

  async function renderBlock(b) {
    if (b.tool)              return renderTool(b);
    if (b.text)              return renderText(b.text);
    if (b.list)              return renderRows(b.list.map(function (i) {
                                    return '<span class="bullet">•</span> ' + mdRich(i); }), 'list');
    if (b.kv)                return renderKv(b.kv);
    if (b.cmds)              return renderCmds(b.cmds);
    if (b.code)              return renderCode(b);
    if (b.box)               return renderBox(b);
    if (b.todo)              return renderTodo(b.todo);
    if (b.screen)            return renderScreen(b.screen);
    if (b.rule)              return renderRule(b.rule);
    if (b.note)              return renderOne('<div class="note">' + mdRich(b.note) + '</div>');
    if (b.ok)                return renderOne('<div class="line"><span class="c-green">⏺</span> ' + mdRich(b.ok) + '</div>');
    if (b.err)               return renderOne('<div class="line"><span class="c-red">⏺</span> ' + mdRich(b.err) + '</div>');
    if (b.raw)               return renderOne(b.raw);
  }

  async function renderOne(html) {
    add(html, 'blk fade');
    await wait(50);
  }

  async function renderText(text) {
    var el = add('<span class="dot">⏺</span> <span class="body"></span>', 'blk msg');
    await typeInto(el.querySelector('.body'), mdRich(text), 9);
    await wait(60);
  }

  async function renderTool(b) {
    var el = add(
      '<span class="dot c-cyan">⏺</span> <span class="tool-name">' + D.escapeHtml(b.tool) + '</span>' +
      '<span class="tool-args">(' + D.escapeHtml(b.args || '') + ')</span>', 'blk tool');
    await wait(160);

    var out  = b.out || [];
    var keep = typeof b.collapse === 'number' ? b.collapse : 3;
    var head = keep > 0 ? out.slice(0, keep) : out;
    var rest = keep > 0 ? out.slice(keep) : [];

    var body = document.createElement('div');
    body.className = 'tool-out';
    body.innerHTML = '<span class="elbow">⎿</span><div class="tool-lines">' +
      head.map(function (l) { return '<div>' + mdRich(l) + '</div>'; }).join('') +
      (rest.length
        ? '<div class="more" data-more="1">… +' + rest.length + ' lines <span class="dim">(ctrl+r to expand)</span></div>' +
          '<div class="hidden-lines" hidden>' +
            rest.map(function (l) { return '<div>' + mdRich(l) + '</div>'; }).join('') + '</div>'
        : '') +
      '</div>';
    el.appendChild(body);
    if (rest.length) lastOut = body;
    scroll();
    await wait(120);
  }

  async function renderRows(rows, cls) {
    var wrap = add('', 'blk ' + (cls || ''));
    for (var i = 0; i < rows.length; i++) {
      if (cancelled()) { wrap.innerHTML += '<div class="row">' + rows[i] + '</div>'; continue; }
      var el = document.createElement('div');
      el.className = 'row fade';
      el.innerHTML = rows[i];
      wrap.appendChild(el);
      scroll();
      await wait(45);
    }
    await wait(40);
  }

  function renderKv(rows) {
    return renderRows(rows.map(function (r) {
      return '<span class="k">' + mdRich(r[0]) + '</span><span class="v">' + mdRich(r[1]) + '</span>';
    }), 'kv');
  }

  function renderTodo(items) {
    return renderRows(items.map(function (t) {
      var done = t[0] === 'x';
      return '<span class="' + (done ? 'c-green' : 'dim') + '">' + (done ? '☒' : '☐') + '</span> ' +
             '<span class="' + (done ? 'todo-done' : '') + '">' + mdRich(t[1]) + '</span>';
    }), 'todo');
  }

  function renderCmds(cmds) {
    return renderRows(cmds.map(function (c) {
      return '<a class="cmdlink" data-cmd="' + c[0] + '">' + D.escapeHtml(c[0]) + '</a>' +
             '<span class="cmd-desc">' + mdRich(c[1]) + '</span>';
    }), 'cmds');
  }

  function renderScreen(lines) {
    var wrap = add('<pre class="screen"></pre>', 'blk');
    var pre  = wrap.firstChild;

    return (async function () {
      for (var i = 0; i < lines.length; i++) {
        var el = document.createElement('div');
        el.className = 'fade';
        el.innerHTML = lines[i] === '' ? '&nbsp;' : lines[i];
        pre.appendChild(el);
        scroll();
        if (!cancelled()) await wait(28);
      }
      await wait(40);
    })();
  }

  async function renderCode(b) {
    var el = add(
      '<div class="code">' +
        '<pre>' + b.code.split('\n').map(function (l) {
          return '<span class="c-green">$</span> ' + D.escapeHtml(l);
        }).join('\n') + '</pre>' +
        (b.copy ? '<button class="copy" type="button">copy</button>' : '') +
      '</div>', 'blk fade');

    if (b.copy) {
      var btn = el.querySelector('.copy');
      btn.addEventListener('click', function () {
        copy(b.code);
        btn.textContent = 'copied';
        btn.classList.add('done');
        setTimeout(function () { btn.textContent = 'copy'; btn.classList.remove('done'); }, 1600);
      });
    }
    await wait(60);
  }

  async function renderBox(b) {
    var el = add('<div class="box"><div class="box-title">' + D.escapeHtml(b.box) + '</div></div>', 'blk fade');
    var box = el.firstChild;
    for (var i = 0; i < b.rows.length; i++) {
      var row = document.createElement('div');
      row.className = 'row fade';
      row.innerHTML = '<span class="k">' + mdRich(b.rows[i][0]) + '</span>' +
                      '<span class="v">' + mdRich(b.rows[i][1]) + '</span>';
      box.appendChild(row);
      scroll();
      await wait(45);
    }
  }

  async function renderRule(label) {
    add('<span class="rule-line"></span><span class="rule-label">' + D.escapeHtml(label) +
        '</span><span class="rule-line"></span>', 'blk rule fade');
    await wait(60);
  }

  // ── built-in commands ───────────────────────────────────────────────────────

  function helpBlocks() {
    return [
      { text: 'Commands — type one, or click it:' },
      { cmds: D.COMMANDS.map(function (c) {
          return [c.name + (c.arg ? ' ' + c.arg : ''), c.desc];
        }) },
      { note: 'Plain shell works too: `ls`, `neofetch`, `dots health`, `dots --help`, `man dots`. Or just ask a question.' },
    ];
  }

  function keysBlocks() {
    return [
      { box: 'shortcuts', rows: [
        ['enter',      'run the line'],
        ['↑ / ↓',      'walk your history'],
        ['tab',        'complete a slash command'],
        ['esc',        'close the menu, or interrupt the answer'],
        ['ctrl+r',     'expand the last truncated output'],
        ['ctrl+l',     'clear the session'],
        ['shift+tab',  'cycle the mode line'],
        ['?',          'this list'],
      ] },
    ];
  }

  var FALLBACKS = [
    [/instal|download|get started|setup|set up/i, 'Start here — one line, then run `dots`.', ['/install', '/docs requirements']],
    [/symlink|stow|link|adopt/i,                  'Symlinks are the core of it: declare them, then create or repair them idempotently.', ['/docs symlinks', '/cli']],
    [/theme|color|colour|noir|palette/i,          'The session you\'re in is wearing noir-cat, the theme dots ships.', ['/theme']],
    [/plugin|lua|extend|pane/i,                   'Panes are pluggable — a few lines of Lua gets you your own.', ['/plugins']],
    [/update|upgrade|version|release/i,           'Self-update is built in, with SHA-256 verified release tarballs.', ['/docs updating', '/changelog']],
    [/tui|dashboard|interface|screen/i,           'The dashboard is five panes over your machine\'s state.', ['/tui']],
    [/host|server|remote|ssh/i,                   'dots tracks remote hosts and services. View them in the hosts table.', ['/hosts']],
    [/window|wsl/i,                               'dots targets macOS and Linux. WSL works the way any Linux does; native Windows is not supported.', ['/docs requirements']],
    [/rust|cargo|build|source|binary/i,           'Rust, a two-crate Cargo workspace, shipped as one static binary.', ['/about', '/docs dev']],
    [/config|nvim|ghostty|opencode|premade/i,     'Configs are discovered from your dotfiles repo; a few starters ship inside the binary.', ['/docs configs']],
    [/profile|machine|migrat|new laptop|backup/i, 'Export your setup to `personal.json`, import it on the next machine.', ['/docs profiles']],
    [/licen|free|cost|price/i,                    'MIT licensed, free, and the source is right there.', ['/about', '/status']],
    [/who|author|made|built by/i,                 'Built by CtrlUserKnown — the repo has the full history.', ['/github', '/changelog']],
  ];

  function fallbackBlocks(text) {
    for (var i = 0; i < FALLBACKS.length; i++) {
      if (FALLBACKS[i][0].test(text)) {
        return [
          { text: FALLBACKS[i][1] },
          { cmds: FALLBACKS[i][2].map(descFor) },
        ];
      }
    }
    return [
      { text: 'Not a command I know — but `' + text.split(/\s+/)[0] + '` isn\'t far off. Try one of these:' },
      { cmds: ['/about', '/install', '/features', '/docs'].map(descFor) },
    ];
  }

  function descFor(name) {
    var base = name.split(' ')[0];
    var cmd  = D.COMMANDS.filter(function (c) { return c.name === base; })[0];
    var topic = name.split(' ')[1];
    if (topic) {
      var t = D.TOPIC_LIST.filter(function (x) { return x[0] === topic; })[0];
      if (t) return [name, t[1]];
    }
    return [name, cmd ? cmd.desc : ''];
  }

  // ── dispatch ────────────────────────────────────────────────────────────────

  function resolve(raw) {
    var line = raw.trim();

    if (line.charAt(0) === '/') {
      var name = line.split(/\s+/)[0].toLowerCase();
      var arg  = line.slice(name.length).trim();
      var cmd  = D.COMMANDS.filter(function (c) { return c.name === name; })[0];

      if (!cmd) {
        var near = D.COMMANDS.filter(function (c) { return c.name.indexOf(name) === 0; });
        return [
          { err: 'Unknown command `' + name + '`.' },
          { cmds: (near.length ? near : D.COMMANDS).map(function (c) { return [c.name, c.desc]; }) },
        ];
      }
      if (cmd.name === '/help')  return helpBlocks();
      if (cmd.name === '/keys')  return keysBlocks();
      if (cmd.name === '/clear') { clear(); return []; }
      return cmd.run(arg);
    }

    if (/^(clear|reset)$/i.test(line)) { clear(); return []; }
    if (/^(exit|quit|logout|:q)$/i.test(line)) {
      return [{ text: 'Nothing to quit — this session lives in a browser tab. The real one quits with `q`.' }];
    }
    if (/^(help|\?)$/i.test(line)) return helpBlocks();

    for (var i = 0; i < D.SHELL.length; i++) {
      var m = line.match(D.SHELL[i].re);
      if (m) return D.SHELL[i].run(m);
    }
    return fallbackBlocks(line);
  }

  async function think() {
    var el = add('<span class="spark">✻</span> <span class="verb">' +
                 VERBS[Math.floor(Math.random() * VERBS.length)] + '…</span> ' +
                 '<span class="dim">(esc to interrupt)</span>', 'blk thinking');

    var spark = el.querySelector('.spark');
    var i = 0;
    var timer = setInterval(function () {
      spark.textContent = SPARKS[++i % SPARKS.length];
    }, 110);

    await wait(240 + Math.random() * 260);
    clearInterval(timer);
    el.remove();
  }

  async function turn(raw, opts) {
    busy = true;
    run  = { cancelled: false };
    input.setAttribute('placeholder', '');

    await think();

    try {
      var blocks = await Promise.resolve((opts && opts.blocks) || resolve(raw));
      for (var i = 0; i < blocks.length; i++) {
        await renderBlock(blocks[i]);
        if (run.cancelled) break;
      }
      if (run.cancelled) add('<span class="c-red">⏹</span> <span class="dim">Interrupted by user</span>', 'blk fade');
    } catch (e) {
      add('<span class="c-red">⏺</span> Something broke rendering that. ' +
          '<a class="cmdlink" data-cmd="/help">/help</a>', 'blk fade');
    }

    busy = false;
    run  = null;
    tickContext();
    input.setAttribute('placeholder', 'Ask about dots, or type / for commands');
    scroll();
  }

  function echoUser(text) {
    add('<span class="caret">&gt;</span> <span>' + D.escapeHtml(text) + '</span>', 'blk user');
  }

  async function submit(raw, opts) {
    if (busy) return;
    var line = raw.trim();
    if (!line) return;

    history.push(line);
    histIdx = -1;
    input.value = '';
    closePopup();
    echoUser(line);
    await turn(line, opts);
  }

  function tickContext() {
    ctxLeft = Math.max(14, ctxLeft - (1 + Math.floor(Math.random() * 2)));
    ctxEl.textContent = 'Context left: ' + ctxLeft + '%';
    if (ctxLeft <= 20) ctxEl.className = 'c-yellow';
  }

  // ── slash popup ─────────────────────────────────────────────────────────────

  function openPopup(list) {
    matches = list;
    sel = Math.min(sel, list.length - 1);
    if (sel < 0) sel = 0;

    popup.innerHTML = list.map(function (c, i) {
      return '<div class="pop-row' + (i === sel ? ' on' : '') + '" data-i="' + i + '">' +
               '<span class="pop-name">' + c.name + (c.arg ? ' <span class="dim">' + c.arg + '</span>' : '') + '</span>' +
               '<span class="pop-desc">' + c.desc + '</span></div>';
    }).join('');
    popup.hidden = false;
  }

  function closePopup() {
    popup.hidden = true;
    matches = [];
    sel = 0;
  }

  function refreshPopup() {
    var v = input.value;
    if (v.charAt(0) !== '/' || /\s/.test(v)) return closePopup();

    var list = D.COMMANDS.filter(function (c) { return c.name.indexOf(v.toLowerCase()) === 0; });
    if (!list.length) return closePopup();
    openPopup(list);
  }

  function accept(runIt) {
    var c = matches[sel];
    if (!c) return;
    input.value = c.name + (c.arg ? ' ' : '');
    closePopup();
    if (runIt && !c.arg) submit(input.value);
    input.focus();
  }

  // ── input handling ──────────────────────────────────────────────────────────

  input.addEventListener('input', refreshPopup);

  input.addEventListener('keydown', function (e) {
    var open = !popup.hidden;

    if (e.key === 'Enter') {
      e.preventDefault();
      if (open) return accept(true);
      return submit(input.value);
    }

    if (e.key === 'Tab' && !e.shiftKey) {
      e.preventDefault();
      if (open) return accept(false);
      if (input.value.charAt(0) === '/') { refreshPopup(); if (!popup.hidden) accept(false); }
      return;
    }

    if (e.key === 'Tab' && e.shiftKey) {
      e.preventDefault();
      return cycleMode();
    }

    if (e.key === 'Escape') {
      if (open) return closePopup();
      if (busy && run) { run.cancelled = true; return; }
      input.value = '';
      return;
    }

    if (e.key === 'ArrowUp') {
      e.preventDefault();
      if (open) { sel = (sel - 1 + matches.length) % matches.length; return openPopup(matches); }
      return histNav(-1);
    }

    if (e.key === 'ArrowDown') {
      e.preventDefault();
      if (open) { sel = (sel + 1) % matches.length; return openPopup(matches); }
      return histNav(1);
    }

    if (e.key === 'l' && e.ctrlKey) { e.preventDefault(); return clear(); }
    if (e.key === 'r' && e.ctrlKey) { e.preventDefault(); return expandLast(); }
    if (e.key === 'c' && e.ctrlKey && input.value) { e.preventDefault(); input.value = ''; return; }

    if (e.key === '?' && !input.value) {
      e.preventDefault();
      return submit('/keys');
    }
  });

  function histNav(dir) {
    if (!history.length) return;
    if (histIdx === -1) {
      if (dir > 0) return;
      draft = input.value;
      histIdx = history.length;
    }
    histIdx = Math.min(history.length, Math.max(0, histIdx + dir));
    input.value = histIdx >= history.length ? draft : history[histIdx];
    if (histIdx >= history.length) histIdx = -1;
    input.setSelectionRange(input.value.length, input.value.length);
  }

  function cycleMode() {
    mode = (mode + 1) % MODES.length;
    modeEl.textContent = MODES[mode].label;
    modeEl.className = MODES[mode].cls;
  }

  function expandLast() {
    if (!lastOut) return;
    var more = lastOut.querySelector('.more');
    var rest = lastOut.querySelector('.hidden-lines');
    if (!rest) return;
    rest.hidden = false;
    if (more) more.remove();
    lastOut = null;
    scroll();
  }

  function copy(text) {
    if (navigator.clipboard) return navigator.clipboard.writeText(text);
    var el = document.createElement('textarea');
    el.value = text;
    el.style.position = 'fixed';
    el.style.opacity = '0';
    document.body.appendChild(el);
    el.select();
    try { document.execCommand('copy'); } catch (err) {}
    document.body.removeChild(el);
  }

  // ── session chrome ──────────────────────────────────────────────────────────

  function banner() {
    add(
      '<div class="welcome">' +
        '<div class="w-title"><span class="spark c-mauve">✻</span> Welcome to <strong>dots</strong></div>' +
        '<div class="w-sub">cross-platform dotfiles manager · v' + D.VERSION + ' · noir-cat</div>' +
        '<div class="w-path dim">~/.dots</div>' +
      '</div>' +
      '<div class="tips dim">' +
        '<div>Type <code>/</code> for commands, or ask a question.</div>' +
        '<div>Try <a class="cmdlink" data-cmd="/about">/about</a>, ' +
             '<a class="cmdlink" data-cmd="/hosts">/hosts</a>, or ' +
             '<a class="cmdlink" data-cmd="/install">/install</a>.</div>' +
      '</div>', 'blk');
  }

  function clear() {
    stream.innerHTML = '';
    lastOut = null;
    banner();
  }

  // Type a line into the prompt as if someone were sitting here, then run it.
  async function autoType(text, opts) {
    for (var i = 0; i < text.length; i++) {
      input.value += text.charAt(i);
      await wait(42 + Math.random() * 40);
    }
    await wait(300);
    await submit(text, opts);
  }

  var INTRO = [
    { tool: 'Bash', args: 'dots --version', out: ['dots v' + D.VERSION], collapse: 0 },
    { text: '**dots** is a fast, cross-platform dotfiles manager with an interactive TUI, written in Rust. It installs your tools, wires up symlinks GNU Stow–style, applies app configs, and keeps everything healthy — on macOS and Linux alike.' },
    { code: D.INSTALL, copy: true },
    { cmds: [
      ['/about',     'why it exists, and how it\'s built'],
      ['/install',   'the install, step by step'],
      ['/features',  'everything it does'],
      ['/tui',       'the dashboard you get'],
      ['/docs',      'documentation by topic'],
      ['/changelog', 'release history'],
    ] },
    { note: 'Everything here works like a terminal — history, tab-completion, `ctrl+l`. Press `?` for shortcuts.' },
  ];

  // The landing turn: type the question for the visitor, then answer it with
  // fixed content instead of routing through resolve().
  async function intro() {
    banner();
    await wait(500);
    await autoType('what is dots?', { blocks: INTRO });
  }

  // ── delegated clicks ────────────────────────────────────────────────────────

  document.addEventListener('click', function (e) {
    var cmd = e.target.closest('.cmdlink');
    if (cmd) {
      e.preventDefault();
      var text = cmd.getAttribute('data-cmd');
      input.value = '';
      submit(text);
      return;
    }

    var more = e.target.closest('.more');
    if (more) {
      var rest = more.parentNode.querySelector('.hidden-lines');
      if (rest) { rest.hidden = false; more.remove(); scroll(); }
      return;
    }

    var row = e.target.closest('.pop-row');
    if (row) {
      sel = +row.getAttribute('data-i');
      return accept(true);
    }

    // Clicking anywhere in the page puts you back on the prompt, the way a
    // focused terminal behaves — unless you're selecting text or hit a link.
    if (e.target.closest('a, button')) return;
    if (window.getSelection().toString()) return;
    input.focus();
  });

  chips.addEventListener('click', function (e) {
    var chip = e.target.closest('button');
    if (chip) submit(chip.getAttribute('data-cmd'));
  });

  document.addEventListener('keydown', function (e) {
    if (e.target === input) return;
    if (e.metaKey || e.ctrlKey || e.altKey) return;
    if (e.key.length === 1 || e.key === 'Backspace') input.focus();
  });

  // ── go ──────────────────────────────────────────────────────────────────────

  modeEl.textContent = MODES[0].label;
  modeEl.className   = MODES[0].cls;
  ctxEl.textContent  = 'Context left: ' + ctxLeft + '%';

  chips.innerHTML = [
    ['/install', 'install'], ['/features', 'features'], ['/hosts', 'hosts'],
    ['/docs', 'docs'], ['/changelog', 'changelog'], ['/github', 'github'],
  ].map(function (c) {
    return '<button type="button" data-cmd="' + c[0] + '">' + c[1] + '</button>';
  }).join('');

  intro();

})();
