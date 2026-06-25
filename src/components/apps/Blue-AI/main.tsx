import React, { useState, useEffect, useRef } from 'react';
import { Send, Bot, User, Settings, X, Loader2, Trash2, Copy } from 'lucide-react';
import { SystemBridge } from '../../../utils/systemBridge';
import { AppProps, AIMessage, AIConfig } from '../../../types';
import { AI_SERVICES } from './src/types';

const BlueAI: React.FC<AppProps> = () => {
    const [messages, setMessages] = useState<AIMessage[]>([]);
    const [input, setInput] = useState('');
    const [isLoading, setIsLoading] = useState(false);
    const [config, setConfig] = useState<AIConfig>({ service: 'chatgpt', model: 'gpt-4o', apiKey: '' });
    const [showSettings, setShowSettings] = useState(false);
    const [error, setError] = useState<string | null>(null);
    const scrollRef = useRef<HTMLDivElement>(null);
    const textareaRef = useRef<HTMLTextAreaElement>(null);

    useEffect(() => {
        // Explicit type annotation on `cfg` fixes TS7006 (implicit any)
        SystemBridge.loadConfig().then((cfg: ReturnType<typeof SystemBridge.loadConfig> extends Promise<infer T> ? T : never) => {
            const ext = cfg as { aiConfig?: AIConfig };
            if (ext.aiConfig) setConfig(ext.aiConfig);
        });
    }, []);

    useEffect(() => {
        scrollRef.current?.scrollTo({ top: scrollRef.current.scrollHeight, behavior: 'smooth' });
    }, [messages, isLoading]);

    const saveConfig = async (newCfg: AIConfig) => {
        setConfig(newCfg);
        const appCfg = await SystemBridge.loadConfig();
        await SystemBridge.saveConfig({ ...appCfg, aiConfig: newCfg });
    };

    const sendMessage = async () => {
        const text = input.trim();
        if (!text || isLoading) return;
        setError(null);
        setInput('');
        if (textareaRef.current) textareaRef.current.style.height = '42px';
        const userMsg: AIMessage = { role: 'user', content: text };
        const newMessages = [...messages, userMsg];
        setMessages(newMessages);
        setIsLoading(true);
        try {
            const reply = await SystemBridge.aiCall({
                service: config.service,
                apiKey: config.apiKey,
                model: config.model,
                messages: newMessages,
            });
            setMessages(prev => [...prev, { role: 'assistant', content: reply }]);
        } catch (e: unknown) {
            setError(e instanceof Error ? e.message : 'AI request failed');
        } finally {
            setIsLoading(false);
        }
    };

    const handleKeyDown = (e: React.KeyboardEvent) => {
        if (e.key === 'Enter' && !e.shiftKey) { e.preventDefault(); sendMessage(); }
    };

    const currentService = AI_SERVICES.find(s => s.id === config.service);

    const renderContent = (content: string) =>
        content.split(/(```[\s\S]*?```)/g).map((part, i) => {
            if (part.startsWith('```')) {
                const lines = part.slice(3).split('\n');
                const lang = lines[0];
                const code = lines.slice(1, -1).join('\n');
                return (
                    <pre key={i} className="bg-slate-950 rounded-lg p-3 my-2 overflow-x-auto text-xs font-mono text-green-300 border border-white/5">
                        {lang && <div className="text-slate-500 text-[10px] mb-1">{lang}</div>}
                        {code}
                    </pre>
                );
            }
            return <span key={i} className="whitespace-pre-wrap">{part}</span>;
        });

    return (
        <div className="flex flex-col h-full bg-slate-900 text-white overflow-hidden">
            <div className="shrink-0 h-12 flex items-center justify-between px-4 bg-slate-800 border-b border-white/5">
                <div className="flex items-center gap-2">
                    <div className="w-7 h-7 rounded-lg bg-gradient-to-br from-blue-500 to-purple-600 flex items-center justify-center shadow">
                        <Bot size={15} />
                    </div>
                    <span className="font-semibold text-sm">Blue AI</span>
                    <span className="text-xs text-slate-500">— {currentService?.name} / {config.model}</span>
                </div>
                <div className="flex gap-1">
                    {messages.length > 0 && (
                        <button onClick={() => setMessages([])} className="p-1.5 hover:bg-white/10 rounded-lg text-slate-400" title="Clear">
                            <Trash2 size={14} />
                        </button>
                    )}
                    <button
                        onClick={() => setShowSettings(s => !s)}
                        className={`p-1.5 rounded-lg ${showSettings ? 'bg-blue-600/20 text-blue-400' : 'hover:bg-white/10 text-slate-400'}`}
                    >
                        <Settings size={16} />
                    </button>
                </div>
            </div>

            {showSettings && (
                <div className="shrink-0 border-b border-white/5 bg-slate-800/80 p-4 space-y-3">
                    <div className="grid grid-cols-2 gap-3">
                        <div>
                            <label className="text-xs text-slate-400 block mb-1">Service</label>
                            <select
                                value={config.service}
                                onChange={e => {
                                    const svc = AI_SERVICES.find(s => s.id === e.target.value);
                                    saveConfig({ ...config, service: e.target.value, model: svc?.models[0] || '' });
                                }}
                                className="w-full bg-slate-900 border border-white/10 rounded-lg px-3 py-1.5 text-sm text-white focus:outline-none"
                            >
                                {AI_SERVICES.map(s => <option key={s.id} value={s.id}>{s.name}</option>)}
                            </select>
                        </div>
                        <div>
                            <label className="text-xs text-slate-400 block mb-1">Model</label>
                            <select
                                value={config.model}
                                onChange={e => saveConfig({ ...config, model: e.target.value })}
                                className="w-full bg-slate-900 border border-white/10 rounded-lg px-3 py-1.5 text-sm text-white focus:outline-none"
                            >
                                {currentService?.models.map(m => <option key={m} value={m}>{m}</option>)}
                            </select>
                        </div>
                    </div>
                    {config.service !== 'local' && (
                        <div>
                            <label className="text-xs text-slate-400 block mb-1">API Key</label>
                            <div className="flex gap-2">
                                <input
                                    type="password"
                                    id="ai-key"
                                    defaultValue={config.apiKey}
                                    placeholder="sk-..."
                                    className="flex-1 bg-slate-900 border border-white/10 rounded-lg px-3 py-1.5 text-sm text-white focus:outline-none"
                                />
                                <button
                                    onClick={() => {
                                        const el = document.getElementById('ai-key') as HTMLInputElement;
                                        if (el) saveConfig({ ...config, apiKey: el.value });
                                    }}
                                    className="px-3 py-1.5 bg-blue-600 hover:bg-blue-500 rounded-lg text-sm"
                                >
                                    Save
                                </button>
                            </div>
                        </div>
                    )}
                    <button onClick={() => setShowSettings(false)} className="w-full text-center text-xs text-slate-500 hover:text-white pt-1">
                        Close
                    </button>
                </div>
            )}

            <div ref={scrollRef} className="flex-1 overflow-y-auto p-4 space-y-4">
                {messages.length === 0 && !showSettings && (
                    <div className="flex flex-col items-center justify-center h-full text-center">
                        <div className="w-16 h-16 rounded-2xl bg-gradient-to-br from-blue-500/20 to-purple-600/20 flex items-center justify-center mb-4 border border-white/5">
                            <Bot size={32} className="text-blue-400" />
                        </div>
                        <h3 className="text-lg font-semibold mb-1">Blue AI</h3>
                        <p className="text-slate-400 text-sm max-w-xs">
                            {config.apiKey || config.service === 'local' ? 'Ask me anything.' : 'Open Settings to configure your API key.'}
                        </p>
                        {!config.apiKey && config.service !== 'local' && (
                            <button onClick={() => setShowSettings(true)} className="mt-4 px-4 py-2 bg-blue-600 hover:bg-blue-500 rounded-xl text-sm">
                                Open Settings
                            </button>
                        )}
                    </div>
                )}
                {messages.map((msg, idx) => (
                    <div key={idx} className={`flex gap-3 ${msg.role === 'user' ? 'justify-end' : 'justify-start'}`}>
                        {msg.role === 'assistant' && (
                            <div className="w-7 h-7 rounded-full bg-gradient-to-br from-blue-500 to-purple-600 flex items-center justify-center shrink-0 mt-0.5">
                                <Bot size={14} />
                            </div>
                        )}
                        <div className="group max-w-[80%] relative">
                            <div className={`rounded-2xl px-4 py-3 text-sm leading-relaxed ${msg.role === 'user' ? 'bg-blue-600 text-white rounded-br-md' : 'bg-slate-800 text-slate-100 rounded-bl-md border border-white/5'}`}>
                                {renderContent(msg.content)}
                            </div>
                            <button
                                onClick={() => navigator.clipboard.writeText(msg.content)}
                                className="absolute top-1 right-1 opacity-0 group-hover:opacity-100 p-1 bg-black/40 rounded-md"
                            >
                                <Copy size={10} />
                            </button>
                        </div>
                        {msg.role === 'user' && (
                            <div className="w-7 h-7 rounded-full bg-slate-700 flex items-center justify-center shrink-0 mt-0.5">
                                <User size={14} />
                            </div>
                        )}
                    </div>
                ))}
                {isLoading && (
                    <div className="flex gap-3">
                        <div className="w-7 h-7 rounded-full bg-gradient-to-br from-blue-500 to-purple-600 flex items-center justify-center shrink-0">
                            <Bot size={14} />
                        </div>
                        <div className="bg-slate-800 rounded-2xl rounded-bl-md px-4 py-3 border border-white/5">
                            <div className="flex gap-1 items-center h-5">
                                {[0, 1, 2].map(i => (
                                    <div key={i} className="w-1.5 h-1.5 bg-blue-400 rounded-full animate-bounce" style={{ animationDelay: `${i * 150}ms` }} />
                                ))}
                            </div>
                        </div>
                    </div>
                )}
                {error && (
                    <div className="bg-red-500/10 border border-red-500/20 rounded-xl p-3 text-red-300 text-sm flex gap-2">
                        <X size={14} className="shrink-0 mt-0.5" />
                        {error}
                    </div>
                )}
            </div>

            <div className="shrink-0 p-3 border-t border-white/5 bg-slate-800/50">
                <div className="flex gap-2 items-end">
                    <textarea
                        ref={textareaRef}
                        value={input}
                        onChange={e => setInput(e.target.value)}
                        onKeyDown={handleKeyDown}
                        placeholder="Ask me anything... (Enter to send)"
                        rows={1}
                        className="flex-1 bg-slate-800 border border-white/10 rounded-xl px-4 py-2.5 text-sm text-white resize-none focus:outline-none focus:border-blue-500/50 max-h-32 placeholder-slate-500"
                        style={{ minHeight: '42px' }}
                        onInput={e => {
                            const t = e.target as HTMLTextAreaElement;
                            t.style.height = 'auto';
                            t.style.height = Math.min(t.scrollHeight, 128) + 'px';
                        }}
                    />
                    <button
                        onClick={sendMessage}
                        disabled={!input.trim() || isLoading}
                        className="w-10 h-10 bg-blue-600 hover:bg-blue-500 rounded-xl flex items-center justify-center shrink-0 disabled:opacity-40"
                    >
                        {isLoading ? <Loader2 size={16} className="animate-spin" /> : <Send size={16} />}
                    </button>
                </div>
                <div className="flex justify-between mt-1.5 px-1">
                    <span className="text-[10px] text-slate-600">Shift+Enter = newline</span>
                    <span className="text-[10px] text-slate-600">{messages.length} messages</span>
                </div>
            </div>
        </div>
    );
};

export default BlueAI;
