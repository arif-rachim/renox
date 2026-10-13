// learn.renox.rs's code editor (#528): CodeMirror 6 from the vendored bundle
// (vendor/codemirror-*.js sets window.CM), in Rust or HTML mode. The HTML mode
// marks Renox 1.1 templates: `{{ … }}`, `{# … #}` and `{% … %}` (cm-rx-expr)
// and `rx-*` / `app-*` / `live-*` tag names (cm-rx-tag).
//
// window.LearnEditor:
//   mount(el, { language: "rust" | "html", doc, path, readOnly, onChange }) → editor
//   editors: a Map of the mounted editors, keyed by path
// An editor: getValue(), setValue(text), focus(), reveal(line, col) (1-based),
// destroy(), setCompletions(data), setDiagnostics(list), view, path, language.
//
// Every textarea[data-learn-editor="rust|html"] (data-path gives the path) is
// mounted on load and after an htmx swap, once. The textarea is hidden and
// kept in step, so a form still posts its value.
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

  const editors = new Map();

  const mount = (el, options = {}) => {
    const language = options.language === "html" ? "html" : "rust";
    const path = options.path ?? el.dataset.path ?? `untitled.${language === "html" ? "html" : "rs"}`;
    const textarea = el instanceof HTMLTextAreaElement ? el : null;
    const doc = options.doc ?? (textarea ? textarea.value : "");
    const onChange = options.onChange;

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
      // Filled in by #529 (autocomplete) and #530 (diagnostics).
      setCompletions: () => {},
      setDiagnostics: () => {},
    };
    editors.set(path, editor);
    return editor;
  };

  const mountAll = (root = document) => {
    root.querySelectorAll("textarea[data-learn-editor]:not([data-learn-mounted])").forEach((el) => {
      mount(el, { language: el.dataset.learnEditor, path: el.dataset.path, readOnly: el.readOnly });
    });
  };

  window.LearnEditor = { mount, editors, mountAll };

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", () => mountAll());
  } else {
    mountAll();
  }
  document.addEventListener("htmx:afterSettle", (event) => mountAll(event.target instanceof Element ? event.target : document));
}
