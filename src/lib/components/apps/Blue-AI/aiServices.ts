export const AI_SERVICES = [
    { id: 'chatgpt',  name: 'ChatGPT',       kind: 'cloud', models: ['gpt-4o', 'gpt-4o-mini', 'gpt-3.5-turbo'],
      keyHint: 'platform.openai.com/api-keys' },
    { id: 'claude',   name: 'Claude',         kind: 'cloud', models: ['claude-sonnet-4-6', 'claude-3-5-sonnet-20241022', 'claude-3-haiku-20240307'],
      keyHint: 'console.anthropic.com/settings/keys' },
    { id: 'gemini',   name: 'Gemini',         kind: 'cloud', models: ['gemini-1.5-pro', 'gemini-1.5-flash', 'gemini-2.0-flash-exp'],
      keyHint: 'aistudio.google.com/apikey' },
    { id: 'deepseek', name: 'DeepSeek',       kind: 'cloud', models: ['deepseek-chat', 'deepseek-reasoner'],
      keyHint: 'platform.deepseek.com/api_keys' },
    { id: 'grok',     name: 'Grok (xAI)',     kind: 'cloud', models: ['grok-2-latest', 'grok-beta'],
      keyHint: 'console.x.ai' },
    // `models` here is only the fallback shown before Ollama has been
    // pinged — BlueAI.svelte replaces it with the real, currently-pulled
    // model list once checkOllamaStatus() succeeds, so the picker never
    // offers a model name the user doesn't actually have.
    { id: 'local',    name: 'Local (Ollama)', kind: 'local', models: ['llama3.2', 'mistral', 'codellama', 'phi3', 'qwen2.5'],
      keyHint: '' },
] as const;

export type AIServiceId = typeof AI_SERVICES[number]['id'];
