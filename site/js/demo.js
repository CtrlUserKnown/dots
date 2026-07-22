(function () {
  var lines = [
    { type: 'full', html: '<span class="tui-title"> dots </span> <span class="tui-subtitle">dotfiles manager</span> <span class="tui-float">q: quit  ?: help  1-6: panes</span>' },
    { type: 'full', html: '<span class="tui-border">─────────────────────────── symlinks ───────────────────────────</span>' },
    { type: 'full', html: '  <span class="tui-ok">●</span> <span class="tui-label">ghostty</span>    <span class="tui-path">~/.config/ghostty/config</span>' },
    { type: 'full', html: '  <span class="tui-ok">●</span> <span class="tui-label">nvim</span>       <span class="tui-path">~/.config/nvim/</span>' },
    { type: 'full', html: '  <span class="tui-ok">●</span> <span class="tui-label">opencode</span>   <span class="tui-path">~/.config/opencode/</span>' },
    { type: 'full', html: '  <span class="tui-ok">●</span> <span class="tui-label">zsh</span>        <span class="tui-path">~/.zshrc</span>' },
    { type: 'full', html: '  <span class="tui-ok">●</span> <span class="tui-label">tmux</span>       <span class="tui-path">~/.tmux.conf</span>' },
    { type: 'full', html: '  <span class="tui-ok">●</span> <span class="tui-label">git</span>        <span class="tui-path">~/.gitconfig</span>' },
    { type: 'full', html: '<span class="tui-border">──────────────────────────── tools ────────────────────────────</span>' },
    { type: 'full', html: '  <span class="tui-ok">●</span> <span class="tui-label">eza</span>        <span class="tui-ver">v1.1.0</span>    <span class="tui-status-ok">installed</span>' },
    { type: 'full', html: '  <span class="tui-ok">●</span> <span class="tui-label">fzf</span>        <span class="tui-ver">v0.53.0</span>   <span class="tui-status-ok">installed</span>' },
    { type: 'full', html: '  <span class="tui-ok">●</span> <span class="tui-label">bat</span>        <span class="tui-ver">v0.24.0</span>   <span class="tui-status-ok">installed</span>' },
    { type: 'full', html: '  <span class="tui-ok">●</span> <span class="tui-label">zoxide</span>     <span class="tui-ver">v0.9.4</span>    <span class="tui-status-ok">installed</span>' },
    { type: 'full', html: '  <span class="tui-ok">●</span> <span class="tui-label">ripgrep</span>    <span class="tui-ver">v14.1.0</span>   <span class="tui-status-ok">installed</span>' },
    { type: 'full', html: '<span class="tui-border">──────────────────────────── plugins ───────────────────────────</span>' },
    { type: 'full', html: '  <span class="tui-ok">●</span> <span class="tui-label">lazy.nvim</span>           <span class="tui-status-ok">active</span>' },
    { type: 'full', html: '  <span class="tui-ok">●</span> <span class="tui-label">telescope.nvim</span>      <span class="tui-status-ok">active</span>' },
    { type: 'full', html: '  <span class="tui-ok">●</span> <span class="tui-label">treesitter</span>          <span class="tui-status-ok">active</span>' },
    { type: 'full', html: '<span class="tui-border">──────────────────────────── updates ───────────────────────────</span>' },
    { type: 'full', html: '  <span class="tui-warn">○</span> <span class="tui-label">eza</span>        <span class="tui-ver">v1.1.0 → v1.2.0</span>  <span class="tui-status-warn">update available</span>' },
    { type: 'full', html: '  <span class="tui-ok">●</span> <span class="tui-label">fzf</span>        <span class="tui-ver">v0.53.0</span>           <span class="tui-status-ok">up to date</span>' },
    { type: 'full', html: '  <span class="tui-ok">●</span> <span class="tui-label">bat</span>        <span class="tui-ver">v0.24.0</span>           <span class="tui-status-ok">up to date</span>' },
    { type: 'full', html: '<span class="tui-border">──────────────────────────── network ───────────────────────────</span>' },
    { type: 'full', html: '  <span class="tui-ok">●</span> <span class="tui-label">status</span>     <span class="tui-status-ok">online</span>    <span class="tui-ver">12ms</span>    <span class="tui-label">Wi-Fi</span>' },
    { type: 'full', html: '  <span class="tui-ok">●</span> <span class="tui-label">dns</span>        <span class="tui-path">1.1.1.1, 8.8.8.8</span>' },
    { type: 'full', html: '  <span class="tui-ok">●</span> <span class="tui-label">vpn</span>        <span class="tui-status-ok">not connected</span>' },
    { type: 'full', html: '<span class="tui-border">─────────────────────────────────────────────────────────────────</span>' },
    { type: 'full', html: '<span class="tui-footer"> <span class="tui-key">↑↓</span> navigate  <span class="tui-key">enter</span> select  <span class="tui-key">i</span> install  <span class="tui-key">l</span> link  <span class="tui-key">u</span> update</span>' },
  ];

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
        }, 3000);
        return;
      }

      var line = lines[idx];
      var el = document.createElement('div');
      el.className = 'demo-line tui-line';
      el.innerHTML = line.html;
      el.style.opacity = '0';
      container.appendChild(el);

      requestAnimationFrame(function () {
        el.style.opacity = '1';
      });

      idx++;
      var delay = line.type === 'full' ? 110 : 100;
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
