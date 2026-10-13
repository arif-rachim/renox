// learn.renox.rs's code editor (#528): CodeMirror 6 from the vendored bundle
// (vendor/codemirror-*.js sets window.CM), in Rust or HTML mode. The HTML mode
// marks Renox 1.1 templates: `{{ … }}`, `{# … #}` and `{% … %}` (cm-rx-expr)
// and `rx-*` / `app-*` / `live-*` tag names (cm-rx-tag).
//
// window.LearnEditor:
//   mount(el, { language: "rust" | "html", doc, path, readOnly, onChange, stage }) → editor
//   editors: a Map of the mounted editors, keyed by path
//   loadCompletions(stage) → a promise of { api, stage } from /editor/data/
// An editor: getValue(), setValue(text), focus(), reveal(line, col) (1-based),
// destroy(), setCompletions({ api, stage }), setDiagnostics(list), view, path,
// language.
//
// Every textarea[data-learn-editor="rust|html"] (data-path gives the path,
// data-stage the course stage whose models and tags it suggests) is mounted on
// load and after an htmx swap, once. The textarea is hidden and kept in step,
// so a form still posts its value.
//
// Autocomplete (#529) comes from Renox's API (renox-api.json, read from its
// source) and the stage's code (stage-NN.json: models, typed columns, tags).
// It is not type inference: `Type::`, a `.` after a chain of known calls,
// `renox::` paths, prelude names, `#[derive(` and, in HTML, tags, attributes
// and their values.
//
// CSP: this file and the bundle are same-origin scripts (no inline script, no
// eval). CodeMirror injects its styles (style-mod), which Renox's strict
// policy allows through `style-src 'unsafe-inline'`; if that ever goes, pass a
// nonce with EditorView.cspNonce.
{
  const CM = window.CM;

  // Tokens get the classes of the docs' highlighter (learn.css colours them),
  // so the editor and the code panels look the same.
  const t = CM.tags;
  const highlight = CM.HighlightStyle.define([
    { tag: [t.comment, t.lineComment, t.blockComment, t.docComment], class: "hl-com" },
    { tag: [t.keyword, t.controlKeyword, t.operatorKeyword, t.definitionKeyword, t.moduleKeyword, t.modifier, t.self], class: "hl-kw" },
    { tag: [t.string, t.special(t.string), t.character, t.attributeValue, t.regexp], class: "hl-str" },
    { tag: [t.number, t.bool, t.null, t.atom], class: "hl-num" },
    { tag: [t.function(t.variableName), t.function(t.definition(t.variableName)), t.function(t.propertyName)], class: "hl-fn" },
    { tag: [t.typeName, t.className, t.namespace, t.standard(t.typeName)], class: "hl-ty" },
    { tag: [t.macroName, t.special(t.variableName)], class: "hl-mac" },
    { tag: [t.attributeName, t.meta, t.annotation], class: "hl-attr" },
    { tag: [t.tagName, t.angleBracket], class: "hl-tag" },
    { tag: [t.propertyName], class: "hl-prop" },
    { tag: [t.labelName], class: "hl-var" },
    { tag: [t.processingInstruction, t.documentMeta], class: "hl-tpl" },
  ]);

  // The HTML mode's marks for Renox's template syntax.
  const exprMark = CM.Decoration.mark({ class: "cm-rx-expr" });
  const tagMark = CM.Decoration.mark({ class: "cm-rx-tag" });
  const templateMarks = (decorator) =>
    CM.ViewPlugin.fromClass(
      class {
        constructor(view) {
          this.marks = decorator.createDeco(view);
        }
        update(update) {
          this.marks = decorator.updateDeco(update, this.marks);
        }
      },
      { decorations: (plugin) => plugin.marks },
    );
  const exprs = templateMarks(
    new CM.MatchDecorator({ regexp: /\{\{.*?\}\}|\{#.*?#\}|\{%.*?%\}/g, decoration: () => exprMark }),
  );
  const tagNames = templateMarks(
    new CM.MatchDecorator({
      regexp: /<\/?((?:rx|app|live)-[a-z0-9-]+)/g,
      decorate: (add, from, _to, match) => {
        const start = from + match[0].indexOf(match[1]);
        add(start, start + match[1].length, tagMark);
      },
    }),
  );

  // ---- Autocomplete (#529) ----

  // The data files, fetched once per page.
  const fetched = new Map();
  const getJson = (url) => {
    if (!fetched.has(url)) {
      fetched.set(
        url,
        fetch(url).then((r) => {
          if (!r.ok) throw new Error(`${url}: ${r.status}`);
          return r.json();
        }),
      );
    }
    return fetched.get(url);
  };
  const loadCompletions = (stage) =>
    Promise.all([getJson("/editor/data/renox-api.json"), getJson(`/editor/data/stage-${String(stage).padStart(2, "0")}.json`)]).then(
      ([api, stageData]) => ({ api, stage: stageData }),
    );

  const KIND_TYPE = {
    struct: "class", enum: "class", trait: "class", type: "class", derive: "class", attribute: "keyword",
    fn: "function", macro: "function", method: "method", const: "constant", static: "constant",
    variant: "constant", mod: "keyword", reexport: "class",
  };
  const STD_DERIVES = ["Debug", "Clone", "Default", "PartialEq", "Serialize", "Deserialize"];

  // Lookups over one data set, built once.
  const indexes = new WeakMap();
  const indexOf = (data) => {
    if (indexes.has(data)) return indexes.get(data);
    const byName = new Map();
    const children = new Map();
    for (const item of data.api.items) {
      if (!byName.has(item.name)) byName.set(item.name, []);
      byName.get(item.name).push(item);
      const parent = item.path.slice(0, item.path.lastIndexOf("::"));
      if (!children.has(parent)) children.set(parent, []);
      children.get(parent).push(item);
    }
    const models = new Map((data.stage.models ?? []).map((m) => [m.name, m]));
    const modelTrait = data.api.items.find((i) => i.path === "renox::db::Model" && i.kind === "trait");
    const methods = new Set();
    for (const item of data.api.items) for (const m of item.members) if (m.kind === "method") methods.add(m.name);
    const index = { byName, children, models, modelTrait, methods: [...methods].sort(), data };
    indexes.set(data, index);
    return index;
  };

  // The detail is cut to fit the list; the info shows the whole signature.
  const memberOption = (m, boost) => ({
    label: m.name,
    type: KIND_TYPE[m.kind] ?? "property",
    detail: m.sig,
    info: m.doc ? `${m.sig}\n\n${m.doc}` : m.sig,
    boost,
  });
  const itemOption = (item) => ({
    label: item.name,
    type: KIND_TYPE[item.kind] ?? "class",
    detail: item.kind === "reexport" ? `from ${item.from}` : item.kind,
    info: item.doc || undefined,
  });
  const columnOptions = (model) =>
    model.columns.map((c) => ({
      label: c.const,
      type: "constant",
      detail: `Col<${model.name}, ${c.type}>`,
      info: c.doc || `The \`${c.column}\` column.`,
      boost: 2,
    }));
  // The members of every API item called `name` (a type, trait or enum).
  const membersOf = (ix, name) => {
    const out = [];
    for (const item of ix.byName.get(name) ?? []) if (item.kind !== "mod" && item.kind !== "fn") out.push(...item.members);
    // A stage model: the Model trait's members.
    if (ix.models.has(name) && ix.modelTrait) out.push(...ix.modelTrait.members);
    return out;
  };
  // What calling `member` on `type` gives (null when unknown).
  const resultOf = (ix, type, member) => {
    const m = membersOf(ix, type).find((x) => x.name === member);
    if (!m || !m.returns) return null;
    if (m.returns === "Self") return type;
    return m.returns;
  };
  const dedupe = (options) => {
    const seen = new Set();
    return options.filter((o) => (seen.has(o.label) ? false : seen.add(o.label)));
  };

  // The calls before the `.` at the end of `text`: `Name::f(…).g(…)?.await.`
  // → { root: "Name", steps: [["call", "f"], ["call", "g"]] }, or null.
  const chainOf = (text) => {
    const tokens = text.match(/[A-Za-z_]\w*|::|\.|\(|\)|\?|"(?:[^"\\]|\\.)*"|\S/g) ?? [];
    let chain = null;
    let depth = 0;
    for (let i = 0; i < tokens.length; i++) {
      const tok = tokens[i];
      if (tok === "(") { depth++; continue; }
      if (tok === ")") { depth = Math.max(0, depth - 1); continue; }
      if (depth > 0) continue;
      const next = tokens[i + 1];
      if (/^[A-Za-z_]/.test(tok) && next === "::" && /^[A-Za-z_]/.test(tokens[i + 2] ?? "")) {
        // A path: its last two segments are the root and the first step.
        let j = i;
        while (tokens[j + 1] === "::" && /^[A-Za-z_]/.test(tokens[j + 2] ?? "")) j += 2;
        chain = { root: tokens[j - 2], steps: [[tokens[j + 1] === "(" ? "call" : "const", tokens[j]]] };
        i = j;
        continue;
      }
      if (tok === "." && chain) {
        const name = tokens[i + 1];
        if (name === undefined) break;
        if (name === "await") { i++; continue; }
        if (/^[A-Za-z_]/.test(name) && tokens[i + 2] === "(") { chain.steps.push(["call", name]); i++; continue; }
        chain = null;
        continue;
      }
      if (tok === "?" && chain) continue;
      if (tok === "." ) continue;
      chain = null;
    }
    return chain;
  };

  const rustCompletions = (context, ix) => {
    const pos = context.pos;
    const before = context.state.sliceDoc(Math.max(0, pos - 2000), pos);
    const line = before.slice(before.lastIndexOf("\n") + 1);

    // #[derive(Mod…
    const derive = /#\[derive\(([^)]*)$/.exec(before);
    if (derive) {
      const word = /(\w*)$/.exec(derive[1])[1];
      const prelude = (ix.data.api.items ?? []).filter((i) => i.kind === "derive" && i.prelude).map(itemOption);
      return { from: pos - word.length, options: [...prelude, ...STD_DERIVES.map((d) => ({ label: d, type: "class", detail: "std / serde" }))], validFor: /^\w*$/ };
    }

    // Name::prefix or renox::a::prefix
    const path = /((?:[A-Za-z_]\w*::)+)(\w*)$/.exec(line);
    if (path) {
      const segments = path[1].slice(0, -2).split("::");
      const prefix = path[2];
      const from = pos - prefix.length;
      if (segments[0] === "renox") {
        const parent = segments.join("::");
        const options = (ix.children.get(parent) ?? []).map(itemOption);
        return options.length ? { from, options: dedupe(options), validFor: /^\w*$/ } : null;
      }
      const name = segments[segments.length - 1];
      const model = ix.models.get(name);
      if (model) {
        const trait = (ix.modelTrait?.members ?? []).filter((m) => m.kind === "fn" || m.kind === "const").map((m) => memberOption(m, 1));
        return { from, options: dedupe([...columnOptions(model), ...trait]), validFor: /^\w*$/ };
      }
      const members = membersOf(ix, name).filter((m) => m.kind === "fn" || m.kind === "const" || m.kind === "variant");
      return members.length ? { from, options: dedupe(members.map((m) => memberOption(m))), validFor: /^\w*$/ } : null;
    }

    // value.prefix: the chain rule.
    const dot = /\.(\w*)$/.exec(before);
    if (dot && !/\d\.\w*$/.test(before)) {
      const prefix = dot[1];
      const from = pos - prefix.length;
      const statement = before.slice(0, before.length - dot[0].length);
      // The statement, or the innermost argument list still open.
      let start = Math.max(statement.lastIndexOf(";"), statement.lastIndexOf("{"), statement.lastIndexOf("}"));
      const open = [];
      for (let i = start + 1; i < statement.length; i++) {
        if (statement[i] === "(") open.push(i);
        else if (statement[i] === ")") open.pop();
      }
      if (open.length) start = open[open.length - 1];
      const chain = chainOf(statement.slice(start + 1) + ".");
      let type = null;
      if (chain) {
        const [kind, first] = chain.steps[0];
        const model = ix.models.get(chain.root);
        if (kind === "const" && model?.columns.some((c) => c.const === first)) type = "Col";
        else type = resultOf(ix, chain.root, first);
        for (const [, name] of chain.steps.slice(1)) {
          if (!type) break;
          type = resultOf(ix, type, name);
        }
      }
      if (type) {
        const options = membersOf(ix, type).filter((m) => m.kind === "method").map((m) => memberOption(m, 99));
        if (options.length) return { from, options: dedupe(options), validFor: /^\w*$/ };
      }
      if (!prefix) return null;
      return { from, options: ix.methods.map((name) => ({ label: name, type: "method" })), validFor: /^\w*$/ };
    }

    // A bare name: prelude items and the stage's models.
    const word = /(?:^|[^\w:.])([A-Za-z_]\w+)$/.exec(before);
    if (word) {
      const prelude = ix.data.api.items.filter((i) => i.prelude && i.kind !== "mod").map(itemOption);
      const models = [...ix.models.values()].map((m) => ({ label: m.name, type: "class", detail: `model, table ${m.table}`, info: m.doc || undefined, boost: 3 }));
      return { from: pos - word[1].length, options: dedupe([...models, ...prelude]), validFor: /^\w*$/ };
    }
    return null;
  };

  const htmlCompletions = (context, ix) => {
    const pos = context.pos;
    const before = context.state.sliceDoc(Math.max(0, pos - 2000), pos);
    const open = before.lastIndexOf("<");
    if (open < 0 || before.indexOf(">", open) >= 0) return null;
    const inTag = before.slice(open);
    const tags = ix.data.stage.tags ?? [];
    // <rx-ta…
    const name = /^<([a-z][a-z0-9-]*)$/.exec(inTag);
    if (name) {
      return {
        from: pos - name[1].length,
        options: tags.map((t) => ({ label: t.name, type: "keyword", info: t.doc || undefined, boost: t.name.startsWith("app-") ? 2 : 1 })),
        validFor: /^[a-z0-9-]*$/,
      };
    }
    const tag = /^<((?:rx|app|live)-[a-z0-9-]+)\s/.exec(inTag);
    if (!tag) return null;
    const def = tags.find((t) => t.name === tag[1]);
    // Inside a value: name="…
    const value = /\s([:@]?[\w.-]+)="([^"]*)$/.exec(inTag);
    if (value) {
      const attribute = def?.attributes.find((a) => a.name === value[1]);
      if (!attribute || !attribute.values.length) return null;
      return { from: pos - value[2].length, options: attribute.values.map((v) => ({ label: v, type: "constant" })), validFor: /^[\w-]*$/ };
    }
    // At an attribute name (not inside quotes).
    if ((inTag.match(/"/g) ?? []).length % 2 === 1) return null;
    const attr = /\s([:@]?[\w-]*)$/.exec(inTag);
    if (!attr) return null;
    const own = (def?.attributes ?? []).map((a) => ({ label: a.name, type: "property", info: a.doc || undefined, boost: 2 }));
    const global = (ix.data.stage.global_attributes ?? []).map((a) => ({ label: a.name, type: "property", info: a.doc || undefined }));
    return { from: pos - attr[1].length, options: dedupe([...own, ...global]), validFor: /^[:@]?[\w-]*$/ };
  };

  const editors = new Map();

  const mount = (el, options = {}) => {
    const language = options.language === "html" ? "html" : "rust";
    const path = options.path ?? el.dataset.path ?? `untitled.${language === "html" ? "html" : "rs"}`;
    const textarea = el instanceof HTMLTextAreaElement ? el : null;
    const doc = options.doc ?? (textarea ? textarea.value : "");
    const onChange = options.onChange;
    const stage = options.stage ?? el.dataset.stage;
    // Filled by setCompletions; the source reads it on every request.
    let completions = null;
    const source = (context) => {
      if (!completions) return null;
      return language === "html" ? htmlCompletions(context, completions) : rustCompletions(context, completions);
    };

    const extensions = [
      CM.lineNumbers(),
      CM.highlightActiveLineGutter(),
      CM.history(),
      CM.drawSelection(),
      CM.indentOnInput(),
      CM.bracketMatching(),
      CM.closeBrackets(),
      CM.highlightActiveLine(),
      CM.highlightSelectionMatches(),
      CM.search({ top: true }),
      CM.syntaxHighlighting(highlight),
      CM.EditorView.darkTheme.of(true),
      CM.keymap.of([
        ...CM.closeBracketsKeymap,
        ...CM.defaultKeymap,
        ...CM.searchKeymap,
        ...CM.historyKeymap,
        ...CM.completionKeymap,
        CM.indentWithTab,
      ]),
      CM.EditorState.readOnly.of(Boolean(options.readOnly)),
      CM.EditorView.contentAttributes.of({ "aria-label": `${language === "html" ? "HTML" : "Rust"} editor: ${path}` }),
      CM.EditorView.updateListener.of((update) => {
        if (!update.docChanged) return;
        const value = update.state.doc.toString();
        if (textarea) textarea.value = value;
        onChange?.(value, editor);
      }),
      language === "html" ? [CM.html(), exprs, tagNames] : CM.rust(),
      // Ours next to the language's own (lang-html's tags), not instead of them.
      CM.EditorState.languageData.of(() => [{ autocomplete: source }]),
      CM.autocompletion({ icons: true }),
      // Suggestions live in <body>, so the editor's frame doesn't clip them.
      CM.tooltips({ parent: document.body }),
    ];

    const parent = document.createElement("div");
    parent.className = "learn-editor__area";
    if (textarea) {
      textarea.hidden = true;
      textarea.after(parent);
    } else {
      el.append(parent);
    }
    el.dataset.learnMounted = "";

    const view = new CM.EditorView({ state: CM.EditorState.create({ doc, extensions }), parent });

    const editor = {
      view,
      path,
      language,
      getValue: () => view.state.doc.toString(),
      setValue: (text) => view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: text } }),
      focus: () => view.focus(),
      reveal: (line, col = 1) => {
        const doc = view.state.doc;
        const l = doc.line(Math.min(Math.max(1, line), doc.lines));
        const pos = Math.min(l.from + Math.max(0, col - 1), l.to);
        view.dispatch({ selection: { anchor: pos }, effects: CM.EditorView.scrollIntoView(pos, { y: "center" }) });
        view.focus();
      },
      destroy: () => {
        view.destroy();
        parent.remove();
        if (textarea) textarea.hidden = false;
        delete el.dataset.learnMounted;
        if (editors.get(path) === editor) editors.delete(path);
      },
      setCompletions: (data) => {
        completions = data ? indexOf(data) : null;
      },
      // Filled in by #530 (diagnostics), if it comes back.
      setDiagnostics: () => {},
    };
    editors.set(path, editor);
    if (stage !== undefined && stage !== "") {
      editor.ready = loadCompletions(stage).then(
        (data) => editor.setCompletions(data),
        (error) => console.warn(`learn editor: no suggestions (${error.message})`),
      );
    }
    return editor;
  };

  const mountAll = (root = document) => {
    root.querySelectorAll("textarea[data-learn-editor]:not([data-learn-mounted])").forEach((el) => {
      mount(el, { language: el.dataset.learnEditor, path: el.dataset.path, readOnly: el.readOnly, stage: el.dataset.stage });
    });
  };

  window.LearnEditor = { mount, editors, mountAll, loadCompletions };

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", () => mountAll());
  } else {
    mountAll();
  }
  document.addEventListener("htmx:afterSettle", (event) => mountAll(event.target instanceof Element ? event.target : document));
}
