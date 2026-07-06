(function () {
  var sidebarLinks = document.querySelectorAll('.sidebar-nav a[href^="#"]');
  var sections = [];

  sidebarLinks.forEach(function (a) {
    var id = a.getAttribute('href').slice(1);
    var el = document.getElementById(id);
    if (el) sections.push({ id: id, el: el, link: a });
  });

  if (!sections.length) return;

  var navHeight = parseInt(
    getComputedStyle(document.documentElement)
      .getPropertyValue('--nav-height') || '60',
    10
  );
  var offset = navHeight + 40;

  function onScroll() {
    var scrollY = window.scrollY;
    var active = sections[0];

    sections.forEach(function (s) {
      if (scrollY >= s.el.offsetTop - offset) active = s;
    });

    sidebarLinks.forEach(function (a) { a.classList.remove('active'); });
    if (active) active.link.classList.add('active');
  }

  window.addEventListener('scroll', onScroll, { passive: true });
  onScroll();
})();
