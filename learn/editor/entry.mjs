// The CodeMirror 6 pieces learn.renox.rs uses, bundled by bundle.sh into one
// IIFE that sets `window.CM`. Only libraries: learn's own logic lives in
// learn/public/editor.js, so it changes without rebuilding this bundle.
export { EditorState, Compartment, StateEffect, StateField } from '@codemirror/state';
export {
  EditorView,
  keymap,
  lineNumbers,
  highlightActiveLine,
  highlightActiveLineGutter,
  drawSelection,
  Decoration,
  MatchDecorator,
  ViewPlugin,
  showPanel,
  placeholder,
  tooltips,
} from '@codemirror/view';
export { defaultKeymap, history, historyKeymap, indentWithTab } from '@codemirror/commands';
export {
  syntaxHighlighting,
  defaultHighlightStyle,
  HighlightStyle,
  bracketMatching,
  indentOnInput,
  foldGutter,
} from '@codemirror/language';
export {
  autocompletion,
  completionKeymap,
  closeBrackets,
  closeBracketsKeymap,
  startCompletion,
} from '@codemirror/autocomplete';
export { linter, lintGutter, setDiagnostics, lintKeymap, forceLinting } from '@codemirror/lint';
export { search, searchKeymap, highlightSelectionMatches } from '@codemirror/search';
export { rust } from '@codemirror/lang-rust';
export { html } from '@codemirror/lang-html';
export { tags } from '@lezer/highlight';
