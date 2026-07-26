(function () {
  var tocLinks = document.querySelectorAll('.docs-toc a[href^="#"]');
  var sections = [];

  tocLinks.forEach(function (a) {
    var id = a.getAttribute('href').slice(1);
    var el = document.getElementById(id);
    if (el) sections.push({ id: id, el: el, link: a });
  });

  if (!sections.length) return;

  function onScroll() {
    var scrollY = window.scrollY;
    var active = sections[0];
    var offset = 80;

    sections.forEach(function (s) {
      if (scrollY >= s.el.offsetTop - offset) active = s;
    });

    tocLinks.forEach(function (a) { a.classList.remove('active'); });
    if (active) active.link.classList.add('active');
  }

  window.addEventListener('scroll', onScroll, { passive: true });
  onScroll();
})();
