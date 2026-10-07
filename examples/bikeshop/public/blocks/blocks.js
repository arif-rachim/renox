// Bike shop blocks: the behaviour of resources/views/blocks/*.html (see
// /about/blocks). Plain DOM, set up from data-bs-* attributes on the page and
// in whatever htmx swaps in (htmx:load), so no inline handlers: it works under
// CSP=strict. Movement goes through Motion (motion.dev, loaded before this
// file as the global `Motion`) on `transform` only, and nothing moves under
// prefers-reduced-motion. Everything here is keyboard-usable.
(function () {
  "use strict";

  var reduce = window.matchMedia("(prefers-reduced-motion: reduce)");
  var EASE = [0.22, 1, 0.36, 1];
  var ISO = /^\d{4}-\d{2}-\d{2}$/;

  /** Whether to skip animations: asked for less motion, or no Motion. */
  function still() {
    return reduce.matches || !window.Motion;
  }

  function animate(el, keyframes, options) {
    if (still()) return null;
    return window.Motion.animate(el, keyframes, options);
  }

  function fire(el, type) {
    el.dispatchEvent(new Event(type, { bubbles: true }));
  }

  /** The texts a block carries in data-bs-texts (translated by the server). */
  function texts(el) {
    try {
      return JSON.parse(el.getAttribute("data-bs-texts") || "{}");
    } catch (_) {
      return {};
    }
  }

  /** "Moved :card" with {card: "Tune-up"} → "Moved Tune-up". */
  function fill(text, values) {
    return String(text || "").replace(/:(\w+)/g, function (match, key) {
      return Object.prototype.hasOwnProperty.call(values, key) ? values[key] : match;
    });
  }

  /** Runs `init` once for each element matching `selector` in `root`. */
  function each(root, selector, init) {
    if (root.matches && root.matches(selector)) init(root);
    root.querySelectorAll(selector).forEach(init);
  }

  /** Moves `el` from where `before` (a DOMRect) was to where it is now. */
  function glide(el, before) {
    if (!before || still()) return;
    var after = el.getBoundingClientRect();
    var dx = before.left - after.left;
    var dy = before.top - after.top;
    if (!dx && !dy) return;
    animate(
      el,
      { transform: ["translate(" + dx + "px, " + dy + "px)", "translate(0px, 0px)"] },
      { duration: 0.22, ease: EASE }
    );
  }

  // ---------- gallery ----------

  function setupGallery(gallery) {
    if (gallery.hasAttribute("data-bs-ready")) return;
    var track = gallery.querySelector("[data-bs-gallery-track]");
    var viewport = gallery.querySelector("[data-bs-gallery-viewport]");
    var slides = track ? Array.prototype.slice.call(track.children) : [];
    if (!slides.length) return;
    gallery.setAttribute("data-bs-ready", "");
    var prev = gallery.querySelector("[data-bs-gallery-prev]");
    var next = gallery.querySelector("[data-bs-gallery-next]");
    var status = gallery.querySelector("[data-bs-gallery-status]");
    var thumbs = gallery.querySelectorAll("[data-bs-gallery-thumb]");
    if (prev) prev.hidden = false;
    if (next) next.hidden = false;
    viewport.scrollLeft = 0;
    var index = 0;

    // At rest the track sits at the current photo, in pixels (as Motion
    // leaves it), and follows the gallery's width when the window changes.
    function settle() {
      track.style.transform = "translateX(" + -index * viewport.clientWidth + "px)";
    }
    window.addEventListener("resize", settle);

    function show(target, from) {
      index = Math.max(0, Math.min(slides.length - 1, target));
      var to = -index * viewport.clientWidth;
      var start = from === undefined ? null : from;
      if (start === null) {
        // Where the track is now, in pixels.
        var matrix = new DOMMatrixReadOnly(getComputedStyle(track).transform);
        start = matrix.m41;
      }
      if (!animate(track, { transform: ["translateX(" + start + "px)", "translateX(" + to + "px)"] }, { duration: 0.42, ease: EASE })) {
        settle();
      }
      slides.forEach(function (slide, n) {
        var current = n === index;
        slide.toggleAttribute("inert", !current);
        if (current) slide.removeAttribute("aria-hidden");
        else slide.setAttribute("aria-hidden", "true");
      });
      thumbs.forEach(function (thumb, n) {
        if (n === index) thumb.setAttribute("aria-current", "true");
        else thumb.removeAttribute("aria-current");
      });
      if (prev) prev.disabled = index === 0;
      if (next) next.disabled = index === slides.length - 1;
      if (status) {
        var slide = slides[index];
        status.textContent = slide.getAttribute("aria-label") + ": " + slide.getAttribute("data-alt");
      }
      gallery.setAttribute("data-bs-index", String(index));
    }

    gallery.addEventListener("click", function (event) {
      var thumb = event.target.closest("[data-bs-gallery-thumb]");
      if (thumb) {
        event.preventDefault();
        show(parseInt(thumb.getAttribute("data-bs-gallery-thumb"), 10));
        return;
      }
      if (event.target.closest("[data-bs-gallery-prev]")) show(index - 1);
      else if (event.target.closest("[data-bs-gallery-next]")) show(index + 1);
      else if (event.target.closest("[data-bs-gallery-enlarge]")) {
        // The kit opens the sheet (data-rx-open); it shows the current photo.
        var sheet = document.getElementById(gallery.id + "-zoom");
        var slide = slides[index];
        if (!sheet) return;
        var image = sheet.querySelector("[data-bs-gallery-zoom]");
        var caption = sheet.querySelector("[data-bs-gallery-zoom-caption]");
        if (image) {
          image.src = slide.getAttribute("data-large");
          image.alt = slide.getAttribute("data-alt");
        }
        if (caption) caption.textContent = slide.getAttribute("data-caption") || "";
        if (image && !still()) animate(image, { opacity: [0, 1], transform: ["scale(0.96)", "scale(1)"] }, { duration: 0.3, ease: EASE });
      }
    });

    gallery.addEventListener("keydown", function (event) {
      var keys = { ArrowLeft: index - 1, ArrowRight: index + 1, Home: 0, End: slides.length - 1 };
      if (!(event.key in keys) || event.altKey || event.ctrlKey || event.metaKey) return;
      event.preventDefault();
      show(keys[event.key]);
      // A thumbnail with the focus follows the photo.
      if (document.activeElement && document.activeElement.hasAttribute("data-bs-gallery-thumb") && thumbs[index]) {
        thumbs[index].focus();
      }
    });

    // Swipes and drags: the track follows the finger, and a pull of a fifth
    // of the width (or a flick) goes to the next photo.
    var drag = null;
    viewport.addEventListener("pointerdown", function (event) {
      if (event.button !== 0) return;
      drag = { id: event.pointerId, x: event.clientX, at: -index * viewport.clientWidth, dx: 0, time: Date.now(), moved: false };
    });
    viewport.addEventListener("pointermove", function (event) {
      if (!drag || event.pointerId !== drag.id) return;
      drag.dx = event.clientX - drag.x;
      if (!drag.moved && Math.abs(drag.dx) < 6) return;
      if (!drag.moved) {
        drag.moved = true;
        try {
          viewport.setPointerCapture(event.pointerId);
        } catch (_) {}
      }
      var edge = (index === 0 && drag.dx > 0) || (index === slides.length - 1 && drag.dx < 0);
      track.style.transform = "translateX(" + (drag.at + (edge ? drag.dx / 3 : drag.dx)) + "px)";
    });
    function release(event) {
      if (!drag || event.pointerId !== drag.id) return;
      var done = drag;
      drag = null;
      if (!done.moved) return;
      var width = viewport.clientWidth;
      var fast = Math.abs(done.dx) / Math.max(1, Date.now() - done.time) > 0.5;
      var step = Math.abs(done.dx) > width / 5 || fast ? (done.dx < 0 ? 1 : -1) : 0;
      show(index + step, done.at + done.dx);
    }
    viewport.addEventListener("pointerup", release);
    viewport.addEventListener("pointercancel", release);

    show(0, 0);
  }

  // ---------- range_slider ----------

  /** How a range shows its values: money (from the server's "0" in that
   *  currency, e.g. "Rp 0" or "$0.00"), a number, or as it is. */
  function formatter(field) {
    var kind = field.getAttribute("data-bs-format");
    var locale = field.getAttribute("data-bs-locale") || undefined;
    var prefix = field.getAttribute("data-bs-prefix") || "";
    var suffix = field.getAttribute("data-bs-suffix") || "";
    function numbers(decimals) {
      var options = { minimumFractionDigits: decimals, maximumFractionDigits: decimals };
      try {
        return new Intl.NumberFormat(locale, Object.assign({ useGrouping: "always" }, options));
      } catch (_) {
        return new Intl.NumberFormat(locale, options);
      }
    }
    if (kind === "money") {
      var unit = field.getAttribute("data-bs-unit") || "0";
      var zero = (unit.match(/0[.,]?0*/) || ["0"])[0];
      var decimals = zero.length > 1 ? zero.length - 2 : 0;
      var money = numbers(decimals);
      return function (value) {
        return unit.replace(zero, money.format(value));
      };
    }
    if (kind === "number") {
      var plain = numbers(0);
      return function (value) {
        return plain.format(value);
      };
    }
    return function (value) {
      return prefix + value + suffix;
    };
  }

  function setupRange(field) {
    if (field.hasAttribute("data-bs-ready")) return;
    var low = field.querySelector('[data-bs-range-input="min"]');
    var high = field.querySelector('[data-bs-range-input="max"]');
    var track = field.querySelector("[data-bs-range-track]");
    if (!low || !high || !track) return;
    field.setAttribute("data-bs-ready", "");
    var format = formatter(field);
    var min = parseFloat(low.min);
    var max = parseFloat(low.max);
    var span = max - min || 1;

    function update(moved) {
      var a = parseFloat(low.value);
      var b = parseFloat(high.value);
      // The handles never cross: the one moving stops at the other.
      if (a > b) {
        if (moved === high) high.value = b = a;
        else low.value = a = b;
      }
      track.style.setProperty("--bs-from", String((a - min) / span));
      track.style.setProperty("--bs-to", String((b - min) / span));
      // When both sit at the top end, the lower handle must be the one on top
      // (else it couldn't be dragged down again).
      low.style.zIndex = a === b && b === max ? "2" : "";
      [
        [low, a, "min"],
        [high, b, "max"],
      ].forEach(function (part) {
        var text = format(part[1]);
        part[0].setAttribute("aria-valuetext", text);
        var shown = field.querySelector('[data-bs-range-shown="' + part[2] + '"]');
        if (shown) shown.textContent = text;
      });
    }
    low.addEventListener("input", function () {
      update(low);
    });
    high.addEventListener("input", function () {
      update(high);
    });
    update(null);
  }

  // ---------- quantity ----------

  function quantityLimits(box) {
    var input = box.querySelector("input");
    if (!input) return;
    var value = parseFloat(input.value);
    var min = input.min === "" ? -Infinity : parseFloat(input.min);
    var max = input.max === "" ? Infinity : parseFloat(input.max);
    box.querySelectorAll("[data-bs-step]").forEach(function (button) {
      var up = button.getAttribute("data-bs-step") === "1";
      button.disabled = !isNaN(value) && (up ? value >= max : value <= min);
    });
  }

  document.addEventListener("click", function (event) {
    var button = event.target.closest && event.target.closest("[data-bs-quantity] [data-bs-step]");
    if (!button) return;
    var box = button.closest("[data-bs-quantity]");
    var input = box.querySelector("input");
    var step = parseFloat(input.step) || 1;
    var min = input.min === "" ? -Infinity : parseFloat(input.min);
    var max = input.max === "" ? Infinity : parseFloat(input.max);
    var current = parseFloat(input.value);
    if (isNaN(current)) current = isFinite(min) ? min : 0;
    var value = Math.min(max, Math.max(min, current + step * parseFloat(button.getAttribute("data-bs-step"))));
    if (value === current && input.value !== "") return;
    input.value = String(value);
    fire(input, "input");
    fire(input, "change");
    var shown = animate(input, { transform: ["scale(1.12)", "scale(1)"] }, { duration: 0.18, ease: EASE });
    if (shown) shown.then(function () { input.style.transform = ""; });
  });
  document.addEventListener("input", function (event) {
    var box = event.target.closest && event.target.closest("[data-bs-quantity]");
    if (box) quantityLimits(box);
  });

  // ---------- keypad ----------

  /** The page language's decimal separator ("." or ","). */
  function decimalSeparator(locale) {
    try {
      var part = new Intl.NumberFormat(locale).formatToParts(1.5).find(function (p) {
        return p.type === "decimal";
      });
      return part ? part.value : ".";
    } catch (_) {
      return ".";
    }
  }

  function setupKeypad(pad) {
    if (pad.hasAttribute("data-bs-ready")) return;
    pad.setAttribute("data-bs-ready", "");
    var separator = decimalSeparator(pad.getAttribute("data-bs-locale") || undefined);
    pad.querySelectorAll("[data-bs-decimal]").forEach(function (el) {
      el.textContent = separator;
    });
    pad._bsSeparator = separator;
  }

  function pressKey(pad, key) {
    var target = document.getElementById(pad.getAttribute("data-bs-keypad"));
    if (!target) return;
    var button = pad.querySelector('[data-bs-key="' + key + '"]');
    if (button) {
      button.setAttribute("data-bs-pressed", "");
      setTimeout(function () {
        button.removeAttribute("data-bs-pressed");
      }, 120);
    }
    if (key === "enter") {
      if (target.form) {
        if (target.form.requestSubmit) target.form.requestSubmit();
        else target.form.submit();
      }
      return;
    }
    var value = target.value;
    var separator = pad._bsSeparator || ".";
    if (key === "backspace") value = value.slice(0, -1);
    else if (key === "clear") value = "";
    else if (key === "decimal") {
      if (value.indexOf(separator) === -1) value = (value || "0") + separator;
    } else if (/^\d+$/.test(key)) {
      value = value === "0" ? key.replace(/^0+(?=\d)/, "") || "0" : value + key;
    }
    var limit = parseInt(target.getAttribute("maxlength"), 10);
    if (limit > 0) value = value.slice(0, limit);
    if (value === target.value) return;
    target.value = value;
    fire(target, "input");
    fire(target, "change");
  }

  /** The key next to `from` in a direction, by where the keys are drawn. */
  function nearestKey(pad, from, dx, dy) {
    var a = from.getBoundingClientRect();
    var ax = a.left + a.width / 2;
    var ay = a.top + a.height / 2;
    var best = null;
    var bestScore = Infinity;
    pad.querySelectorAll("[data-bs-key]").forEach(function (key) {
      if (key === from) return;
      var b = key.getBoundingClientRect();
      var x = b.left + b.width / 2 - ax;
      var y = b.top + b.height / 2 - ay;
      var along = x * dx + y * dy;
      if (along <= 4) return;
      var across = Math.abs(x * dy) + Math.abs(y * dx);
      var score = along + across * 3;
      if (score < bestScore) {
        bestScore = score;
        best = key;
      }
    });
    return best;
  }

  function focusKey(pad, key) {
    pad.querySelectorAll("[data-bs-key]").forEach(function (k) {
      k.tabIndex = k === key ? 0 : -1;
    });
    key.focus();
  }

  document.addEventListener("click", function (event) {
    var key = event.target.closest && event.target.closest("[data-bs-keypad] [data-bs-key]");
    if (!key) return;
    var pad = key.closest("[data-bs-keypad]");
    focusKey(pad, key);
    pressKey(pad, key.getAttribute("data-bs-key"));
  });

  document.addEventListener("keydown", function (event) {
    var pad = event.target.closest && event.target.closest("[data-bs-keypad]");
    if (!pad || event.ctrlKey || event.metaKey || event.altKey) return;
    var keys = pad.querySelectorAll("[data-bs-key]");
    var current = event.target.closest("[data-bs-key]") || keys[0];
    var moves = { ArrowLeft: [-1, 0], ArrowRight: [1, 0], ArrowUp: [0, -1], ArrowDown: [0, 1] };
    var next = null;
    if (moves[event.key]) next = nearestKey(pad, current, moves[event.key][0], moves[event.key][1]);
    else if (event.key === "Home") next = keys[0];
    else if (event.key === "End") next = keys[keys.length - 1];
    else if (/^[0-9]$/.test(event.key)) pressKey(pad, event.key);
    else if (event.key === "Backspace") pressKey(pad, "backspace");
    else if (event.key === "Delete") pressKey(pad, "clear");
    else if ((event.key === "." || event.key === ",") && pad.querySelector('[data-bs-key="decimal"]')) pressKey(pad, "decimal");
    else return; // Enter and Space press the focused key, as on any button.
    event.preventDefault();
    if (next) focusKey(pad, next);
  });

  // ---------- kanban ----------

  function cardsOf(list) {
    return Array.prototype.filter.call(list.children, function (el) {
      return el.hasAttribute("data-bs-card");
    });
  }

  /** Where a card is: its column, its title, its place (from 1) of how many. */
  function placeOf(card) {
    var column = card.closest("[data-bs-column]");
    var list = card.parentElement;
    var cards = cardsOf(list);
    var title = card.querySelector("[data-bs-card-title]");
    return {
      card: title ? title.textContent.trim() : card.getAttribute("data-bs-card"),
      column: column ? column.getAttribute("data-bs-column-title") : "",
      key: column ? column.getAttribute("data-bs-column") : "",
      position: cards.indexOf(card) + 1,
      total: cards.length,
    };
  }

  function announce(board, text) {
    var live = board.querySelector("[data-bs-kanban-live]");
    if (live) live.textContent = text;
  }

  function counts(board) {
    board.querySelectorAll("[data-bs-column]").forEach(function (column) {
      var badge = column.querySelector("[data-bs-column-count]");
      var list = column.querySelector("[data-bs-kanban-list]");
      if (badge && list) badge.textContent = String(cardsOf(list).length);
    });
  }

  function prepareCards(board) {
    var help = board.getAttribute("aria-describedby");
    board.querySelectorAll("[data-bs-card]").forEach(function (card) {
      if (card.hasAttribute("data-bs-ready")) return;
      card.setAttribute("data-bs-ready", "");
      card.tabIndex = 0;
      if (help) card.setAttribute("aria-describedby", help);
    });
  }

  function setupKanban(board) {
    prepareCards(board);
    if (board.hasAttribute("data-bs-ready")) return;
    board.setAttribute("data-bs-ready", "");
    board._bsPending = [];
    var form = board.querySelector("[data-bs-kanban-form]");
    if (!form) return;
    // The server's answer: keep the move, or put the card back.
    form.addEventListener("htmx:afterRequest", function (event) {
      var move = board._bsPending.shift();
      if (!move) return;
      var ok = !!event.detail.successful;
      if (!ok) {
        var before = move.card.getBoundingClientRect();
        restore(move);
        glide(move.card, before);
        counts(board);
        announce(board, fill(texts(board).failed, placeOf(move.card)));
      }
      board.dispatchEvent(
        new CustomEvent("bs:kanban-moved", {
          bubbles: true,
          detail: { card: move.id, column: move.column, position: move.position, ok: ok },
        })
      );
    });
  }

  function restore(move) {
    var next = move.origin.next;
    if (next && next.parentElement === move.origin.list) move.origin.list.insertBefore(move.card, next);
    else move.origin.list.appendChild(move.card);
  }

  /** The card was dropped: tell the server where, unless it didn't move. */
  function commit(board, card, origin) {
    counts(board);
    var place = placeOf(card);
    announce(board, fill(texts(board).dropped, place));
    if (card.parentElement === origin.list && card.nextElementSibling === origin.next) return;
    var form = board.querySelector("[data-bs-kanban-form]");
    if (!form || !window.htmx) return;
    var move = {
      card: card,
      origin: origin,
      id: card.getAttribute("data-bs-card"),
      column: place.key,
      position: place.position - 1,
    };
    form.elements.card.value = move.id;
    form.elements.column.value = move.column;
    form.elements.position.value = String(move.position);
    board._bsPending.push(move);
    window.htmx.trigger(form, "bs:move");
  }

  // The keyboard: Space/Enter picks up and drops, arrows move, Escape cancels.
  var grabbed = null;

  function release(card) {
    card.removeAttribute("aria-grabbed");
    grabbed = null;
  }

  document.addEventListener("keydown", function (event) {
    var card = event.target.closest && event.target.closest("[data-bs-card]");
    var board = card && card.closest("[data-bs-kanban][data-bs-ready]");
    if (!board || event.target !== card || event.ctrlKey || event.metaKey || event.altKey) return;
    var t = texts(board);
    var list = card.parentElement;
    var lists = Array.prototype.slice.call(board.querySelectorAll("[data-bs-kanban-list]"));
    var column = lists.indexOf(list);
    var siblings = cardsOf(list);
    var at = siblings.indexOf(card);

    if (event.key === " " || event.key === "Enter") {
      event.preventDefault();
      if (grabbed && grabbed.card === card) {
        var origin = grabbed.origin;
        release(card);
        commit(board, card, origin);
      } else {
        if (grabbed) {
          restore(grabbed);
          release(grabbed.card);
        }
        grabbed = { card: card, board: board, origin: { list: list, next: card.nextElementSibling } };
        card.setAttribute("aria-grabbed", "true");
        announce(board, fill(t.picked, placeOf(card)));
      }
      return;
    }

    var moves = { ArrowUp: true, ArrowDown: true, ArrowLeft: true, ArrowRight: true };
    if (grabbed && grabbed.card === card && event.key === "Escape") {
      event.preventDefault();
      var back = card.getBoundingClientRect();
      restore(grabbed);
      release(card);
      card.focus();
      glide(card, back);
      counts(board);
      announce(board, fill(t.cancelled, placeOf(card)));
      return;
    }
    if (!moves[event.key]) return;
    event.preventDefault();

    if (!(grabbed && grabbed.card === card)) {
      // Not holding it: the arrows move the focus between cards.
      var target = null;
      if (event.key === "ArrowUp") target = siblings[at - 1];
      else if (event.key === "ArrowDown") target = siblings[at + 1];
      else {
        var other = lists[column + (event.key === "ArrowLeft" ? -1 : 1)];
        if (other) {
          var cards = cardsOf(other);
          target = cards[Math.min(at, cards.length - 1)];
        }
      }
      if (target) target.focus();
      return;
    }

    // Holding it: the arrows move the card.
    var before = card.getBoundingClientRect();
    var moved = false;
    if (event.key === "ArrowUp" && at > 0) {
      list.insertBefore(card, siblings[at - 1]);
      moved = true;
    } else if (event.key === "ArrowDown" && at < siblings.length - 1) {
      list.insertBefore(card, siblings[at + 1].nextSibling);
      moved = true;
    } else if (event.key === "ArrowLeft" || event.key === "ArrowRight") {
      var to = lists[column + (event.key === "ArrowLeft" ? -1 : 1)];
      if (to) {
        var there = cardsOf(to);
        var spot = there[Math.min(at, there.length)];
        if (spot) to.insertBefore(card, spot);
        else to.appendChild(card);
        moved = true;
      }
    }
    if (!moved) return;
    card.focus();
    if (card.scrollIntoView) card.scrollIntoView({ block: "nearest", inline: "nearest" });
    glide(card, before);
    counts(board);
    announce(board, fill(t.moved, placeOf(card)));
  });

  // A pointer: drag a card (a touch drags it by its handle, so the page
  // still scrolls), a gap opens where it would land.
  var drag = null;

  document.addEventListener("pointerdown", function (event) {
    var card = event.target.closest && event.target.closest("[data-bs-card]");
    var board = card && card.closest("[data-bs-kanban][data-bs-ready]");
    if (!board || event.button !== 0 || event.target.closest("a, button, input, select, textarea")) return;
    if (event.pointerType !== "mouse" && !event.target.closest(".bs-kanban__grip")) return;
    drag = {
      card: card,
      board: board,
      id: event.pointerId,
      x: event.clientX,
      y: event.clientY,
      started: false,
      origin: { list: card.parentElement, next: card.nextElementSibling },
    };
  });

  function startDrag() {
    var card = drag.card;
    var place = placeOf(card);
    var box = card.getBoundingClientRect();
    drag.box = box;
    drag.gap = document.createElement("li");
    drag.gap.className = "bs-kanban__gap";
    drag.gap.setAttribute("aria-hidden", "true");
    drag.gap.style.height = box.height + "px";
    card.parentElement.insertBefore(drag.gap, card);
    card.setAttribute("data-bs-dragging", "");
    card.style.position = "fixed";
    card.style.left = box.left + "px";
    card.style.top = box.top + "px";
    card.style.width = box.width + "px";
    card.style.margin = "0";
    // Out of the board while it moves, so nothing the board is inside of
    // (a scrolling frame, a transform) clips it or shifts it.
    document.body.appendChild(card);
    drag.started = true;
    if (grabbed) {
      restore(grabbed);
      release(grabbed.card);
    }
    announce(drag.board, fill(texts(drag.board).picked, place));
  }

  document.addEventListener("pointermove", function (event) {
    if (!drag || event.pointerId !== drag.id) return;
    var dx = event.clientX - drag.x;
    var dy = event.clientY - drag.y;
    if (!drag.started) {
      if (Math.abs(dx) + Math.abs(dy) < 6) return;
      startDrag();
    }
    event.preventDefault();
    drag.card.style.transform = "translate(" + dx + "px, " + dy + "px)";
    // The column under the pointer, and the card the gap goes before.
    var under = document.elementsFromPoint(event.clientX, event.clientY);
    var column = null;
    for (var i = 0; i < under.length; i++) {
      if (drag.card.contains(under[i])) continue;
      column = under[i].closest("[data-bs-column]");
      if (column) break;
    }
    drag.board.querySelectorAll("[data-bs-over]").forEach(function (c) {
      if (c !== column) c.removeAttribute("data-bs-over");
    });
    if (!column || !drag.board.contains(column)) return;
    column.setAttribute("data-bs-over", "");
    var list = column.querySelector("[data-bs-kanban-list]");
    var spot = null;
    cardsOf(list).some(function (card) {
      var r = card.getBoundingClientRect();
      if (event.clientY < r.top + r.height / 2) {
        spot = card;
        return true;
      }
      return false;
    });
    if (spot !== drag.gap.nextElementSibling || list !== drag.gap.parentElement) {
      if (spot) list.insertBefore(drag.gap, spot);
      else list.appendChild(drag.gap);
    }
    // Near the board's edges, it scrolls (columns off screen on a phone).
    var edge = drag.board.getBoundingClientRect();
    if (event.clientX > edge.right - 40) drag.board.scrollLeft += 12;
    else if (event.clientX < edge.left + 40) drag.board.scrollLeft -= 12;
  });

  function endDrag(event, cancelled) {
    if (!drag || event.pointerId !== drag.id) return;
    var done = drag;
    drag = null;
    if (!done.started) return;
    var card = done.card;
    var before = card.getBoundingClientRect();
    done.board.querySelectorAll("[data-bs-over]").forEach(function (c) {
      c.removeAttribute("data-bs-over");
    });
    if (cancelled) {
      done.gap.remove();
      restore({ card: card, origin: done.origin });
    } else {
      done.gap.parentElement.replaceChild(card, done.gap);
    }
    card.removeAttribute("data-bs-dragging");
    ["position", "left", "top", "width", "margin", "transform"].forEach(function (p) {
      card.style[p] = "";
    });
    glide(card, before);
    card.focus({ preventScroll: true });
    if (cancelled) {
      counts(done.board);
      announce(done.board, fill(texts(done.board).cancelled, placeOf(card)));
    } else {
      commit(done.board, card, done.origin);
    }
  }
  document.addEventListener("pointerup", function (event) {
    endDrag(event, false);
  });
  document.addEventListener("pointercancel", function (event) {
    endDrag(event, true);
  });

  // ---------- datetime_range ----------

  function pad2(n) {
    return (n < 10 ? "0" : "") + n;
  }

  function durationText(t, minutes) {
    var days = Math.floor(minutes / 1440);
    var hours = Math.floor((minutes % 1440) / 60);
    var rest = minutes % 60;
    var parts = [];
    if (days) parts.push(fill(days === 1 ? t.day : t.days, { n: days }));
    if (hours) parts.push(fill(hours === 1 ? t.hour : t.hours, { n: hours }));
    if (rest) parts.push(fill(t.minutes, { n: rest }));
    return parts.join(" ");
  }

  function updateDatetime(field) {
    var t = texts(field);
    var at = {};
    ["start", "end"].forEach(function (which) {
      var date = field.querySelector('[data-bs-dt-date="' + which + '"]');
      var time = field.querySelector('[data-bs-dt-time="' + which + '"]');
      var hidden = field.querySelector('[data-bs-dt-value="' + which + '"]');
      var day = date ? date.value.trim() : "";
      var value = ISO.test(day) && time && time.value ? day + "T" + time.value : "";
      if (hidden && hidden.value !== value) {
        hidden.value = value;
        fire(hidden, "change");
      }
      if (value) {
        at[which] = new Date(+day.slice(0, 4), +day.slice(5, 7) - 1, +day.slice(8, 10), +time.value.slice(0, 2), +time.value.slice(3, 5));
      }
    });
    // The end's calendar starts at the start's day.
    var startDay = field.querySelector('[data-bs-dt-date="start"]');
    var endCalendar = field.querySelector('[data-bs-dt-end="end"] calendar-date');
    if (startDay && endCalendar) {
      if (!endCalendar.hasAttribute("data-bs-min")) endCalendar.setAttribute("data-bs-min", endCalendar.getAttribute("min") || "");
      var floor = endCalendar.getAttribute("data-bs-min");
      var chosen = startDay.value.trim();
      endCalendar.setAttribute("min", ISO.test(chosen) && chosen > floor ? chosen : floor);
    }
    var summary = field.querySelector("[data-bs-dt-summary]");
    var endTime = field.querySelector('[data-bs-dt-time="end"]');
    if (!summary) return;
    if (at.start && at.end) {
      var minutes = Math.round((at.end - at.start) / 60000);
      if (minutes <= 0) {
        summary.textContent = t.order || "";
        summary.setAttribute("data-bs-invalid", "");
        if (endTime) endTime.setAttribute("aria-invalid", "true");
        return;
      }
      summary.textContent = durationText(t, minutes);
    } else {
      summary.textContent = "";
    }
    summary.removeAttribute("data-bs-invalid");
    if (endTime) endTime.removeAttribute("aria-invalid");
  }

  function setupDatetime(field) {
    if (field.hasAttribute("data-bs-ready")) return;
    field.setAttribute("data-bs-ready", "");
    field.addEventListener("change", function () {
      updateDatetime(field);
    });
    field.addEventListener("input", function (event) {
      if (!event.target.hasAttribute("data-bs-dt-value")) updateDatetime(field);
    });
    updateDatetime(field);
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
    each(root, "[data-bs-gallery]", setupGallery);
    each(root, "[data-bs-range]", setupRange);
    each(root, "[data-bs-quantity]", quantityLimits);
    each(root, "[data-bs-keypad]", setupKeypad);
    each(root, "[data-bs-kanban]", setupKanban);
    // Cards swapped into a board that is already set up.
    if (root.closest) {
      var board = root.closest("[data-bs-kanban]");
      if (board) prepareCards(board);
    }
    each(root, "[data-bs-datetime]", setupDatetime);
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
