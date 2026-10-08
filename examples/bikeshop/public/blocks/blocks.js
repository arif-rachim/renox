// Bike shop blocks: the behaviour of resources/views/blocks/date_picker_blocked.html
// (see /about/blocks). The other blocks moved to the renox-blocks crate (#347),
// which loads its own script. Plain DOM, set up from data-bs-* attributes on the
// page and in whatever htmx swaps in (htmx:load), so no inline handlers: it works
// under CSP=strict.
(function () {
  "use strict";

  var ISO = /^\d{4}-\d{2}-\d{2}$/;

  /** Runs `init` once for each element matching `selector` in `root`. */
  function each(root, selector, init) {
    if (root.matches && root.matches(selector)) init(root);
    root.querySelectorAll(selector).forEach(init);
  }

  // ---------- date_picker_blocked ----------

  function setupBlocked(box) {
    if (box.hasAttribute("data-bs-ready")) return;
    box.setAttribute("data-bs-ready", "");
    var dates = {};
    var closed = [];
    try {
      JSON.parse(box.getAttribute("data-bs-blocked") || "[]").forEach(function (d) {
        dates[d] = true;
      });
      closed = JSON.parse(box.getAttribute("data-bs-closed") || "[]");
    } catch (_) {}
    function blocked(iso) {
      return !!dates[iso] || closed.indexOf(new Date(iso + "T00:00:00Z").getUTCDay()) !== -1;
    }
    var calendar = box.querySelector("calendar-date");
    if (calendar && window.customElements) {
      // Cally's days are UTC dates.
      customElements.whenDefined("calendar-date").then(function () {
        calendar.isDateDisallowed = function (date) {
          return blocked(date.toISOString().slice(0, 10));
        };
      });
    }
    var input = box.querySelector("[data-bs-blocked-input]");
    var slot = box.querySelector(".rx-error");
    var message = box.getAttribute("data-bs-message") || "";
    var ours = false;
    function check() {
      var value = input.value.trim();
      if (ISO.test(value) && blocked(value)) {
        input.setCustomValidity(message);
        input.setAttribute("aria-invalid", "true");
        if (slot) slot.textContent = message;
        ours = true;
      } else if (ours) {
        input.setCustomValidity("");
        input.removeAttribute("aria-invalid");
        if (slot) slot.textContent = "";
        ours = false;
      }
    }
    if (input) {
      input.addEventListener("input", check);
      input.addEventListener("change", check);
      check();
    }
  }

  // ---------- Setting up ----------

  function setup(root) {
    root = root && root.querySelectorAll ? root : document;
    each(root, "[data-bs-blocked]", setupBlocked);
  }

  window.BikeshopBlocks = { setup: setup };
  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", function () {
      setup(document);
    });
  } else {
    setup(document);
  }
  document.addEventListener("htmx:load", function (event) {
    setup(event.target);
  });
})();
