<script lang="ts">
  import { onMount, onDestroy, createEventDispatcher } from 'svelte';
  import { registerEditorThemes, registerCompletionProviders } from './completions';

  export let language: string;
  export let value: string;
  export let theme: string;
  export let fontSize: number;

  const dispatch = createEventDispatcher<{ change: string; mount: { editor: any; monaco: any } }>();

  let container: HTMLDivElement;
  let editor: any;
  let monaco: any;
  let applyingExternalValue = false;

  const EDITOR_OPTIONS = {
    minimap: { enabled: true }, scrollBeyondLastLine: false, automaticLayout: true,
    fontFamily: 'JetBrains Mono, Fira Code, monospace', renderWhitespace: 'boundary' as const,
    tabSize: 4, wordWrap: 'off' as const, suggestOnTriggerCharacters: true,
    quickSuggestions: { other: true, comments: true, strings: true },
    parameterHints: { enabled: true }, formatOnPaste: true, formatOnType: false,
    bracketPairColorization: { enabled: true }, guides: { bracketPairs: true, indentation: true },
    renderLineHighlight: 'all' as const, scrollbar: { verticalScrollbarSize: 6, horizontalScrollbarSize: 6 },
  };

  onMount(async () => {
    monaco = await import('monaco-editor');
    registerEditorThemes(monaco);
    registerCompletionProviders(monaco);

    editor = monaco.editor.create(container, {
      value, language, theme, fontSize,
      ...EDITOR_OPTIONS,
    });

    editor.onDidChangeModelContent(() => {
      if (applyingExternalValue) return;
      dispatch('change', editor.getValue());
    });

    dispatch('mount', { editor, monaco });
  });

  onDestroy(() => editor?.dispose());

  // Swap the model's content (and language) when switching tabs — avoids
  // tearing down/recreating the whole editor instance on every tab click.
  $: if (editor && monaco) {
    const model = editor.getModel();
    if (model && model.getValue() !== value) {
      applyingExternalValue = true;
      editor.setValue(value);
      applyingExternalValue = false;
    }
    if (model && monaco.editor.getModelLanguage?.(model) !== language) {
      monaco.editor.setModelLanguage(model, language);
    }
  }
  $: if (editor) editor.updateOptions({ fontSize });
  $: if (monaco) monaco.editor.setTheme(theme);
</script>

<div bind:this={container} class="w-full h-full" />
