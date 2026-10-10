(function () {
  "use strict";
  if (window.RenoxLive) return;

  // "add(5)" -> { action: "add", args: [5] }; "save" -> { action: "save", args: [] }.
  function parseCall(text) {
    var m = /^\s*([A-Za-z][A-Za-z0-9_]*)\s*(?:\((.*)\))?\s*$/.exec(text || "");
    if (!m) return null;
    var args = [];
    if (m[2] !== undefined && m[2].trim() !== "") {
      try {
        args = JSON.parse("[" + m[2] + "]");
      } catch (e) {
        console.error("rx: bad arguments in " + JSON.stringify(text), e);
        return null;
      }
    }
    return { action: m[1], args: args };
  }

  // Sends an action to the component's route; the answer is morphed in
  // by the htmx:afterRequest listener below.
  function send(wrapper, action, args, extra) {
    var values = {
      _snapshot: wrapper.getAttribute("data-rx-snapshot"),
      _args: JSON.stringify(args)
    };
    if (extra) Object.keys(extra).forEach(function (k) { values[k] = extra[k]; });
    wrapper.setAttribute("aria-busy", "true");
    htmx.ajax(
      "POST",
      "/_renox/live/" + wrapper.getAttribute("data-rx-live") + "/" + action,
      { source: wrapper, values: values, swap: "none" }
    );
  }

  document.addEventListener("click", function (event) {
    var el = event.target.closest && event.target.closest("[rx-click]");
    if (!el) return;
    var wrapper = el.closest("[data-rx-live]");
    if (!wrapper) return;
    var call = parseCall(el.getAttribute("rx-click"));
    if (!call) return;
    event.preventDefault();
    send(wrapper, call.action, call.args);
  });

  // In the capture phase, and prevented, so the kit's busy-button submit
  // handler (renox-ui.js, which checks defaultPrevented) leaves it alone.
  document.addEventListener("submit", function (event) {
    var form = event.target;
    if (!form.matches || !form.matches("form[rx-submit]")) return;
    var wrapper = form.closest("[data-rx-live]");
    if (!wrapper) return;
    var call = parseCall(form.getAttribute("rx-submit"));
    if (!call) return;
    event.preventDefault();
    send(wrapper, call.action, call.args, Object.fromEntries(new FormData(form)));
  }, true);

  document.addEventListener("htmx:afterRequest", function (event) {
    var w = event.detail.elt;
    if (!w || !w.matches || !w.matches("[data-rx-live]")) return;
    w.removeAttribute("aria-busy");
    if (event.detail.xhr.status === 200) {
      Idiomorph.morph(w, event.detail.xhr.responseText, {
        morphStyle: "outerHTML",
        // Without it Idiomorph resets the focused field to the new HTML's value.
        ignoreActiveValue: true
      });
      htmx.process(w);
      // The kit's setup and its parts loader listen to htmx:load.
      w.dispatchEvent(new CustomEvent("htmx:load", { bubbles: true }));
    }
  });

  window.RenoxLive = { parseCall: parseCall, send: send };
})();
