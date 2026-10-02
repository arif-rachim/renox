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
    if (name.indexOf(".") > 0) {
      var parts = name.split(".");
      names.push(parts[0] + parts.slice(1).map(function (p) { return "[" + p + "]"; }).join(""));
      names.push(parts[0]);
    }
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

  // `items[0][name]` as its errors are keyed: `items.0.name`.
  function errorKey(name) {
    return name.indexOf("[") < 0 ? name : name.replace(/\]/g, "").split(/[\[.]/).filter(Boolean).join(".");
  }

  function setFieldError(form, name, messages) {
    var slot = form.querySelector('[data-error-for="' + CSS.escape(errorKey(name)) + '"]');
    var inputs = form.querySelectorAll('[name="' + CSS.escape(name) + '"]');
    var message = (messages || [])[0] || "";
    if (slot) slot.textContent = message;
    inputs.forEach(function (input) {
      if (message) input.setAttribute("aria-invalid", "true");
      else input.removeAttribute("aria-invalid");
    });
  }

  // Resolves with the field's messages (none: valid), or null when it
  // can't be checked.
  function validate(form, input) {
    var name = input.name;
    if (!name || input.type === "file" || input.type === "password" && !input.value) return Promise.resolve(null);
    var body = new FormData(form);
    var method = (form.getAttribute("method") || "post").toUpperCase();
    if (method === "GET") return Promise.resolve(null);
    return fetch(form.getAttribute("action") || location.href, {
      method: "POST",
      body: body,
      credentials: "same-origin",
      headers: { "X-Renox-Validate": name, "X-CSRF-Token": csrf(), "Accept": "application/json" }
    })
      .then(function (res) { return res.ok ? res.json() : null; })
      .then(function (data) {
        if (!data || data.field !== name) return null;
        input.setAttribute("data-rx-checked", "");
        setFieldError(form, name, data.errors);
        return data.errors || [];
      })
      .catch(function () { return null; });
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

  // ---------- Tags ----------

  function tagValues(box) {
    return Array.prototype.map.call(box.querySelectorAll('.rx-tag input[type="hidden"]'), function (i) { return i.value; });
  }

  function addTags(box, text) {
    var entry = box.querySelector("[data-rx-tags-entry]");
    var list = box.querySelector(".rx-tags__list");
    var name = box.getAttribute("data-rx-tags");
    var have = tagValues(box).map(function (v) { return v.toLowerCase(); });
    var added = false;
    text.split(",").map(function (t) { return t.trim(); }).filter(Boolean).forEach(function (tag) {
      if (have.indexOf(tag.toLowerCase()) >= 0) return;
      have.push(tag.toLowerCase());
      var li = document.createElement("li");
      li.className = "rx-tag";
      var label = document.createElement("span");
      label.className = "rx-tag__text";
      label.textContent = tag;
      var remove = document.createElement("button");
      remove.type = "button";
      remove.className = "rx-tag__remove";
      remove.setAttribute("data-rx-tag-remove", "");
      remove.setAttribute("aria-label", (box.getAttribute("data-remove") || "Remove") + " " + tag);
      remove.innerHTML = CLOSE;
      var hidden = document.createElement("input");
      hidden.type = "hidden";
      hidden.name = name;
      hidden.value = tag;
      li.append(label, remove, hidden);
      list.appendChild(li);
      added = true;
    });
    if (entry) entry.value = "";
    if (added && entry) entry.dispatchEvent(new Event("change", { bubbles: true }));
  }

  document.addEventListener("keydown", function (event) {
    var entry = event.target;
    if (!entry.hasAttribute || !entry.hasAttribute("data-rx-tags-entry")) return;
    var box = entry.closest("[data-rx-tags]");
    if (event.key === "Enter" || event.key === ",") {
      if (event.isComposing) return;
      event.preventDefault();
      addTags(box, entry.value);
    } else if (event.key === "Backspace" && !entry.value) {
      var last = box.querySelector(".rx-tag:last-of-type");
      if (last) { last.remove(); entry.dispatchEvent(new Event("change", { bubbles: true })); }
    }
  });
  document.addEventListener("input", function (event) {
    var entry = event.target;
    if (!entry.hasAttribute || !entry.hasAttribute("data-rx-tags-entry") || entry.value.indexOf(",") < 0) return;
    // A pasted "a, b, c": all but what follows the last comma become tags.
    var cut = entry.value.lastIndexOf(",");
    var rest = entry.value.slice(cut + 1);
    addTags(entry.closest("[data-rx-tags]"), entry.value.slice(0, cut));
    entry.value = rest;
  });
  document.addEventListener("focusout", function (event) {
    var entry = event.target;
    if (entry.hasAttribute && entry.hasAttribute("data-rx-tags-entry") && entry.value.trim()) {
      addTags(entry.closest("[data-rx-tags]"), entry.value);
    }
  });
  // Text still in the box when the form is sent is a tag too.
  document.addEventListener("submit", function (event) {
    event.target.querySelectorAll && event.target.querySelectorAll("[data-rx-tags-entry]").forEach(function (entry) {
      if (entry.value.trim()) addTags(entry.closest("[data-rx-tags]"), entry.value);
    });
  }, true);
  document.addEventListener("click", function (event) {
    var target = event.target.closest ? event.target : event.target.parentElement;
    if (!target) return;
    var remove = target.closest("[data-rx-tag-remove]");
    if (remove) {
      var box = remove.closest("[data-rx-tags]");
      remove.closest(".rx-tag").remove();
      var entry = box.querySelector("[data-rx-tags-entry]");
      if (entry) { entry.focus(); entry.dispatchEvent(new Event("change", { bubbles: true })); }
      return;
    }
    var tags = target.closest(".rx-tags");
    if (tags && target === tags) {
      var input = tags.querySelector("input:not([type=hidden])");
      if (input) input.focus();
    }
  });

  // ---------- Repeater ----------

  function idOf(name) {
    return "rx-" + name.replace(/\./g, "-").replace(/\[/g, "-").replace(/\]/g, "");
  }

  function rowsOf(rep) {
    var holder = rep.querySelector("[data-rx-rows]");
    return Array.prototype.filter.call(holder.children, function (el) { return el.hasAttribute("data-rx-row"); });
  }

  // Names, ids and references follow the row's place: `lines[2][name]`,
  // `lines.2.name`, `rx-lines-2-name`.
  function renumber(rep) {
    var name = rep.getAttribute("data-rx-repeater");
    var dotted = errorKey(name), idp = idOf(name);
    rowsOf(rep).forEach(function (row, n) {
      var o = row.getAttribute("data-rx-index");
      if (o !== String(n)) {
        var swaps = [[name + "[" + o + "]", name + "[" + n + "]"], [dotted + "." + o + ".", dotted + "." + n + "."], [idp + "-" + o + "-", idp + "-" + n + "-"]];
        [row].concat(Array.prototype.slice.call(row.querySelectorAll("*"))).forEach(function (el) {
          Array.prototype.forEach.call(el.attributes, function (attr) {
            var value = attr.value, next = value;
            swaps.forEach(function (s) { next = next.split(s[0]).join(s[1]); });
            if (next !== value) el.setAttribute(attr.name, next);
          });
        });
        row.setAttribute("data-rx-index", String(n));
      }
      var number = row.querySelector("[data-rx-row-number]");
      if (number) number.textContent = String(n + 1);
    });
    limits(rep);
  }

  function limits(rep) {
    var count = rowsOf(rep).length;
    var min = parseInt(rep.getAttribute("data-rx-min") || "0", 10);
    var max = parseInt(rep.getAttribute("data-rx-max") || "0", 10);
    rep.querySelectorAll("[data-rx-row-remove]").forEach(function (b) { if (b.closest("[data-rx-repeater]") === rep) b.disabled = count <= min; });
    var add = rep.querySelector("[data-rx-row-add]");
    if (add) add.disabled = max > 0 && count >= max;
  }

  function firstField(root) {
    return root.querySelector("input:not([type=hidden]):not([disabled]), select:not([data-rx-enhanced]), textarea, [role=combobox]");
  }

  function addRow(rep) {
    var template = Array.prototype.find.call(rep.querySelectorAll("template[data-rx-row-template]"), function (t) { return t.closest("[data-rx-repeater]") === rep; });
    if (!template) return;
    var index = rowsOf(rep).length;
    var holder = document.createElement("template");
    holder.innerHTML = template.innerHTML.split("__INDEX__").join(String(index));
    var row = holder.content.querySelector("[data-rx-row]");
    // Scripts cloned from a template don't run: put them back as new ones,
    // once per source.
    holder.content.querySelectorAll("script").forEach(function (old) {
      if (old.src && document.querySelector('script[src="' + old.getAttribute("src") + '"]')) { old.remove(); return; }
      var fresh = document.createElement("script");
      Array.prototype.forEach.call(old.attributes, function (a) { fresh.setAttribute(a.name, a.value); });
      fresh.textContent = old.textContent;
      old.replaceWith(fresh);
    });
    rep.querySelector("[data-rx-rows]").appendChild(holder.content);
    renumber(rep);
    setup(row);
    if (window.htmx && window.htmx.process) window.htmx.process(row);
    var first = firstField(row);
    if (first) first.focus();
  }

  document.addEventListener("click", function (event) {
    var target = event.target.closest ? event.target : event.target.parentElement;
    if (!target) return;
    var button = target.closest("[data-rx-row-add], [data-rx-row-remove], [data-rx-row-up], [data-rx-row-down]");
    if (!button || button.disabled) return;
    var rep = button.closest("[data-rx-repeater]");
    if (button.hasAttribute("data-rx-row-add")) { addRow(rep); return; }
    var row = button.closest("[data-rx-row]");
    if (button.hasAttribute("data-rx-row-remove")) {
      var next = row.nextElementSibling || row.previousElementSibling;
      row.remove();
      renumber(rep);
      var focus = next && firstField(next) || rep.querySelector("[data-rx-row-add]");
      if (focus) focus.focus();
    } else if (button.hasAttribute("data-rx-row-up") && row.previousElementSibling) {
      row.parentNode.insertBefore(row, row.previousElementSibling);
      renumber(rep);
      button.focus();
    } else if (button.hasAttribute("data-rx-row-down") && row.nextElementSibling) {
      row.parentNode.insertBefore(row.nextElementSibling, row);
      renumber(rep);
      button.focus();
    }
    var form = rep.closest("form");
    if (form) form.dispatchEvent(new Event("change", { bubbles: true }));
  });

  // ---------- Searchable select (combobox) ----------

  var comboCount = 0;

  function enhanceSelect(select) {
    if (select.hasAttribute("data-rx-enhanced")) return;
    select.setAttribute("data-rx-enhanced", "");
    select.setAttribute("tabindex", "-1");
    select.setAttribute("aria-hidden", "true");
    var multiple = select.multiple;
    var base = select.id || "rx-combobox-" + (++comboCount);
    var wrap = document.createElement("div");
    wrap.className = "rx-combobox";
    select.parentNode.insertBefore(wrap, select);
    var box = document.createElement("div");
    box.className = "rx-combobox__box" + (multiple ? " rx-tags" : "");
    var input = document.createElement("input");
    input.type = "text";
    input.id = base + "-search";
    input.className = multiple ? "rx-tags__entry" : "rx-input rx-select";
    input.autocomplete = "off";
    input.setAttribute("role", "combobox");
    input.setAttribute("aria-autocomplete", "list");
    input.setAttribute("aria-expanded", "false");
    input.setAttribute("aria-controls", base + "-listbox");
    ["aria-describedby", "aria-invalid", "aria-required"].forEach(function (a) { if (select.hasAttribute(a)) input.setAttribute(a, select.getAttribute(a)); });
    input.placeholder = select.getAttribute("data-placeholder") || "";
    input.disabled = select.disabled;
    var chips = document.createElement("ul");
    chips.className = "rx-tags__list";
    chips.setAttribute("role", "list");
    if (multiple) box.append(chips);
    box.append(input);
    var list = document.createElement("ul");
    list.className = "rx-combobox__list";
    list.id = base + "-listbox";
    list.setAttribute("role", "listbox");
    if (multiple) list.setAttribute("aria-multiselectable", "true");
    list.hidden = true;
    wrap.append(box, list, select);
    var label = select.id && document.querySelector('label[for="' + CSS.escape(select.id) + '"]');
    if (label) { label.htmlFor = input.id; list.setAttribute("aria-labelledby", label.id || (label.id = base + "-label")); }

    // A single select's empty option ("None") stays choosable, to clear it.
    var options = Array.prototype.filter.call(select.options, function (o) { return o.value !== "" || !multiple; });

    function selectedText() {
      var chosen = select.options[select.selectedIndex];
      return chosen && chosen.value !== "" ? chosen.textContent : "";
    }

    function renderChips() {
      if (!multiple) { input.value = selectedText(); return; }
      chips.textContent = "";
      options.filter(function (o) { return o.selected; }).forEach(function (o) {
        var li = document.createElement("li");
        li.className = "rx-tag";
        var text = document.createElement("span");
        text.className = "rx-tag__text";
        text.textContent = o.textContent;
        var remove = document.createElement("button");
        remove.type = "button";
        remove.className = "rx-tag__remove";
        remove.setAttribute("aria-label", (select.getAttribute("data-remove") || "Remove") + " " + o.textContent);
        remove.innerHTML = CLOSE;
        remove.disabled = select.disabled;
        remove.addEventListener("click", function () { o.selected = false; changed(); input.focus(); });
        li.append(text, remove);
        chips.appendChild(li);
      });
    }

    function render(filter) {
      var needle = (filter || "").trim().toLowerCase();
      list.textContent = "";
      var shown = 0;
      options.forEach(function (o, i) {
        if (needle && o.textContent.toLowerCase().indexOf(needle) < 0) return;
        var li = document.createElement("li");
        li.className = "rx-combobox__option";
        li.id = base + "-option-" + i;
        li.setAttribute("role", "option");
        li.setAttribute("aria-selected", o.selected ? "true" : "false");
        li.textContent = o.textContent;
        li.addEventListener("mousedown", function (e) { e.preventDefault(); });
        li.addEventListener("click", function () { choose(o); });
        li._option = o;
        list.appendChild(li);
        shown++;
      });
      if (!shown) {
        var empty = document.createElement("li");
        empty.className = "rx-combobox__empty";
        empty.setAttribute("role", "presentation");
        empty.textContent = select.getAttribute("data-empty") || "No matches";
        list.appendChild(empty);
      }
      activate(list.querySelector('[aria-selected="true"]') || list.querySelector("[role=option]"));
    }

    function activate(li) {
      list.querySelectorAll("[data-active]").forEach(function (el) { el.removeAttribute("data-active"); });
      if (!li) { input.removeAttribute("aria-activedescendant"); return; }
      li.setAttribute("data-active", "");
      input.setAttribute("aria-activedescendant", li.id);
      li.scrollIntoView({ block: "nearest" });
    }

    function open() {
      if (input.disabled || !list.hidden) return;
      list.hidden = false;
      input.setAttribute("aria-expanded", "true");
      render(multiple ? input.value : "");
    }

    function close() {
      list.hidden = true;
      input.setAttribute("aria-expanded", "false");
      input.removeAttribute("aria-activedescendant");
      if (!multiple) input.value = selectedText();
    }

    function changed() {
      renderChips();
      if (!list.hidden) render(multiple ? input.value : "");
      select.dispatchEvent(new Event("change", { bubbles: true }));
    }

    function choose(option) {
      if (multiple) {
        option.selected = !option.selected;
        input.value = "";
        changed();
      } else {
        select.value = option.value;
        changed();
        close();
      }
    }

    input.addEventListener("focus", open);
    input.addEventListener("click", open);
    input.addEventListener("input", function () { if (list.hidden) open(); render(input.value); });
    input.addEventListener("blur", function () { setTimeout(close, 0); });
    input.addEventListener("keydown", function (e) {
      var items = Array.prototype.slice.call(list.querySelectorAll("[role=option]"));
      var current = list.querySelector("[data-active]");
      var i = items.indexOf(current);
      if (e.key === "ArrowDown") { e.preventDefault(); if (list.hidden) open(); else activate(items[Math.min(i + 1, items.length - 1)]); }
      else if (e.key === "ArrowUp") { e.preventDefault(); activate(items[Math.max(i - 1, 0)]); }
      else if (e.key === "Home" && !list.hidden) { e.preventDefault(); activate(items[0]); }
      else if (e.key === "End" && !list.hidden) { e.preventDefault(); activate(items[items.length - 1]); }
      else if (e.key === "Enter") { if (!list.hidden && current) { e.preventDefault(); choose(current._option); } }
      else if (e.key === "Escape") { if (!list.hidden) { e.preventDefault(); close(); } }
      else if (e.key === "Backspace" && multiple && !input.value) {
        var last = options.filter(function (o) { return o.selected; }).pop();
        if (last) { last.selected = false; changed(); }
      }
    });
    box.addEventListener("click", function (e) { if (e.target === box) input.focus(); });
    // The browser checks the hidden select for `required`: point at the box.
    select.addEventListener("invalid", function () { input.focus(); });
    select.addEventListener("rx:refresh", renderChips);
    renderChips();
  }

  // ---------- Wizard ----------

  function wizardParts(wizard) {
    return {
      panels: Array.prototype.filter.call(wizard.querySelectorAll("[data-rx-step]"), function (p) { return p.closest("[data-rx-wizard]") === wizard; }),
      tabs: Array.prototype.filter.call(wizard.querySelectorAll("[data-rx-step-tab]"), function (t) { return t.closest("[data-rx-wizard]") === wizard; })
    };
  }

  function showStep(wizard, index, focus) {
    var parts = wizardParts(wizard);
    var last = parts.panels.length - 1;
    parts.panels.forEach(function (panel, i) {
      panel.hidden = i !== index;
      if (!panel.hasAttribute("aria-label") && parts.tabs[i]) panel.setAttribute("aria-label", parts.tabs[i].textContent.trim());
      panel.setAttribute("tabindex", "-1");
    });
    parts.tabs.forEach(function (tab, i) {
      if (i === index) tab.setAttribute("aria-current", "step"); else tab.removeAttribute("aria-current");
      if (i < index) tab.setAttribute("data-done", ""); else tab.removeAttribute("data-done");
    });
    wizard.setAttribute("data-rx-step-index", String(index));
    var back = wizard.querySelector("[data-rx-wizard-back]");
    var next = wizard.querySelector("[data-rx-wizard-next]");
    var submit = wizard.querySelector("[data-rx-wizard-submit]");
    if (back) back.hidden = index === 0;
    if (next) next.hidden = index >= last;
    if (submit) submit.hidden = index < last;
    if (focus && parts.panels[index]) {
      var first = firstField(parts.panels[index]);
      (first || parts.panels[index]).focus();
    }
  }

  // The step's fields pass the browser's rules, then (data-live-validate)
  // the server's.
  function checkStep(wizard, panel) {
    var form = wizard.closest("form");
    var fields = Array.prototype.filter.call(panel.querySelectorAll("input, select, textarea"), function (el) {
      return el.name && !el.disabled && el.type !== "hidden" && el.type !== "button" && el.type !== "submit";
    });
    for (var i = 0; i < fields.length; i++) {
      if (!fields[i].checkValidity()) {
        if (fields[i].hasAttribute("data-rx-enhanced")) fields[i].dispatchEvent(new Event("invalid"));
        fields[i].reportValidity();
        return Promise.resolve(false);
      }
    }
    if (!form || !form.hasAttribute("data-live-validate")) return Promise.resolve(true);
    var seen = {};
    var unique = fields.filter(function (el) {
      if (seen[el.name] || el.type === "file") return false;
      seen[el.name] = true;
      return true;
    });
    return Promise.all(unique.map(function (el) { return validate(form, el); })).then(function (results) {
      var bad = results.some(function (messages) { return messages && messages.length; });
      if (bad) {
        var first = panel.querySelector('[aria-invalid="true"]');
        if (first) first.focus();
      }
      return !bad;
    });
  }

  function stepIndex(wizard) {
    return parseInt(wizard.getAttribute("data-rx-step-index") || "0", 10);
  }

  function nextStep(wizard) {
    var parts = wizardParts(wizard);
    var index = stepIndex(wizard);
    var button = wizard.querySelector("[data-rx-wizard-next]");
    if (button) button.setAttribute("aria-busy", "true");
    checkStep(wizard, parts.panels[index]).then(function (ok) {
      if (button) button.removeAttribute("aria-busy");
      if (ok) showStep(wizard, Math.min(index + 1, parts.panels.length - 1), true);
    });
  }

  function setupWizard(wizard) {
    if (wizard.hasAttribute("data-rx-ready")) return;
    wizard.setAttribute("data-rx-ready", "");
    var parts = wizardParts(wizard);
    // After a failed submit, open the first step with an error.
    var start = parts.panels.findIndex(function (p) { return p.querySelector('[aria-invalid="true"]'); });
    showStep(wizard, start < 0 ? 0 : start, start >= 0);
  }

  document.addEventListener("click", function (event) {
    var target = event.target.closest ? event.target : event.target.parentElement;
    if (!target) return;
    var next = target.closest("[data-rx-wizard-next]");
    if (next) { nextStep(next.closest("[data-rx-wizard]")); return; }
    var back = target.closest("[data-rx-wizard-back]");
    if (back) { var w = back.closest("[data-rx-wizard]"); showStep(w, Math.max(stepIndex(w) - 1, 0), true); }
  });
  // Enter in a field before the last step goes on, it doesn't send the form.
  document.addEventListener("keydown", function (event) {
    if (event.key !== "Enter" || event.defaultPrevented || event.isComposing) return;
    var el = event.target;
    var wizard = el.closest && el.closest("[data-rx-wizard][data-rx-ready]");
    if (!wizard || el.tagName === "TEXTAREA" || el.tagName === "BUTTON") return;
    if (stepIndex(wizard) < wizardParts(wizard).panels.length - 1) { event.preventDefault(); nextStep(wizard); }
  });
  document.addEventListener("submit", function (event) {
    var wizard = event.target.querySelector && event.target.querySelector("[data-rx-wizard][data-rx-ready]");
    if (wizard && stepIndex(wizard) < wizardParts(wizard).panels.length - 1) { event.preventDefault(); event.stopImmediatePropagation(); nextStep(wizard); }
  }, true);

  // Everything that sets itself up from the markup, on the page and in
  // what htmx swaps in.
  function setup(root) {
    root = root || document;
    var scope = root.querySelectorAll ? root : document;
    if (root.matches && root.matches("select[data-rx-combobox]")) enhanceSelect(root);
    scope.querySelectorAll("select[data-rx-combobox]").forEach(enhanceSelect);
    scope.querySelectorAll("[data-rx-repeater]").forEach(limits);
    scope.querySelectorAll("[data-rx-wizard]").forEach(setupWizard);
    applyAllWhen(scope);
  }

  // Toasts rendered with the page leave on their own too; conditional
  // groups take their state from the fields, here and in htmx swaps.
  function armAll() {
    document.querySelectorAll("[data-renox-toast]").forEach(arm);
    setup(document);
  }
  if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", armAll);
  else armAll();
  document.addEventListener("htmx:load", function (event) { setup(event.target); });
})();
