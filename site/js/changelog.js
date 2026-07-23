// Mirrored into the site at deploy time from the canonical root CHANGELOG.md.
var CHANGELOG_URL = 'CHANGELOG.md';

var SECTION_BADGE = {
  Added:   'badge-green',
  Changed: 'badge-blue',
  Fixed:   'badge-yellow',
  Removed: 'badge-red',
};

function parseChangelog(text) {
  var entries = [];
  var current = null;
  var section  = null;

  text.split('\n').forEach(function (line) {
    var vMatch = line.match(/^## \[(.+?)\] - (.+)/);
    if (vMatch) {
      if (current) entries.push(current);
      current = { version: vMatch[1], date: vMatch[2], sections: [] };
      section = null;
      return;
    }

    if (!current) return;

    var sMatch = line.match(/^### (.+)/);
    if (sMatch) {
      section = { name: sMatch[1], items: [] };
      current.sections.push(section);
      return;
    }

    if (section && line.startsWith('- ')) {
      section.items.push(line.slice(2).trim());
    }
  });

  if (current) entries.push(current);
  return entries;
}

function escapeHtml(str) {
  return str
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;');
}

function renderInline(text) {
  return escapeHtml(text)
    .replace(/\*\*(.+?)\*\*/g, '<strong>$1</strong>')
    .replace(/`([^`]+)`/g, '<code>$1</code>')
    .replace(/\[([^\]]+)\]\(([^)]+)\)/g, '<a href="$2" target="_blank" rel="noopener noreferrer">$1</a>');
}

function buildHtml(entries) {
  return entries.map(function (entry) {
    var sectionsHtml = entry.sections.map(function (s) {
      var cls = SECTION_BADGE[s.name] || 'badge-purple';
      var items = s.items.map(function (item) {
        return '<li>' + renderInline(item) + '</li>';
      }).join('');
      return '<span class="cl-section-label"><span class="badge ' + cls + '">' +
        escapeHtml(s.name) + '</span></span>' +
        '<ul class="cl-items">' + items + '</ul>';
    }).join('');

    return '<div class="cl-entry">' +
      '<div class="cl-header">' +
        '<span class="cl-version">v' + escapeHtml(entry.version) + '</span>' +
        '<span class="cl-date">' + escapeHtml(entry.date) + '</span>' +
      '</div>' +
      sectionsHtml +
    '</div>';
  }).join('');
}

function loadChangelog() {
  var container = document.getElementById('changelog-container');
  if (!container) return;

  fetch(CHANGELOG_URL)
    .then(function (res) {
      if (!res.ok) throw new Error('HTTP ' + res.status);
      return res.text();
    })
    .then(function (text) {
      var entries = parseChangelog(text);
      if (!entries.length) {
        container.innerHTML = '<p class="text-muted">No entries found.</p>';
        return;
      }
      container.innerHTML = buildHtml(entries);
    })
    .catch(function () {
      container.innerHTML =
        '<div class="cl-error">' +
          '<p>Could not load the changelog. ' +
          '<a href="https://github.com/CtrlUserKnown/dots/blob/main/CHANGELOG" ' +
          'target="_blank" rel="noopener noreferrer">View it on GitHub.</a></p>' +
        '</div>';
    });
}

document.addEventListener('DOMContentLoaded', loadChangelog);
