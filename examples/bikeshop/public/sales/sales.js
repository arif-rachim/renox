// The sales pages' behaviour (#234), on top of htmx and the kit; each part
// is an improvement on a page that works without it. Motion (motion.dev,
// loaded by the layout) moves with `transform`; nothing moves under
// prefers-reduced-motion. No inline handlers, so CSP=strict works.
(function () {
  "use strict";
  if (window.BikeshopSales) return;
  var reduce = window.matchMedia("(prefers-reduced-motion: reduce)");

  // The invoice's "Print" button (and its R shortcut).
  document.addEventListener("click", function (event) {
    if (event.target.closest && event.target.closest("[data-bs-print]")) window.print();
  });

  // The checkout wizard: the step that opens slides in.
  document.addEventListener("click", function (event) {
    var nav = event.target.closest && event.target.closest("[data-rx-wizard-next], [data-rx-wizard-back]");
    if (!nav || !window.Motion || reduce.matches) return;
    var wizard = nav.closest("[data-rx-wizard]");
    var back = nav.hasAttribute("data-rx-wizard-back");
    requestAnimationFrame(function () {
      var open = wizard && Array.prototype.find.call(wizard.querySelectorAll("[data-rx-step]"), function (p) {
        return p.offsetParent !== null;
      });
      if (open) {
        Motion.animate(open, { opacity: [0, 1], transform: [back ? "translateX(-16px)" : "translateX(16px)", "translateX(0)"] },
          { duration: 0.28, ease: [0.22, 1, 0.36, 1] });
      }
    });
  });

  // The cart's lines and the checkout's summary come in gently after a change.
  document.addEventListener("htmx:afterSwap", function (event) {
    var target = event.detail && event.detail.target;
    if (!target || !window.Motion || reduce.matches) return;
    if (target.id === "summary" || target.id === "cart") {
      Motion.animate(target, { opacity: [0.6, 1], transform: ["scale(0.995)", "scale(1)"] }, { duration: 0.2 });
    }
  });

  // The counter: the change due, as the amount received is typed (or keyed in).
  function showChange(input) {
    var form = input.closest("form");
    var total = document.querySelector("[data-bs-counter-total]");
    var hint = form && form.querySelector("#rx-tendered-hint");
    if (!total || !hint) return;
    if (!hint.dataset.bsText) hint.dataset.bsText = hint.textContent;
    var given = parseInt((input.value || "").replace(/[^0-9]/g, ""), 10);
    var due = parseInt(total.dataset.bsCounterTotal, 10);
    if (isNaN(given)) { hint.textContent = hint.dataset.bsText; return; }
    var change = given - due;
    var money = total.textContent.replace(/[0-9.,\s]+/g, " ").trim();
    hint.textContent = change >= 0
      ? (form.dataset.bsChangeLabel || "Change") + ": " + money + " " + change.toLocaleString(document.documentElement.lang)
      : (form.dataset.bsShortLabel || "Short") + ": " + money + " " + (-change).toLocaleString(document.documentElement.lang);
  }
  document.addEventListener("input", function (event) {
    if (event.target && event.target.id === "rx-tendered") showChange(event.target);
  });

  window.BikeshopSales = { showChange: showChange };
})();
