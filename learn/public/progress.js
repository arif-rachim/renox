// learn.renox.rs: progress, quizzes and challenge marks, kept in this browser
// only (localStorage["renox-learn:v1"]). Nothing is sent anywhere, and the
// site reads the same without this script. Wrapped in a block: no globals.
{
  const KEY = "renox-learn:v1";
  const empty = () => ({ v: 1, stages: {}, quizzes: {}, challenges: {} });
  const now = () => new Date().toISOString();
  const isObject = (value) => value !== null && typeof value === "object" && !Array.isArray(value);

  // The parsed value, or an empty state for a missing, broken or foreign one.
  const load = () => {
    let raw = null;
    try {
      raw = localStorage.getItem(KEY);
    } catch {
      // storage blocked: keep what this page already holds
      return state;
    }
    try {
      const data = JSON.parse(raw);
      if (isObject(data) && data.v === 1) {
        for (const name of ["stages", "quizzes", "challenges"]) if (!isObject(data[name])) data[name] = {};
        return data;
      }
    } catch {
      // missing or not JSON: start empty
    }
    return empty();
  };

  // The in-memory copy: pages keep working when saving fails.
  let state = empty();
  state = load();

  const save = () => {
    try {
      localStorage.setItem(KEY, JSON.stringify(state));
    } catch {
      // blocked or full: give up silently
    }
  };

  // Changes the latest stored state (another tab may have saved) and saves it.
  const update = (change) => {
    state = load();
    change(state);
    save();
  };

  const hasProgress = () =>
    Object.keys(state.stages).length + Object.keys(state.quizzes).length + Object.keys(state.challenges).length > 0 ||
    Boolean(state.last);

  // ---- Quizzes ----
  const showQuiz = (form, choice, remember) => {
    const result = form.querySelector(".learn-quiz__result");
    const explain = form.querySelector(".learn-quiz__explain");
    const radios = [...form.querySelectorAll('input[type="radio"]')];
    form.classList.remove("is-correct", "is-wrong");
    if (choice === null) {
      result.textContent = "Choose an answer first.";
      return;
    }
    const correct = radios[choice] !== undefined && radios[choice].hasAttribute("data-correct");
    form.classList.add(correct ? "is-correct" : "is-wrong");
    result.textContent = correct ? "Correct." : "Not quite. Try again.";
    if (explain) explain.hidden = !correct;
    if (remember) {
      update((data) => {
        data.quizzes[form.dataset.quiz] = { choice, correct, at: now() };
      });
    }
  };

  const setUpQuizzes = () => {
    for (const form of document.querySelectorAll("form.learn-quiz[data-quiz]")) {
      const radios = [...form.querySelectorAll('input[type="radio"]')];
      const button = form.querySelector(".learn-quiz__check");
      if (button) button.disabled = false;
      form.addEventListener("submit", (event) => {
        event.preventDefault();
        const checked = radios.findIndex((radio) => radio.checked);
        showQuiz(form, checked < 0 ? null : checked, checked >= 0);
      });
      const stored = state.quizzes[form.dataset.quiz];
      if (isObject(stored) && Number.isInteger(stored.choice) && radios[stored.choice]) {
        radios[stored.choice].checked = true;
        showQuiz(form, stored.choice, false);
      }
    }
  };

  // ---- Challenges ----
  const markChallenge = (section, passed) => {
    section.classList.toggle("is-passed", passed);
    const button = section.querySelector("[data-challenge-done]");
    if (button) button.setAttribute("aria-pressed", String(passed));
  };

  const setUpChallenges = () => {
    for (const section of document.querySelectorAll("section.learn-challenge[data-challenge]")) {
      if (isObject(state.challenges[section.dataset.challenge])) markChallenge(section, true);
    }
    document.addEventListener("learn:challenge-done", (event) => {
      const detail = event.detail;
      if (!isObject(detail) || typeof detail.challenge !== "string") return;
      update((data) => {
        if (detail.done === true) data.challenges[detail.challenge] = { done: now() };
        else delete data.challenges[detail.challenge];
      });
    });
  };

  // ---- A stage page ----
  const setUpStage = (article) => {
    const slug = article.dataset.learnStage;
    update((data) => {
      const stage = isObject(data.stages[slug]) ? data.stages[slug] : {};
      stage.visited = now();
      data.stages[slug] = stage;
      data.last = { stage: slug, at: stage.visited };
    });

    const button = document.querySelector("[data-learn-done]");
    if (button) {
      const paint = () => {
        const done = isObject(state.stages[slug]) && Boolean(state.stages[slug].done);
        button.setAttribute("aria-pressed", String(done));
        button.textContent = done ? "Done (undo)" : "Mark this stage as done";
      };
      paint();
      button.hidden = false;
      button.addEventListener("click", () => {
        update((data) => {
          const stage = isObject(data.stages[slug]) ? data.stages[slug] : {};
          if (stage.done) delete stage.done;
          else stage.done = now();
          data.stages[slug] = stage;
        });
        paint();
      });
    }
    setUpQuizzes();
    setUpChallenges();
  };

  // ---- The home page ----
  const setUpHome = () => {
    const stages = [...document.querySelectorAll(".stage[id]")];
    const link = document.querySelector("[data-learn-continue]");
    const reset = document.querySelector("[data-learn-reset]");
    const status = document.querySelector("[data-learn-reset-status]");
    const body = (stage) => stage.querySelector(".stage__body");

    const paint = () => {
      for (const stage of stages) {
        const entry = state.stages[stage.id];
        const done = isObject(entry) && Boolean(entry.done);
        stage.classList.toggle("is-done", done);
        const badge = stage.querySelector(".stage__done");
        if (done && !badge && body(stage)) {
          const mark = document.createElement("span");
          mark.className = "stage__done rx-badge rx-badge--success";
          mark.textContent = "Done";
          const title = body(stage).querySelector(".stage__title");
          title.after(mark);
        } else if (!done && badge) {
          badge.remove();
        }
      }
      if (link) {
        const slug = state.last && state.last.stage;
        const anchor = typeof slug === "string" ? document.querySelector(`a[href="/stages/${CSS.escape(slug)}"]`) : null;
        link.hidden = !anchor;
        if (anchor) {
          const heading = anchor.closest(".stage__title").cloneNode(true);
          heading.querySelector(".visually-hidden, .rx-visually-hidden")?.remove();
          const number = anchor.closest(".stage").querySelector(".stage__number").textContent.trim();
          link.querySelector("a").href = `/stages/${slug}`;
          link.querySelector("span").textContent = `Stage ${Number(number)}: ${heading.textContent.trim()}`;
        }
      }
      if (reset) reset.hidden = !hasProgress();
    };
    paint();

    const confirm = document.querySelector("[data-learn-reset-confirm]");
    if (confirm) {
      confirm.addEventListener("click", () => {
        try {
          localStorage.removeItem(KEY);
        } catch {
          // blocked: the in-memory copy is cleared below
        }
        state = empty();
        paint();
        reset.open = false;
        if (status) status.textContent = "Your progress was cleared.";
      });
    }
  };

  const article = document.querySelector("article[data-learn-stage]");
  if (article) setUpStage(article);
  else if (document.querySelector(".stage[id]")) setUpHome();
}
