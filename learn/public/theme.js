// The light/dark choice: stored in localStorage, applied as data-theme on
// <html> before the first paint (this script is not deferred). Without
// JavaScript the toggle stays hidden and the page follows the system.
(function () {
  var KEY = 'renox-learn:theme';
  var root = document.documentElement;
  try {
    var saved = localStorage.getItem(KEY);
    if (saved === 'light' || saved === 'dark') root.dataset.theme = saved;
  } catch (e) {
    // Storage blocked: follow the system.
  }
  function dark() {
    if (root.dataset.theme) return root.dataset.theme === 'dark';
    return !!(window.matchMedia && window.matchMedia('(prefers-color-scheme: dark)').matches);
  }
  function show(button) {
    var isDark = dark();
    button.setAttribute('aria-pressed', isDark ? 'true' : 'false');
    button.textContent = isDark ? 'Light' : 'Dark';
  }
  document.addEventListener('DOMContentLoaded', function () {
    var buttons = document.querySelectorAll('[data-theme-toggle]');
    buttons.forEach(function (button) {
      button.hidden = false;
      show(button);
      button.addEventListener('click', function () {
        var next = dark() ? 'light' : 'dark';
        root.dataset.theme = next;
        try {
          localStorage.setItem(KEY, next);
        } catch (e) {
          // The choice stays for this page only.
        }
        buttons.forEach(show);
      });
    });
  });
})();
