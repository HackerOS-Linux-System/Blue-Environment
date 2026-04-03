import React, { useState, useEffect, useRef } from 'react';
import { AppProps } from '../../types';
import { SystemBridge } from '../../utils/systemBridge';
import { useLanguage } from '../../contexts/LanguageContext';
import { Bot, Sparkles, LogOut, User, Loader2, ChevronDown, Key, Send, Settings, Trash2, Copy, Check } from 'lucide-react';

// Typy usług AI
type AIService = 'chatgpt' | 'grok' | 'claude' | 'gemini' | 'deepseek' | 'local';

const AI_SERVICES: { id: AIService; name: string; icon: string; models: string[]; requiresKey: boolean }[] = [
    { id: 'chatgpt', name: 'ChatGPT', icon: '🤖', models: ['gpt-4', 'gpt-3.5-turbo'], requiresKey: true },
{ id: 'grok', name: 'Grok', icon: '🕹️', models: ['grok-1'], requiresKey: true },
{ id: 'claude', name: 'Claude', icon: '🐙', models: ['claude-3-opus', 'claude-3-sonnet'], requiresKey: true },
{ id: 'gemini', name: 'Gemini', icon: '✨', models: ['gemini-pro', 'gemini-1.5-pro'], requiresKey: true },
{ id: 'deepseek', name: 'DeepSeek', icon: '🔍', models: ['deepseek-chat'], requiresKey: true },
{ id: 'local', name: 'Local Model', icon: '💻', models: ['llama2', 'mistral'], requiresKey: false },
];

interface Message {
    role: 'user' | 'assistant';
    content: string;
    timestamp: number;
}

const BlueAI: React.FC<AppProps> = () => {
    const { t } = useLanguage();
    const [selectedService, setSelectedService] = useState<AIService>('chatgpt');
    const [selectedModel, setSelectedModel] = useState<string>('gpt-4');
    const [apiKey, setApiKey] = useState<string>('');
    const [hasKey, setHasKey] = useState(false);
    const [messages, setMessages] = useState<Message[]>([]);
    const [input, setInput] = useState('');
    const [isLoading, setIsLoading] = useState(false);
    const [showSettings, setShowSettings] = useState(false);
    const [showKeyInput, setShowKeyInput] = useState(false);
    const [tempKey, setTempKey] = useState('');
    const [error, setError] = useState<string | null>(null);
    const [copied, setCopied] = useState(false);
    const messagesEndRef = useRef<HTMLDivElement>(null);

    // Załaduj konfigurację AI z backendu
    useEffect(() => {
        const loadConfig = async () => {
            const config = await SystemBridge.getAIConfig();
            if (config) {
                setSelectedService(config.service as AIService);
                setSelectedModel(config.model || getDefaultModel(config.service as AIService));
                setApiKey(config.apiKey || '');
                setHasKey(!!config.apiKey);
                if (config.messages) {
                    setMessages(config.messages);
                } else {
                    setMessages([{ role: 'assistant', content: t('ai.welcome'), timestamp: Date.now() }]);
                }
            } else {
                setMessages([{ role: 'assistant', content: t('ai.welcome'), timestamp: Date.now() }]);
            }
        };
        loadConfig();
    }, [t]);

    const getDefaultModel = (service: AIService): string => {
        const svc = AI_SERVICES.find(s => s.id === service);
        return svc?.models[0] || '';
    };

    // Zapisz konfigurację do backendu
    const saveConfig = async () => {
        await SystemBridge.saveAIConfig({
            service: selectedService,
            model: selectedModel,
            apiKey: apiKey,
            messages: messages,
        });
    };

    // Zmień usługę
    const handleServiceChange = (service: AIService) => {
        setSelectedService(service);
        setSelectedModel(getDefaultModel(service));
        setHasKey(!!apiKey);
        setShowSettings(false);
        saveConfig();
    };

    // Zapisz klucz API
    const handleSaveKey = () => {
        setApiKey(tempKey);
        setHasKey(true);
        setShowKeyInput(false);
        saveConfig();
    };

    // Usuń klucz
    const handleRemoveKey = async () => {
        setApiKey('');
        setHasKey(false);
        await SystemBridge.saveAIConfig({
            service: selectedService,
            model: selectedModel,
            apiKey: '',
            messages: messages,
        });
    };

    // Wyślij wiadomość do API
    const sendMessage = async () => {
        if (!input.trim() || isLoading) return;
        if (AI_SERVICES.find(s => s.id === selectedService)?.requiresKey && !apiKey) {
            setError('Please set an API key in settings first.');
            setShowSettings(true);
            return;
        }

        const userMessage = input;
        setInput('');
        const newMessages = [...messages, { role: 'user', content: userMessage, timestamp: Date.now() }];
        setMessages(newMessages);
        setIsLoading(true);
        setError(null);

        try {
            let response = '';
            switch (selectedService) {
                case 'chatgpt':
                    response = await callChatGPT(apiKey, selectedModel, newMessages);
                    break;
                case 'gemini':
                    response = await callGemini(apiKey, selectedModel, newMessages);
                    break;
                case 'deepseek':
                    response = await callDeepSeek(apiKey, selectedModel, newMessages);
                    break;
                case 'grok':
                    response = await callGrok(apiKey, selectedModel, newMessages);
                    break;
                case 'claude':
                    response = await callClaude(apiKey, selectedModel, newMessages);
                    break;
                case 'local':
                    response = await callLocalModel(selectedModel, newMessages);
                    break;
                default:
                    response = `[${selectedService}] Mock response: ${userMessage}`;
            }
            const assistantMessage = { role: 'assistant' as const, content: response, timestamp: Date.now() };
            setMessages([...newMessages, assistantMessage]);
        } catch (err: any) {
            setError(err.message || 'Failed to get response');
        } finally {
            setIsLoading(false);
            saveConfig();
        }
    };

    // Funkcje wywołań API (przez backend Tauri, aby ukryć klucze)
    const callChatGPT = async (key: string, model: string, history: Message[]): Promise<string> => {
        const response = await SystemBridge.aiCall({
            service: 'chatgpt',
            apiKey: key,
            model,
            messages: history.map(m => ({ role: m.role, content: m.content })),
        });
        return response;
    };

    const callGemini = async (key: string, model: string, history: Message[]): Promise<string> => {
        const response = await SystemBridge.aiCall({
            service: 'gemini',
            apiKey: key,
            model,
            messages: history,
        });
        return response;
    };

    const callDeepSeek = async (key: string, model: string, history: Message[]): Promise<string> => {
        const response = await SystemBridge.aiCall({
            service: 'deepseek',
            apiKey: key,
            model,
            messages: history,
        });
        return response;
    };

    const callGrok = async (key: string, model: string, history: Message[]): Promise<string> => {
        const response = await SystemBridge.aiCall({
            service: 'grok',
            apiKey: key,
            model,
            messages: history,
        });
        return response;
    };

    const callClaude = async (key: string, model: string, history: Message[]): Promise<string> => {
        const response = await SystemBridge.aiCall({
            service: 'claude',
            apiKey: key,
            model,
            messages: history,
        });
        return response;
    };

    const callLocalModel = async (model: string, history: Message[]): Promise<string> => {
        // Symulacja lokalnego modelu (np. przez Ollama)
        const response = await SystemBridge.aiCall({
            service: 'local',
            model,
            messages: history,
        });
        return response;
    };

    // Kopiuj ostatnią odpowiedź
    const copyLastResponse = () => {
        const last = messages.filter(m => m.role === 'assistant').pop();
        if (last) {
            navigator.clipboard.writeText(last.content);
            setCopied(true);
            setTimeout(() => setCopied(false), 2000);
        }
    };

    // Wyczyść historię
    const clearHistory = () => {
        setMessages([{ role: 'assistant', content: t('ai.welcome'), timestamp: Date.now() }]);
        saveConfig();
    };

    // Scroll do dołu
    useEffect(() => {
        messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
    }, [messages]);

    const serviceInfo = AI_SERVICES.find(s => s.id === selectedService);

    return (
        <div className="flex flex-col h-full bg-slate-900 text-white">
        {/* Header */}
        <div className="p-4 border-b border-white/5 flex items-center justify-between">
        <div className="flex items-center gap-3">
        <div className="p-2 bg-blue-600 rounded-lg">
        <Bot size={20} />
        </div>
        <div>
        <h3 className="font-semibold">Blue AI</h3>
        <div className="relative">
        <button
        onClick={() => setShowSettings(!showSettings)}
        className="text-xs text-blue-300 flex items-center gap-1 hover:underline"
        >
        {serviceInfo?.name} {serviceInfo?.icon}
        <ChevronDown size={12} />
        </button>
        </div>
        </div>
        </div>
        <div className="flex items-center gap-2">
        {hasKey && (
            <div className="flex items-center gap-1 text-xs text-green-400">
            <Key size={12} /> Key set
            </div>
        )}
        <button onClick={clearHistory} className="p-1.5 hover:bg-white/10 rounded-lg" title="Clear history">
        <Trash2 size={16} />
        </button>
        <button onClick={copyLastResponse} className="p-1.5 hover:bg-white/10 rounded-lg" title="Copy last response">
        {copied ? <Check size={16} /> : <Copy size={16} />}
        </button>
        </div>
        </div>

        {/* Settings panel */}
        {showSettings && (
            <div className="absolute top-14 right-4 w-80 bg-slate-800 border border-white/10 rounded-xl shadow-2xl z-50 p-4 backdrop-blur-sm">
            <div className="space-y-4">
            <div>
            <label className="block text-xs font-medium text-slate-400 mb-1">AI Service</label>
            <select
            value={selectedService}
            onChange={(e) => handleServiceChange(e.target.value as AIService)}
            className="w-full bg-slate-900 border border-white/10 rounded-lg px-3 py-2 text-sm"
            >
            {AI_SERVICES.map(s => (
                <option key={s.id} value={s.id}>{s.name} {s.icon}</option>
            ))}
            </select>
            </div>
            <div>
            <label className="block text-xs font-medium text-slate-400 mb-1">Model</label>
            <select
            value={selectedModel}
            onChange={(e) => setSelectedModel(e.target.value)}
            className="w-full bg-slate-900 border border-white/10 rounded-lg px-3 py-2 text-sm"
            disabled={!serviceInfo?.models.length}
            >
            {serviceInfo?.models.map(m => (
                <option key={m} value={m}>{m}</option>
            ))}
            </select>
            </div>
            {serviceInfo?.requiresKey && (
                <div>
                <label className="block text-xs font-medium text-slate-400 mb-1">API Key</label>
                {!hasKey ? (
                    <div>
                    <input
                    type="password"
                    placeholder="Enter API key"
                    value={tempKey}
                    onChange={(e) => setTempKey(e.target.value)}
                    className="w-full bg-slate-900 border border-white/10 rounded-lg px-3 py-2 text-sm"
                    />
                    <button
                    onClick={handleSaveKey}
                    className="mt-2 w-full bg-blue-600 hover:bg-blue-500 rounded-lg py-1 text-sm"
                    >
                    Save Key
                    </button>
                    </div>
                ) : (
                    <div className="flex items-center gap-2">
                    <span className="text-green-400 text-sm">✓ Key is set</span>
                    <button
                    onClick={handleRemoveKey}
                    className="text-red-400 text-xs hover:underline"
                    >
                    Remove
                    </button>
                    </div>
                )}
                </div>
            )}
            {!serviceInfo?.requiresKey && (
                <div className="text-xs text-slate-500">Local model runs on your machine (requires Ollama or similar).</div>
            )}
            </div>
            </div>
        )}

        {/* Messages area */}
        <div className="flex-1 overflow-y-auto p-4 space-y-4">
        {messages.map((msg, idx) => (
            <div
            key={idx}
            className={`flex ${msg.role === 'user' ? 'justify-end' : 'justify-start'}`}
            >
            <div
            className={`max-w-[85%] rounded-2xl px-4 py-2 text-sm ${
                msg.role === 'user'
                ? 'bg-blue-600 text-white rounded-br-none'
                : 'bg-slate-800 text-slate-100 rounded-bl-none'
            }`}
            >
            <div className="whitespace-pre-wrap break-words">{msg.content}</div>
            <div className="text-[10px] text-slate-400 mt-1 text-right">
            {new Date(msg.timestamp).toLocaleTimeString()}
            </div>
            </div>
            </div>
        ))}
        {isLoading && (
            <div className="flex justify-start">
            <div className="bg-slate-800 rounded-2xl rounded-bl-none px-4 py-2 flex items-center gap-2">
            <Loader2 size={14} className="animate-spin" />
            <span className="text-xs">{t('ai.thinking')}</span>
            </div>
            </div>
        )}
        {error && (
            <div className="bg-red-500/10 border border-red-500/30 rounded-lg p-3 text-red-400 text-sm">
            {error}
            </div>
        )}
        <div ref={messagesEndRef} />
        </div>

        {/* Input area */}
        <div className="p-4 border-t border-white/5">
        <div className="flex gap-2">
        <input
        type="text"
        value={input}
        onChange={(e) => setInput(e.target.value)}
        onKeyDown={(e) => e.key === 'Enter' && sendMessage()}
        placeholder={t('ai.placeholder')}
        disabled={isLoading}
        className="flex-1 bg-slate-800 border border-white/10 rounded-lg px-4 py-2 text-white focus:outline-none focus:border-blue-500 disabled:opacity-50"
        />
        <button
        onClick={sendMessage}
        disabled={isLoading || !input.trim()}
        className="px-4 py-2 bg-blue-600 hover:bg-blue-500 rounded-lg disabled:opacity-50 transition-colors"
        >
        <Send size={18} />
        </button>
        </div>
        </div>
        </div>
    );
};

export default BlueAI;
