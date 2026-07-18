export const AI_SERVICES = [
    { id: 'chatgpt',  name: 'ChatGPT',       models: ['gpt-4o', 'gpt-4o-mini', 'gpt-3.5-turbo'] },
    { id: 'claude',   name: 'Claude',         models: ['claude-sonnet-4-6', 'claude-3-5-sonnet-20241022', 'claude-3-haiku-20240307'] },
    { id: 'gemini',   name: 'Gemini',         models: ['gemini-1.5-pro', 'gemini-1.5-flash', 'gemini-2.0-flash-exp'] },
    { id: 'deepseek', name: 'DeepSeek',       models: ['deepseek-chat', 'deepseek-reasoner'] },
    { id: 'grok',     name: 'Grok (xAI)',     models: ['grok-2-latest', 'grok-beta'] },
    { id: 'local',    name: 'Local (Ollama)', models: ['llama3.2', 'mistral', 'codellama', 'phi3', 'qwen2.5'] },
] as const;

export type AIServiceId = typeof AI_SERVICES[number]['id'];
