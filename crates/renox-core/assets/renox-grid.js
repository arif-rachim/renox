// Renox data grid (renox/grid.html): columns per screen size, frozen
// columns, the column menu, popovers, the date range calendar, and the
// form that asks the server for filtered, sorted pages. Plain DOM, no
// inline handlers (works under a strict CSP), keyboard-usable.
(function () {
  "use strict";
  if (window.RenoxGrid) return;

  var compact = window.matchMedia("(max-width: 767px)");
  var reopen = null; // the column menu to open again after a reload

  function csrf() {
    var meta = document.querySelector('meta[name="csrf-token"]');
    return meta ? meta.content : "";
  }

  function config(grid) {
    if (!grid._rx) {
      try { grid._rx = JSON.parse(grid.getAttribute("data-rx-grid")); } catch (_) { grid._rx = {}; }
    }
    return grid._rx;
  }

  function screen() { return compact.matches ? "compact" : "wide"; }

  function shownSet(grid) {
    var cfg = config(grid);
    return new Set(cfg[screen()] || cfg.order || []);
  }

  function cellsOf(grid, key) {
    return grid.querySelectorAll('.rx-grid__table [data-col="' + CSS.escape(key) + '"]');
  }

  // Shows the columns picked for this screen size, fixes the grouped
  // headings' spans, then places the frozen columns.
  function applyVisibility(grid) {
    var shown = shownSet(grid);
    grid.querySelectorAll(".rx-grid__table [data-col]").forEach(function (cell) {
      cell.classList.toggle("rx-grid__off", !shown.has(cell.getAttribute("data-col")));
    });
    grid.querySelectorAll(".rx-grid__table [data-cols]").forEach(function (th) {
      var n = th.getAttribute("data-cols").split(" ").filter(function (k) { return shown.has(k); }).length;
      th.colSpan = Math.max(n, 1);
      th.classList.toggle("rx-grid__off", n === 0);
    });
    var span = grid.querySelector("[data-grid-span]");
    if (span) span.colSpan = Math.max(shown.size, 1);
    layoutPins(grid);
  }

  // Frozen columns stick at an offset: the widths of the frozen columns
  // before them.
  function layoutPins(grid) {
    var cfg = config(grid);
    var shown = shownSet(grid);
    grid.querySelectorAll(".rx-grid__edge-left, .rx-grid__edge-right").forEach(function (el) {
      el.classList.remove("rx-grid__edge-left", "rx-grid__edge-right");
    });
    var offsets = {};
    function width(key) {
      var th = grid.querySelector('thead th[data-col="' + CSS.escape(key) + '"]');
      return th ? th.getBoundingClientRect().width : 0;
    }
    function place(keys, side) {
      var at = 0, last = null;
      keys.forEach(function (key) {
        if (!shown.has(key)) return;
        offsets[key] = at;
        cellsOf(grid, key).forEach(function (cell) {
          cell.style[side] = at + "px";
          cell.style[side === "left" ? "right" : "left"] = "";
        });
        at += width(key);
        last = key;
      });
      if (last) cellsOf(grid, last).forEach(function (cell) { cell.classList.add("rx-grid__edge-" + side); });
    }
    place(cfg.left || [], "left");
    place((cfg.right || []).slice().reverse(), "right");
    grid.querySelectorAll(".rx-grid__table th[data-cols][data-pin]").forEach(function (th) {
      var side = th.getAttribute("data-pin");
      var keys = th.getAttribute("data-cols").split(" ").filter(function (k) { return k in offsets; });
      if (!keys.length) return;
      th.style[side] = Math.min.apply(null, keys.map(function (k) { return offsets[k]; })) + "px";
    });
    edges(grid);
  }

  // Shadows on the frozen edges once rows scroll under them.
  function edges(grid) {
    var scroll = grid.querySelector(".rx-grid__scroll");
    if (!scroll) return;
    var max = scroll.scrollWidth - scroll.clientWidth;
    var x = Math.abs(scroll.scrollLeft);
    grid.classList.toggle("rx-grid--scrolled-left", x > 1);
    grid.classList.toggle("rx-grid--scrolled-right", x < max - 1);
  }

  // ---------- The form: sort, pages, filters ----------

  function setState(grid, name, value) {
    var input = grid.querySelector('[data-grid-state="' + name + '"]');
    if (input) input.value = value;
  }

  function submit(grid, keepPage) {
    if (!keepPage) setState(grid, "page", "1");
    if (grid.requestSubmit) grid.requestSubmit(); else grid.submit();
  }

  // Empty fields and defaults stay out of the URL.
  function trim(grid) {
    var disabled = [];
    grid.querySelectorAll("[name]").forEach(function (el) {
      if (el.disabled) return;
      var name = el.name, drop = false;
      if ((el.type === "checkbox" || el.type === "radio") && !el.checked) return;
      if (el.value === "") drop = true;
      if (name === "page" && el.value === "1") drop = true;
      if (name.indexOf("m.") === 0) {
        var text = grid.querySelector('[name="q.' + CSS.escape(name.slice(2)) + '"]');
        if (!text || !text.value.trim() || el.value === "contains") drop = true;
      }
      if (drop) { el.disabled = true; disabled.push(el); }
    });
    grid._rxDisabled = disabled;
  }

  function untrim(grid) {
    (grid._rxDisabled || []).forEach(function (el) { el.disabled = false; });
    grid._rxDisabled = [];
  }

  document.addEventListener("submit", function (event) {
    var grid = event.target.closest && event.target.closest("form.rx-grid");
    if (!grid || event.target !== grid) return;
    if (!grid._rxKeepPage) setState(grid, "page", "1");
    grid._rxKeepPage = false;
    trim(grid);
    // Without htmx the browser submits the form itself; give the fields back after.
    if (!window.htmx) setTimeout(function () { untrim(grid); }, 0);
  }, true);

  document.addEventListener("htmx:afterRequest", function (event) {
    var grid = event.detail.elt;
    if (grid && grid.classList && grid.classList.contains("rx-grid")) untrim(grid);
  });

  document.addEventListener("click", function (event) {
    var target = event.target.closest && event.target.closest("[data-grid-sort], [data-grid-page], [data-grid-clear], [data-grid-clear-all], [data-grid-move], [data-grid-reset]");
    if (!target) return;
    var grid = target.closest("form.rx-grid");
    if (!grid) return;
    if (target.hasAttribute("data-grid-sort")) {
      setState(grid, "sort", target.getAttribute("data-grid-sort"));
      submit(grid);
    } else if (target.hasAttribute("data-grid-page")) {
      setState(grid, "page", target.getAttribute("data-grid-page"));
      grid._rxKeepPage = true;
      submit(grid, true);
    } else if (target.hasAttribute("data-grid-clear")) {
      var pop = target.closest("[data-grid-filter]");
      clearFields(pop);
      submit(grid);
    } else if (target.hasAttribute("data-grid-clear-all")) {
      grid.querySelectorAll("[data-grid-filter]").forEach(clearFields);
      submit(grid);
    } else if (target.hasAttribute("data-grid-move")) {
      move(grid, target.closest("[data-grid-pick]").getAttribute("data-grid-pick"), parseInt(target.getAttribute("data-grid-move"), 10));
    } else if (target.hasAttribute("data-grid-reset")) {
      reset(grid);
    }
  });

  function clearFields(pop) {
    if (!pop) return;
    pop.querySelectorAll("input, select").forEach(function (el) {
      if (el.type === "checkbox") el.checked = false;
      else if (el.tagName === "SELECT") el.selectedIndex = 0;
      else el.value = "";
    });
    var range = pop.querySelector("calendar-range");
    if (range) range.removeAttribute("value");
  }

  document.addEventListener("change", function (event) {
    var el = event.target;
    var grid = el.closest && el.closest("form.rx-grid");
    if (!grid) return;
    if (el.hasAttribute("data-grid-per-page")) {
      submit(grid);
    } else if (el.hasAttribute("data-grid-toggle")) {
      toggle(grid, el.getAttribute("data-grid-toggle"), el.checked);
    } else if (el.hasAttribute("data-grid-pin")) {
      pin(grid, el.getAttribute("data-grid-pin"), el.value);
    } else if (el.matches("calendar-range[data-grid-range]")) {
      var parts = (el.value || "").split("/");
      var pop = el.closest("[data-grid-filter]");
      pop.querySelector('[data-grid-date="from"]').value = parts[0] || "";
      pop.querySelector('[data-grid-date="to"]').value = parts[1] || "";
    } else if (el.hasAttribute("data-grid-date")) {
      var box = el.closest("[data-grid-filter]");
      var from = box.querySelector('[data-grid-date="from"]').value;
      var to = box.querySelector('[data-grid-date="to"]').value;
      var cal = box.querySelector("calendar-range");
      if (cal && from && to) { cal.value = from + "/" + to; cal.focusedDate = from; }
    }
  });

  // ---------- The column menu ----------

  function prefs(cfg) {
    return { order: cfg.order, left: cfg.left, right: cfg.right, compact: cfg.compact, wide: cfg.wide };
  }

  function save(grid, method) {
    var cfg = config(grid);
    return fetch(cfg.prefs, {
      method: method || "POST",
      credentials: "same-origin",
      headers: { "Content-Type": "application/json", "X-CSRF-Token": csrf() },
      body: method === "DELETE" ? undefined : JSON.stringify(prefs(cfg))
    });
  }

  function reload(grid) {
    reopen = grid.id;
    grid._rxKeepPage = true;
    submit(grid, true);
  }

  function toggle(grid, key, on) {
    var cfg = config(grid);
    var list = cfg[screen()] || [];
    var set = new Set(list);
    if (on) set.add(key); else set.delete(key);
    cfg[screen()] = cfg.order.filter(function (k) { return set.has(k); });
    applyVisibility(grid);
    save(grid);
  }

  function pin(grid, key, side) {
    var cfg = config(grid);
    cfg.left = (cfg.left || []).filter(function (k) { return k !== key; });
    cfg.right = (cfg.right || []).filter(function (k) { return k !== key; });
    if (side === "left") cfg.left.push(key);
    if (side === "right") cfg.right.unshift(key);
    // Frozen columns are shown at their edge, in the menu's order.
    cfg.left = cfg.order.filter(function (k) { return cfg.left.indexOf(k) >= 0; });
    cfg.right = cfg.order.filter(function (k) { return cfg.right.indexOf(k) >= 0; });
    save(grid).then(function () { reload(grid); });
  }

  function move(grid, key, by) {
    var cfg = config(grid);
    var order = cfg.order.slice();
    var i = order.indexOf(key), j = i + by;
    if (i < 0 || j < 0 || j >= order.length) return;
    order.splice(i, 1);
    order.splice(j, 0, key);
    cfg.order = order;
    ["compact", "wide", "left", "right"].forEach(function (name) {
      var set = new Set(cfg[name] || []);
      cfg[name] = order.filter(function (k) { return set.has(k); });
    });
    save(grid).then(function () { reload(grid); });
  }

  function reset(grid) {
    save(grid, "DELETE").then(function () { reload(grid); });
  }

  function syncPicker(grid) {
    var shown = shownSet(grid);
    grid.querySelectorAll("[data-grid-toggle]").forEach(function (box) {
      box.checked = shown.has(box.getAttribute("data-grid-toggle"));
    });
  }

  // ---------- Popovers ----------

  function place(pop) {
    if (compact.matches) { pop.style.top = pop.style.left = ""; return; }
    var button = document.querySelector('[popovertarget="' + CSS.escape(pop.id) + '"]');
    if (!button) return;
    var r = button.getBoundingClientRect();
    var w = pop.offsetWidth, h = pop.offsetHeight;
    var left = r.left;
    if (left + w > window.innerWidth - 8) left = r.right - w;
    left = Math.max(8, Math.min(left, window.innerWidth - w - 8));
    var top = r.bottom + 6;
    if (top + h > window.innerHeight - 8) top = Math.max(8, r.top - h - 6);
    pop.style.left = left + "px";
    pop.style.top = top + "px";
  }

  document.addEventListener("toggle", function (event) {
    var pop = event.target;
    if (!pop.classList || !pop.classList.contains("rx-grid__pop") || event.newState !== "open") return;
    var grid = pop.closest("form.rx-grid");
    if (grid && pop.classList.contains("rx-grid__columns")) syncPicker(grid);
    var range = pop.querySelector("calendar-range");
    if (range) range.setAttribute("months", compact.matches ? "1" : "2");
    place(pop);
    // Again once the calendar has drawn its months.
    requestAnimationFrame(function () { place(pop); });
    var first = pop.querySelector("input:not([type=hidden]), select");
    if (first && !compact.matches) first.focus();
  }, true);

  // ---------- Setup ----------

  function setup(grid) {
    if (grid._rxReady) return;
    grid._rxReady = true;
    applyVisibility(grid);
    var scroll = grid.querySelector(".rx-grid__scroll");
    if (scroll) scroll.addEventListener("scroll", function () { edges(grid); }, { passive: true });
    if (window.ResizeObserver) {
      var table = grid.querySelector(".rx-grid__table");
      var pending = false;
      new ResizeObserver(function () {
        if (pending) return;
        pending = true;
        requestAnimationFrame(function () { pending = false; layoutPins(grid); });
      }).observe(table);
    }
    if (reopen === grid.id) {
      reopen = null;
      var menu = grid.querySelector(".rx-grid__columns");
      if (menu && menu.showPopover) menu.showPopover();
    }
  }

  function setupAll(root) {
    (root || document).querySelectorAll("form.rx-grid").forEach(setup);
    if (root && root.matches && root.matches("form.rx-grid")) setup(root);
  }

  compact.addEventListener("change", function () {
    document.querySelectorAll("form.rx-grid").forEach(applyVisibility);
  });
  document.addEventListener("htmx:load", function (event) { setupAll(event.target); });
  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", function () { setupAll(); });
  } else {
    setupAll();
  }

  window.RenoxGrid = { refresh: setupAll };
})();
