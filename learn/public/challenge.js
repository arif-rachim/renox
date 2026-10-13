// learn.renox.rs: the "I passed it" button of a challenge. It toggles
// aria-pressed and tells the page with `learn:challenge-done`; progress.js
// stores the mark. Without JavaScript the button stays hidden.
(() => {
  for (const section of document.querySelectorAll("section.learn-challenge[data-challenge]")) {
    const button = section.querySelector("[data-challenge-done]");
    if (!button) continue;
    button.hidden = false;
    button.addEventListener("click", () => {
      const done = button.getAttribute("aria-pressed") !== "true";
      button.setAttribute("aria-pressed", String(done));
      section.classList.toggle("is-passed", done);
      document.dispatchEvent(
        new CustomEvent("learn:challenge-done", {
          detail: { challenge: section.dataset.challenge, done },
        }),
      );
    });
  }
})();
