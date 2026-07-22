(function () {
  var lines = [
    { type: 'input', text: '$ dots' },
    { type: 'blank' },
    { type: 'header', text: '┌─────────────────────────────────────────────────────┐' },
    { type: 'header', text: '│  ⠋ dots — dotfiles manager                          │' },
    { type: 'header', text: '├─────────────────────────────────────────────────────┤' },
    { type: 'ok',    text: '│  ● symlinks    12 linked   0 broken                 │' },
    { type: 'ok',    text: '│  ● tools       8 installed all up to date           │' },
    { type: 'ok',    text: '│  ● plugins     3 active                            │' },
    { type: 'warn',  text: '│  ○ updates     1 available  (eza → 1.2.0)          │' },
    { type: 'ok',    text: '│  ● configs     synced                              │' },
    { type: 'ok',    text: '│  ● network     online  12ms  Wi-Fi                  │' },
    { type: 'header', text: '├─────────────────────────────────────────────────────┤' },
    { type: 'muted', text: '│  press [q] quit  [↑↓] navigate  [enter] select      │' },
    { type: 'header', text: '└─────────────────────────────────────────────────────┘' },
    { type: 'blank' },
    { type: 'input', text: '$ dots install eza' },
    { type: 'ok',    text: '✓ resolved  eza@1.2.0 (linux-amd64)' },
    { type: 'ok',    text: '✓ downloaded  2.1 MB' },
    { type: 'ok',    text: '✓ installed   /usr/local/bin/eza' },
    { type: 'blank' },
    { type: 'input', text: '$ dots link' },
    { type: 'ok',    text: '✓ linked  ghostty/config → ~/.config/ghostty/config' },
    { type: 'ok',    text: '✓ linked  nvim/           → ~/.config/nvim/' },
    { type: 'ok',    text: '✓ linked  opencode/       → ~/.config/opencode/' },
    { type: 'muted', text: '  12 symlinks OK, 0 skipped, 0 errors' },
  ];

  var classMap = {
    input: 'demo-line--input',
    ok:    'demo-line--ok',
    warn:  'demo-line--warn',
    muted: 'demo-line--muted',
    header:'demo-line--header',
    blank: '',
  };

  function init() {
    var container = document.getElementById('demo-output');
    if (!container) return;

    var idx = 0;
    function addLine() {
      if (idx >= lines.length) {
        setTimeout(function () {
          container.innerHTML = '';
          idx = 0;
          addLine();
        }, 2400);
        return;
      }

      var line = lines[idx];
      var el = document.createElement('div');
      el.className = 'demo-line ' + (classMap[line.type] || '');

      if (line.type === 'blank') {
        el.innerHTML = '&nbsp;';
      } else {
        el.textContent = line.text;
      }

      el.style.opacity = '0';
      container.appendChild(el);

      requestAnimationFrame(function () {
        el.style.opacity = '1';
      });

      idx++;
      var delay = line.type === 'blank' ? 100 : line.type === 'input' ? 420 : 160;
      setTimeout(addLine, delay);
    }

    addLine();
  }

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', init);
  } else {
    init();
  }
})();
