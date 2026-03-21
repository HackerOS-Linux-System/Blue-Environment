import React, { useState, useEffect, useRef } from 'react';
import { AppProps } from '../../types';
import { SystemBridge } from '../../utils/systemBridge';
import { useLanguage } from '../../contexts/LanguageContext';
import { Bot, Sparkles, LogOut, User, Loader2, ChevronDown, Key } from 'lucide-react';

type AIService = 'chatgpt' | 'grok' | 'claude' | 'gemini' | 'deepseek' | 'local';

const AI_SERVICES: { id: AIService; name: string; icon: string; requiresLogin: boolean }[] = [
    { id: 'chatgpt', name: 'ChatGPT', icon: '🤖', requiresLogin: true },
{ id: 'grok', name: 'Grok', icon: '🕹️', requiresLogin: true },
{ id: 'claude', name: 'Claude', icon: '🐙', requiresLogin: true },
{ id: 'gemini', name: 'Gemini', icon: '✨', requiresLogin: true },
{ id: 'deepseek', name: 'DeepSeek', icon: '🔍', requiresLogin: true },
{ id: 'local', name: 'Local Model', icon: '💻', requiresLogin: false },
];

interface Message {
    role: 'user' | 'assistant';
    content: string;
}

const BlueAI: React.FC<AppProps> = () => {
    const { t } = useLanguage();
    const [selectedService, setSelectedService] = useState<AIService>('chatgpt');
    const [isLoggedIn, setIsLoggedIn] = useState(false);
    const [user, setUser] = useState<{ name: string; email: string } | null>(null);
    const [messages, setMessages] = useState<Message[]>([]);
    const [input, setInput] = useState('');
    const [isLoading, setIsLoading] = useState(false);
    const [showServiceMenu, setShowServiceMenu] = useState(false);
    const [showLoginForm, setShowLoginForm] = useState(false);
    const [loginEmail, setLoginEmail] = useState('');
    const [loginPassword, setLoginPassword] = useState('');
    const [loginLoading, setLoginLoading] = useState(false);
    const messagesEndRef = useRef<HTMLDivElement>(null);

    useEffect(() => {
        loadSavedConfig();
    }, []);

    useEffect(() => {
        messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
    }, [messages]);

    const loadSavedConfig = async () => {
        const config = await SystemBridge.getAIConfig();
        if (config) {
            setSelectedService(config.service as AIService);
            if (config.user) {
                setUser(config.user);
                setIsLoggedIn(true);
            }
        }
    };

    const handleLogin = async () => {
        if (selectedService === 'local') {
            setUser({ name: 'Local User', email: 'local@blue' });
            setIsLoggedIn(true);
            await SystemBridge.saveAIConfig({ service: selectedService, user: { name: 'Local User', email: 'local@blue' } });
            setShowLoginForm(false);
            return;
        }

        setLoginLoading(true);
        const result = await SystemBridge.aiLogin(selectedService);
        if (result) {
            setUser(result.user);
            setIsLoggedIn(true);
            await SystemBridge.saveAIConfig({ service: selectedService, user: result.user, token: result.token });
            setShowLoginForm(false);
        }
        setLoginLoading(false);
    };

    const handleLogout = async () => {
        setIsLoggedIn(false);
        setUser(null);
        setMessages([]);
        await SystemBridge.saveAIConfig({ service: selectedService, user: null });
    };

    const handleSend = async () => {
        if (!input.trim() || isLoading) return;
        const userMessage = input;
        setMessages(prev => [...prev, { role: 'user', content: userMessage }]);
        setInput('');
        setIsLoading(true);

        try {
            // Mock AI response – w rzeczywistości wywołanie API
            setTimeout(() => {
                let response = `Response from ${selectedService.toUpperCase()}: ${userMessage}`;
                if (selectedService === 'local') {
                    response = `[Local model] Simulated response: ${userMessage}`;
                }
                setMessages(prev => [...prev, { role: 'assistant', content: response }]);
                setIsLoading(false);
            }, 1000);
        } catch (error: any) {
            setMessages(prev => [...prev, { role: 'assistant', content: `Error: ${error.message}` }]);
            setIsLoading(false);
        }
    };

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
        <h3 className="font-semibold">{t('blueai.title')}</h3>
        <div className="relative">
        <button
        onClick={() => setShowServiceMenu(!showServiceMenu)}
        className="text-xs text-blue-300 flex items-center gap-1 hover:underline"
        >
        {serviceInfo?.name} {serviceInfo?.icon}
        <ChevronDown size={12} />
        </button>
        {showServiceMenu && (
            <div className="absolute top-6 left-0 bg-slate-800 border border-white/10 rounded-lg shadow-lg z-10 w-40">
            {AI_SERVICES.map(service => (
                <button
                key={service.id}
                onClick={() => {
                    setSelectedService(service.id);
                    setShowServiceMenu(false);
                    setIsLoggedIn(false);
                    setUser(null);
                    setMessages([]);
                }}
                className="block w-full text-left px-4 py-2 hover:bg-white/10 text-sm"
                >
                {service.name} {service.icon}
                </button>
            ))}
            </div>
        )}
        </div>
        </div>
        </div>
        {isLoggedIn ? (
            <div className="flex items-center gap-3">
            <div className="flex items-center gap-2">
            <div className="w-8 h-8 bg-blue-600 rounded-full flex items-center justify-center">
            <User size={16} />
            </div>
            <span className="text-sm hidden sm:inline">{user?.name}</span>
            </div>
            <button
            onClick={handleLogout}
            className="p-1.5 hover:bg-white/10 rounded-lg"
            title={t('ai.logout')}
            >
            <LogOut size={16} />
            </button>
            </div>
        ) : (
            <button
            onClick={() => setShowLoginForm(true)}
            className="px-3 py-1.5 bg-blue-600 hover:bg-blue-500 rounded-lg text-sm flex items-center gap-2"
            >
            <Key size={14} /> {t('ai.login')}
            </button>
        )}
        </div>

        {/* Messages area */}
        <div className="flex-1 overflow-y-auto p-4 space-y-4">
        {messages.length === 0 && (
            <div className="flex flex-col items-center justify-center h-full text-center text-slate-500">
            <Bot size={48} className="mb-4 opacity-50" />
            <p>{t('ai.welcome')}</p>
            <p className="text-xs mt-2">{serviceInfo?.requiresLogin ? t('ai.login') : t('ai.local_model')}</p>
            </div>
        )}
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
            {msg.content}
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
        <div ref={messagesEndRef} />
        </div>

        {/* Input area */}
        <div className="p-4 border-t border-white/5">
        <div className="flex gap-2">
        <input
        type="text"
        value={input}
        onChange={(e) => setInput(e.target.value)}
        onKeyDown={(e) => e.key === 'Enter' && handleSend()}
        placeholder={t('ai.placeholder')}
        disabled={!isLoggedIn && serviceInfo?.requiresLogin}
        className="flex-1 bg-slate-800 border border-white/10 rounded-lg px-4 py-2 text-white focus:outline-none focus:border-blue-500 disabled:opacity-50"
        />
        <button
        onClick={handleSend}
        disabled={!input.trim() || isLoading || (!isLoggedIn && serviceInfo?.requiresLogin)}
        className="px-4 py-2 bg-blue-600 hover:bg-blue-500 rounded-lg disabled:opacity-50 transition-colors"
        >
        Send
        </button>
        </div>
        </div>

        {/* Login modal */}
        {showLoginForm && (
            <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50">
            <div className="bg-slate-800 rounded-2xl p-6 w-96 border border-white/10 shadow-2xl">
            <h3 className="text-xl font-bold text-white mb-4">
            {t('ai.login_to')} {serviceInfo?.name}
            </h3>
            {serviceInfo?.id === 'local' ? (
                <div className="space-y-4">
                <p className="text-slate-300 text-sm">Local model does not require authentication.</p>
                <button
                onClick={handleLogin}
                className="w-full bg-blue-600 hover:bg-blue-500 text-white py-2 rounded-lg"
                >
                Continue
                </button>
                </div>
            ) : (
                <div className="space-y-4">
                <input
                type="email"
                placeholder="Email"
                value={loginEmail}
                onChange={(e) => setLoginEmail(e.target.value)}
                className="w-full bg-slate-900 border border-white/10 rounded-lg px-4 py-2 text-white"
                />
                <input
                type="password"
                placeholder="Password"
                value={loginPassword}
                onChange={(e) => setLoginPassword(e.target.value)}
                className="w-full bg-slate-900 border border-white/10 rounded-lg px-4 py-2 text-white"
                />
                <button
                onClick={handleLogin}
                disabled={loginLoading}
                className="w-full bg-blue-600 hover:bg-blue-500 text-white py-2 rounded-lg disabled:opacity-50 flex items-center justify-center gap-2"
                >
                {loginLoading && <Loader2 size={16} className="animate-spin" />}
                {t('ai.login')}
                </button>
                </div>
            )}
            <button
            onClick={() => setShowLoginForm(false)}
            className="mt-4 w-full text-center text-sm text-slate-400 hover:text-white"
            >
            {t('settings.cancel')}
            </button>
            </div>
            </div>
        )}
        </div>
    );
};

export default BlueAI;
