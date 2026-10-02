// Renox UI: behavior for the components of renox/ui.html. Plain DOM, no
// inline handlers (works under a strict CSP), everything keyboard-usable.
(function () {
  "use strict";

  var reduceMotion = window.matchMedia && window.matchMedia("(prefers-reduced-motion: reduce)").matches;

  function csrf() {
    var meta = document.querySelector('meta[name="csrf-token"]');
    return meta ? meta.content : "";
  }

  // ---------- Toasts ----------

  var ICONS = {
    success: '<svg viewBox="0 0 20 20" width="20" height="20"><circle cx="10" cy="10" r="9" fill="currentColor"/><path d="M6 10.5l2.5 2.5L14 7.5" stroke="#fff" stroke-width="2" fill="none" stroke-linecap="round" stroke-linejoin="round"/></svg>',
    info: '<svg viewBox="0 0 20 20" width="20" height="20"><circle cx="10" cy="10" r="9" fill="currentColor"/><path d="M10 9v5M10 6h.01" stroke="#fff" stroke-width="2" stroke-linecap="round"/></svg>',
    warning: '<svg viewBox="0 0 20 20" width="20" height="20"><path d="M10 2l8.5 15h-17z" fill="currentColor" stroke="currentColor" stroke-width="1.5" stroke-linejoin="round"/><path d="M10 8v4M10 14.5h.01" stroke="#fff" stroke-width="2" stroke-linecap="round"/></svg>',
    error: '<svg viewBox="0 0 20 20" width="20" height="20"><circle cx="10" cy="10" r="9" fill="currentColor"/><path d="M7 7l6 6M13 7l-6 6" stroke="#fff" stroke-width="2" stroke-linecap="round"/></svg>'
  };
  var CLOSE = '<svg viewBox="0 0 20 20" width="16" height="16" aria-hidden="true"><path d="M5 5l10 10M15 5L5 15" stroke="currentColor" stroke-width="2" stroke-linecap="round"/></svg>';

  function region() {
    var el = document.querySelector("[data-renox-toasts]");
    if (!el) {
      el = document.createElement("div");
      el.className = "rx-toasts";
      el.setAttribute("data-renox-toasts", "");
      el.setAttribute("aria-live", "polite");
      document.body.appendChild(el);
    }
    return el;
  }

  function dismiss(toast) {
    if (!toast || toast.hasAttribute("data-leaving")) return;
    toast.setAttribute("data-leaving", "");
    setTimeout(function () { toast.remove(); }, reduceMotion ? 0 : 200);
  }

  // Success and info leave after a while, longer for longer messages, and
  // wait while hovered or focused; errors stay until dismissed.
  function arm(toast) {
    if (toast.hasAttribute("data-sticky") || toast.hasAttribute("data-armed")) return;
    toast.setAttribute("data-armed", "");
    var text = (toast.textContent || "").length;
    var delay = Math.min(10000, 4000 + text * 40);
    var timer;
    function start() { timer = setTimeout(function () { dismiss(toast); }, delay); }
    function stop() { clearTimeout(timer); }
    toast.addEventListener("mouseenter", stop);
    toast.addEventListener("mouseleave", start);
    toast.addEventListener("focusin", stop);
    toast.addEventListener("focusout", start);
    start();
  }

  function showToast(toast) {
    var kind = ICONS[toast.kind] ? toast.kind : "info";
    var el = document.createElement("div");
    el.className = "rx-toast rx-toast--" + kind;
    el.setAttribute("role", kind === "error" ? "alert" : "status");
    el.setAttribute("data-renox-toast", "");
    if (kind === "error") el.setAttribute("data-sticky", "");
    var icon = document.createElement("span");
    icon.className = "rx-toast__icon";
    icon.setAttribute("aria-hidden", "true");
    icon.innerHTML = ICONS[kind];
    var message = document.createElement("p");
    message.className = "rx-toast__message";
    message.textContent = toast.message;
    var close = document.createElement("button");
    close.type = "button";
    close.className = "rx-toast__close";
    close.setAttribute("data-renox-dismiss", "");
    close.setAttribute("aria-label", region().getAttribute("data-dismiss-label") || "Dismiss");
    close.innerHTML = CLOSE;
    el.append(icon, message, close);
    region().appendChild(el);
    arm(el);
  }

  document.addEventListener("renox:toast", function (event) {
    var detail = event.detail || {};
    (detail.toasts || []).forEach(showToast);
  });

  // ---------- Sheets (dialogs) ----------

  function openSheet(id, opener) {
    var dialog = document.getElementById(id);
    if (!dialog || typeof dialog.showModal !== "function" || dialog.open) return;
    dialog._opener = opener;
    dialog.showModal();
    var first = dialog.querySelector("[autofocus]") || dialog.querySelector("[data-rx-initial-focus]");
    if (first) first.focus();
  }

  document.addEventListener("close", function (event) {
    var dialog = event.target;
    if (dialog && dialog._opener && dialog._opener.focus) dialog._opener.focus();
  }, true);

  // A click on the backdrop (outside the sheet's box) closes it.
  document.addEventListener("click", function (event) {
    var dialog = event.target;
    if (!dialog || dialog.tagName !== "DIALOG" || !dialog.classList.contains("rx-sheet")) return;
    var box = dialog.getBoundingClientRect();
    var inside = event.clientX >= box.left && event.clientX <= box.right &&
      event.clientY >= box.top && event.clientY <= box.bottom;
    if (!inside) dialog.close();
  });

  // ---------- Menus ----------

  function menuItems(list) {
    return Array.prototype.slice.call(list.querySelectorAll('[role="menuitem"]'));
  }

  function openMenu(button, focusLast) {
    var list = document.getElementById(button.getAttribute("aria-controls"));
    if (!list) return;
    closeMenus(list);
    list.hidden = false;
    button.setAttribute("aria-expanded", "true");
    var items = menuItems(list);
    var target = focusLast ? items[items.length - 1] : items[0];
    if (target) target.focus();
  }

  function closeMenus(except, restoreFocus) {
    document.querySelectorAll("[data-rx-menu] > [aria-expanded='true']").forEach(function (button) {
      var list = document.getElementById(button.getAttribute("aria-controls"));
      if (list === except) return;
      if (list) list.hidden = true;
      button.setAttribute("aria-expanded", "false");
      if (restoreFocus) button.focus();
    });
  }

  // ---------- Tabs (segmented control) ----------

  function selectTab(tab, focus) {
    var list = tab.closest('[role="tablist"]');
    if (!list) return;
    list.querySelectorAll('[role="tab"]').forEach(function (other) {
      var selected = other === tab;
      other.setAttribute("aria-selected", selected ? "true" : "false");
      other.tabIndex = selected ? 0 : -1;
      var panel = document.getElementById(other.getAttribute("aria-controls"));
      if (panel) panel.hidden = !selected;
    });
    if (focus) tab.focus();
  }

  // The first input sent as `name`: "photos.1" (an item's error) falls back
  // to "photos". In `scope` (the summary's form) first, else on the page.
  function fieldNamed(name, scope) {
    var names = [name];
    if (name.indexOf(".") > 0) names.push(name.slice(0, name.indexOf(".")));
    for (var i = 0; i < names.length; i++) {
      var selector = '[name="' + CSS.escape(names[i]) + '"]:not([type="hidden"])';
      var found = (scope && scope.querySelector(selector)) || document.querySelector(selector);
      if (found) return found;
    }
    return null;
  }

  // ---------- Clicks ----------

  document.addEventListener("click", function (event) {
    var target = event.target.closest ? event.target : event.target.parentElement;
    if (!target) return;

    // The error summary's links go to the field by name, so a field with
    // its own `id` (or a radio group) is found too.
    var summaryLink = target.closest("[data-rx-error-summary] [data-rx-field]");
    if (summaryLink) {
      var field = fieldNamed(summaryLink.getAttribute("data-rx-field"), summaryLink.closest("form"));
      if (field) { event.preventDefault(); field.focus(); field.scrollIntoView({ block: "center", behavior: reduceMotion ? "auto" : "smooth" }); }
      return;
    }

    var dismisser = target.closest("[data-renox-dismiss]");
    if (dismisser) { dismiss(dismisser.closest("[data-renox-toast]")); return; }

    var opener = target.closest("[data-rx-open]");
    if (opener) { event.preventDefault(); openSheet(opener.getAttribute("data-rx-open"), opener); return; }

    var closer = target.closest("[data-rx-close]");
    if (closer) { var d = closer.closest("dialog"); if (d) d.close(); return; }

    var menuButton = target.closest("[data-rx-menu] > [aria-haspopup]");
    if (menuButton) {
      if (menuButton.getAttribute("aria-expanded") === "true") closeMenus(null, true);
      else openMenu(menuButton);
      return;
    }
    if (!target.closest("[data-rx-menu]")) closeMenus(null);

    var tab = target.closest('[role="tab"]');
    if (tab) selectTab(tab, false);
  });

  // ---------- Keys ----------

  document.addEventListener("keydown", function (event) {
    var target = event.target;
    // Menus: arrows move, Esc closes and returns focus to the button.
    var list = target.closest && target.closest('[role="menu"]');
    var menuButton = target.closest && target.closest("[data-rx-menu] > [aria-haspopup]");
    if (menuButton && (event.key === "ArrowDown" || event.key === "ArrowUp")) {
      event.preventDefault();
      openMenu(menuButton, event.key === "ArrowUp");
      return;
    }
    if (list) {
      var items = menuItems(list);
      var i = items.indexOf(target);
      if (event.key === "ArrowDown") { event.preventDefault(); items[(i + 1) % items.length].focus(); }
      else if (event.key === "ArrowUp") { event.preventDefault(); items[(i - 1 + items.length) % items.length].focus(); }
      else if (event.key === "Home") { event.preventDefault(); items[0].focus(); }
      else if (event.key === "End") { event.preventDefault(); items[items.length - 1].focus(); }
      else if (event.key === "Escape") { event.preventDefault(); closeMenus(null, true); }
      else if (event.key === "Tab") { closeMenus(null); }
      return;
    }
    if (event.key === "Escape") closeMenus(null, true);

    // Tabs: arrows, Home and End move the selection.
    if (target.getAttribute && target.getAttribute("role") === "tab") {
      var tabs = Array.prototype.slice.call(target.closest('[role="tablist"]').querySelectorAll('[role="tab"]'));
      var j = tabs.indexOf(target);
      var next = null;
      if (event.key === "ArrowRight") next = tabs[(j + 1) % tabs.length];
      else if (event.key === "ArrowLeft") next = tabs[(j - 1 + tabs.length) % tabs.length];
      else if (event.key === "Home") next = tabs[0];
      else if (event.key === "End") next = tabs[tabs.length - 1];
      if (next) { event.preventDefault(); selectTab(next, true); }
    }
  });

  // ---------- Busy buttons ----------

  // A regular form shows its submit button as busy and can't be sent twice.
  document.addEventListener("submit", function (event) {
    var form = event.target;
    if (event.defaultPrevented || form.hasAttribute("hx-post") || form.hasAttribute("hx-put") ||
        form.hasAttribute("hx-patch") || form.hasAttribute("hx-delete")) return;
    if (form.hasAttribute("data-rx-sending")) { event.preventDefault(); return; }
    form.setAttribute("data-rx-sending", "");
    var button = event.submitter || form.querySelector('button[type="submit"], button:not([type])');
    if (button && button.classList.contains("rx-button")) button.setAttribute("aria-busy", "true");
  });
  // Back/forward cache: a page shown again is ready again.
  window.addEventListener("pageshow", function () {
    document.querySelectorAll("[data-rx-sending]").forEach(function (form) { form.removeAttribute("data-rx-sending"); });
    document.querySelectorAll('.rx-button[aria-busy="true"]').forEach(function (b) { b.removeAttribute("aria-busy"); });
  });
  document.addEventListener("htmx:beforeRequest", function (event) {
    var elt = event.detail && event.detail.elt;
    var button = elt && (elt.classList && elt.classList.contains("rx-button") ? elt : elt.querySelector && elt.querySelector('.rx-button[type="submit"]'));
    if (button) button.setAttribute("aria-busy", "true");
  });
  document.addEventListener("htmx:afterRequest", function (event) {
    var elt = event.detail && event.detail.elt;
    if (!elt || !elt.querySelectorAll) return;
    if (elt.removeAttribute) elt.removeAttribute("aria-busy");
    elt.querySelectorAll('.rx-button[aria-busy="true"]').forEach(function (b) { b.removeAttribute("aria-busy"); });
  });

  // ---------- Live validation ----------

  // Forms with data-live-validate check a field when it's left, then as it's
  // typed in while it has an error: quick to confirm a fix, never nagging
  // about a field that's half-typed. The handler doesn't run.
  var timers = new WeakMap();

  function setFieldError(form, name, messages) {
    var slot = form.querySelector('[data-error-for="' + CSS.escape(name) + '"]');
    var inputs = form.querySelectorAll('[name="' + CSS.escape(name) + '"]');
    var message = (messages || [])[0] || "";
    if (slot) slot.textContent = message;
    inputs.forEach(function (input) {
      if (message) input.setAttribute("aria-invalid", "true");
      else input.removeAttribute("aria-invalid");
    });
  }

  function validate(form, input) {
    var name = input.name;
    if (!name || input.type === "file" || input.type === "password" && !input.value) return;
    var body = new FormData(form);
    var method = (form.getAttribute("method") || "post").toUpperCase();
    if (method === "GET") return;
    fetch(form.getAttribute("action") || location.href, {
      method: "POST",
      body: body,
      credentials: "same-origin",
      headers: { "X-Renox-Validate": name, "X-CSRF-Token": csrf(), "Accept": "application/json" }
    })
      .then(function (res) { return res.ok ? res.json() : null; })
      .then(function (data) {
        if (!data || data.field !== name) return;
        input.setAttribute("data-rx-checked", "");
        setFieldError(form, name, data.errors);
      })
      .catch(function () {});
  }

  document.addEventListener("focusout", function (event) {
    var input = event.target;
    var form = input.form;
    if (!form || !form.hasAttribute("data-live-validate") || !input.name) return;
    if (!input.value && !input.hasAttribute("data-rx-checked")) return; // not touched yet
    validate(form, input);
  });

  document.addEventListener("input", function (event) {
    var input = event.target;
    var form = input.form;
    if (!form || !form.hasAttribute("data-live-validate") || input.getAttribute("aria-invalid") !== "true") return;
    clearTimeout(timers.get(input));
    timers.set(input, setTimeout(function () { validate(form, input); }, 400));
  });

  // ---------- Field buttons: show the password, copy ----------

  document.addEventListener("click", function (event) {
    var target = event.target.closest ? event.target : event.target.parentElement;
    if (!target) return;
    var reveal = target.closest("[data-rx-reveal]");
    if (reveal) {
      var field = document.getElementById(reveal.getAttribute("data-rx-reveal"));
      if (!field) return;
      var shown = field.type === "password";
      field.type = shown ? "text" : "password";
      reveal.setAttribute("aria-pressed", shown ? "true" : "false");
      // The label says what a press does next.
      var next = reveal.getAttribute("data-label-hide");
      reveal.setAttribute("data-label-hide", reveal.getAttribute("aria-label"));
      reveal.setAttribute("aria-label", next);
      return;
    }
    var copy = target.closest("[data-rx-copy]");
    if (copy) {
      var source = document.getElementById(copy.getAttribute("data-rx-copy"));
      if (!source || !navigator.clipboard) return;
      navigator.clipboard.writeText(source.value).then(function () {
        var said = copy.querySelector("[aria-live]");
        copy.setAttribute("data-rx-done", "");
        if (said) said.textContent = copy.getAttribute("data-label-done");
        setTimeout(function () {
          copy.removeAttribute("data-rx-done");
          if (said) said.textContent = "";
        }, 1500);
      }, function () {});
    }
  });

  // ---------- File fields ----------

  function fileSize(bytes) {
    if (bytes < 1024) return bytes + " B";
    if (bytes < 1024 * 1024) return Math.round(bytes / 1024) + " KB";
    return (bytes / 1024 / 1024).toFixed(1) + " MB";
  }

  function listFiles(input) {
    var box = input.closest("[data-rx-file]");
    var list = box && box.parentElement.querySelector("[data-rx-file-list]");
    if (!list) return;
    list.querySelectorAll("img").forEach(function (img) { URL.revokeObjectURL(img.src); });
    list.textContent = "";
    Array.prototype.forEach.call(input.files || [], function (file) {
      var item = document.createElement("li");
      item.className = "rx-file__item";
      if (box.hasAttribute("data-rx-preview") && /^image\//.test(file.type)) {
        var img = document.createElement("img");
        img.className = "rx-file__thumb";
        img.alt = "";
        img.src = URL.createObjectURL(file);
        item.appendChild(img);
      }
      var name = document.createElement("span");
      name.className = "rx-file__name";
      name.textContent = file.name;
      var size = document.createElement("span");
      size.className = "rx-file__size";
      size.textContent = fileSize(file.size);
      item.append(name, size);
      list.appendChild(item);
    });
  }

  document.addEventListener("change", function (event) {
    var input = event.target;
    if (input.classList && input.classList.contains("rx-file__input")) listFiles(input);
  });
  ["dragenter", "dragover"].forEach(function (type) {
    document.addEventListener(type, function (event) {
      var box = event.target.closest && event.target.closest("[data-rx-file]");
      if (box) box.setAttribute("data-rx-dragging", "");
    });
  });
  ["dragleave", "drop"].forEach(function (type) {
    document.addEventListener(type, function (event) {
      var box = event.target.closest && event.target.closest("[data-rx-file]");
      if (box) box.removeAttribute("data-rx-dragging");
    });
  });

  // ---------- Date picker ----------

  var ISO_DATE = /^\d{4}-\d{2}-\d{2}$/;

  function today() {
    var d = new Date();
    return d.getFullYear() + "-" + String(d.getMonth() + 1).padStart(2, "0") + "-" + String(d.getDate()).padStart(2, "0");
  }

  // The header names the month shown, in the page's language.
  function calendarHeading(cal, iso) {
    var slot = cal.querySelector(".rx-calendar__heading");
    if (!slot) return;
    // A YYYY-MM-DD string, or the Date Cally's focusday event carries.
    var day = new Date();
    if (iso instanceof Date) day = iso;
    else if (typeof iso === "string" && ISO_DATE.test(iso)) day = new Date(+iso.slice(0, 4), +iso.slice(5, 7) - 1, 1);
    try {
      slot.textContent = new Intl.DateTimeFormat(cal.getAttribute("locale") || undefined, { month: "long", year: "numeric" }).format(day);
    } catch (e) {
      slot.textContent = day.getFullYear() + "-" + String(day.getMonth() + 1).padStart(2, "0");
    }
  }
  // Moving the focus (arrows, the month buttons) changes the month shown.
  document.addEventListener("focusday", function (event) {
    var cal = event.target;
    if (cal.classList && cal.classList.contains("rx-calendar")) calendarHeading(cal, event.detail);
  }, true);

  // The calendar opens under its field (above it when there's no room),
  // showing the field's date, and focuses that day.
  document.addEventListener("toggle", function (event) {
    var pop = event.target;
    if (!pop.hasAttribute || !pop.hasAttribute("data-rx-calendar-for") || event.newState !== "open") return;
    var input = document.getElementById(pop.getAttribute("data-rx-calendar-for"));
    var cal = pop.querySelector("calendar-date");
    if (!input || !cal) return;
    if (ISO_DATE.test(input.value)) { cal.value = input.value; cal.focusedDate = input.value; }
    calendarHeading(cal, ISO_DATE.test(input.value) ? input.value : cal.getAttribute("min") > today() ? cal.getAttribute("min") : null);
    var box = (input.closest(".rx-affix") || input).getBoundingClientRect();
    var height = pop.offsetHeight, width = pop.offsetWidth;
    var top = box.bottom + 6;
    if (top + height > window.innerHeight - 8 && box.top - height - 6 > 8) top = box.top - height - 6;
    var left = Math.min(box.right - width, window.innerWidth - width - 8);
    pop.style.top = Math.max(8, top) + "px";
    pop.style.left = Math.max(8, left) + "px";
    requestAnimationFrame(function () { if (cal.focus) cal.focus(); });
  }, true);

  document.addEventListener("change", function (event) {
    var el = event.target;
    if (el.tagName === "CALENDAR-DATE" && el.closest("[data-rx-calendar-for]")) {
      var pop = el.closest("[data-rx-calendar-for]");
      var input = document.getElementById(pop.getAttribute("data-rx-calendar-for"));
      if (!input) return;
      input.value = el.value;
      if (pop.hidePopover) pop.hidePopover();
      input.focus();
      input.dispatchEvent(new Event("input", { bubbles: true }));
      input.dispatchEvent(new Event("change", { bubbles: true }));
    }
  }, true); // Cally's change event doesn't bubble: catch it on the way down.

  // ---------- show_when / hide_when ----------

  // The values `name` has in `form` now: the ticked radio or checkboxes, a
  // select's choices, or a field's text.
  function valuesOf(form, name) {
    var values = [];
    (form ? form.querySelectorAll('[name="' + CSS.escape(name) + '"]') : []).forEach(function (el) {
      if (el.disabled) return;
      if (el.type === "radio" || el.type === "checkbox") { if (el.checked) values.push(el.value); }
      else if (el.tagName === "SELECT") Array.prototype.forEach.call(el.selectedOptions, function (o) { values.push(o.value); });
      else values.push(el.value);
    });
    return values;
  }

  function applyWhen(group) {
    var show = group.hasAttribute("data-rx-show-when");
    var name = group.getAttribute(show ? "data-rx-show-when" : "data-rx-hide-when");
    var wanted;
    try { wanted = JSON.parse(group.getAttribute("data-rx-values") || "[]"); } catch (e) { wanted = []; }
    var matches = valuesOf(group.form || group.closest("form"), name).some(function (v) { return wanted.indexOf(v) >= 0; });
    var visible = show ? matches : !matches;
    // Hidden fields are disabled too, so the form doesn't send them.
    group.hidden = !visible;
    group.disabled = !visible;
  }

  function applyAllWhen(root) {
    (root || document).querySelectorAll("[data-rx-show-when], [data-rx-hide-when]").forEach(applyWhen);
  }

  ["change", "input"].forEach(function (type) {
    document.addEventListener(type, function (event) {
      var el = event.target;
      if (!el.name || !el.form) return;
      el.form.querySelectorAll("[data-rx-show-when], [data-rx-hide-when]").forEach(function (group) {
        var name = group.getAttribute("data-rx-show-when") || group.getAttribute("data-rx-hide-when");
        if (name === el.name) applyWhen(group);
      });
    });
  });

  // Toasts rendered with the page leave on their own too; conditional
  // groups take their state from the fields, here and in htmx swaps.
  function armAll() {
    document.querySelectorAll("[data-renox-toast]").forEach(arm);
    applyAllWhen(document);
  }
  if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", armAll);
  else armAll();
  document.addEventListener("htmx:load", function (event) { applyAllWhen(event.target); });
})();
