import React, { useState, useRef, useEffect } from 'react';
import { Send, Bot, Sparkles, AlertTriangle, LogOut, User, Loader2 } from 'lucide-react';
import { AppProps } from '../../types';

// Deklaracja dla Google Identity Services
declare global {
    interface Window {
        google?: {
            accounts: {
                id: {
                    initialize: (config: any) => void;
                    prompt: (momentListener?: (notification: any) => void) => void;
                    renderButton: (parent: HTMLElement, options: any) => void;
                    disableAutoSelect: () => void;
                    revoke: (email: string, callback: () => void) => void;
                };
            };
        };
        handleCredentialResponse?: (response: any) => void;
    }
}

interface Message {
    role: 'user' | 'model' | 'error';
    text: string;
}

interface UserInfo {
    name: string;
    email: string;
    picture: string;
    sub: string;
}

// Poprawny Client ID – zastąp własnym z Google Cloud Console
const GOOGLE_CLIENT_ID = '1078882561818-9jvjbj1i3j1v3k5gk5k5k5k5k5k5k5k5.apps.googleusercontent.com';

const BlueAI: React.FC<AppProps> = () => {
    const [isLoggedIn, setIsLoggedIn] = useState(false);
    const [user, setUser] = useState<UserInfo | null>(null);
    const [messages, setMessages] = useState<Message[]>([
        { role: 'model', text: 'Cześć! Jestem Blue AI. Zaloguj się przez Google, aby korzystać z moich możliwości.' }
    ]);
    const [input, setInput] = useState('');
    const [isLoading, setIsLoading] = useState(false);
    const [isLoginLoading, setIsLoginLoading] = useState(false);
    const bottomRef = useRef<HTMLDivElement>(null);
    const loginButtonRef = useRef<HTMLDivElement>(null);

    useEffect(() => {
        bottomRef.current?.scrollIntoView({ behavior: 'smooth' });
    }, [messages, isLoading]);

    // Inicjalizacja Google Identity Services
    useEffect(() => {
        // Załaduj skrypt GIS
        const script = document.createElement('script');
        script.src = 'https://accounts.google.com/gsi/client';
        script.async = true;
        script.defer = true;
        script.onload = initializeGIS;
        document.body.appendChild(script);

        // Globalny callback dla logowania
        window.handleCredentialResponse = handleCredentialResponse;

        return () => {
            document.body.removeChild(script);
            delete window.handleCredentialResponse;
        };
    }, []);

    const initializeGIS = () => {
        if (!window.google) return;

        window.google.accounts.id.initialize({
            client_id: GOOGLE_CLIENT_ID,
            callback: handleCredentialResponse,
            auto_select: false,
            cancel_on_tap_outside: true,
        });

        // Renderuj przycisk w divie
        if (loginButtonRef.current) {
            window.google.accounts.id.renderButton(loginButtonRef.current, {
                type: 'standard',
                theme: 'filled_blue',
                size: 'large',
                text: 'signin_with',
                shape: 'pill',
                logo_alignment: 'left',
            });
        }
    };

    const handleCredentialResponse = (response: any) => {
        // Odebrano token JWT od Google
        const credential = response.credential;
        // Dekoduj payload
        const payload = JSON.parse(atob(credential.split('.')[1]));
        setUser({
            name: payload.name,
            email: payload.email,
            picture: payload.picture,
            sub: payload.sub,
        });
        setIsLoggedIn(true);
        setMessages([{ role: 'model', text: `Witaj, ${payload.name}! Jak mogę Ci pomóc?` }]);
        setIsLoginLoading(false);
    };

    const handleGoogleLogin = () => {
        setIsLoginLoading(true);
        if (window.google) {
            window.google.accounts.id.prompt(); // otwiera okno logowania
        } else {
            alert('Biblioteka Google nie została załadowana. Spróbuj odświeżyć stronę.');
            setIsLoginLoading(false);
        }
    };

    const handleLogout = () => {
        if (window.google && user) {
            window.google.accounts.id.disableAutoSelect();
            window.google.accounts.id.revoke(user.email, () => {
                console.log('Wylogowano');
            });
        }
        setIsLoggedIn(false);
        setUser(null);
        setMessages([{ role: 'model', text: 'Zostałeś wylogowany. Zaloguj się ponownie, aby kontynuować.' }]);
    };

    const handleSend = async () => {
        if (!input.trim() || isLoading || !isLoggedIn) return;

        const userMessage = input;
        setInput('');
        setMessages(prev => [...prev, { role: 'user', text: userMessage }]);
        setIsLoading(true);

        try {
            // Symulacja odpowiedzi AI (w rzeczywistości wywołanie API Gemini z tokenem)
            setTimeout(() => {
                setMessages(prev => [...prev, {
                    role: 'model',
                    text: `Otrzymałem wiadomość: "${userMessage}". To jest przykładowa odpowiedź – w rzeczywistości połączymy się z Gemini API.`
                }]);
                setIsLoading(false);
            }, 1000);
        } catch (error: any) {
            console.error("Błąd:", error);
            setMessages(prev => [...prev, { role: 'error', text: `Wystąpił błąd: ${error.message}` }]);
            setIsLoading(false);
        }
    };

    if (!isLoggedIn) {
        return (
            <div className="h-full bg-slate-900 flex flex-col items-center justify-center p-6 text-center space-y-8">
            <div className="relative">
            <div className="w-24 h-24 bg-gradient-to-br from-blue-600 to-indigo-600 rounded-3xl flex items-center justify-center shadow-2xl shadow-blue-600/20">
            <Bot size={48} className="text-white" />
            </div>
            <div className="absolute -bottom-2 -right-2 bg-white text-blue-600 p-2 rounded-full shadow-lg">
            <Sparkles size={20} />
            </div>
            </div>

            <div>
            <h2 className="text-3xl font-bold text-white mb-2">Witaj w Blue AI</h2>
            <p className="text-slate-400 max-w-xs mx-auto text-sm leading-relaxed">
            Zaloguj się przez Google, aby korzystać z asystenta AI.
            </p>
            </div>

            {/* Kontener na przycisk Google (renderowany przez GIS) */}
            <div ref={loginButtonRef} className="w-full max-w-xs" />

            {/* Przycisk awaryjny, gdyby renderowanie nie zadziałało */}
            <button
            onClick={handleGoogleLogin}
            disabled={isLoginLoading}
            className="w-full max-w-xs flex items-center justify-center gap-3 bg-white text-slate-900 px-6 py-3.5 rounded-xl font-semibold hover:bg-slate-100 transition-all active:scale-95 disabled:opacity-70 shadow-lg"
            >
            {isLoginLoading ? (
                <Loader2 size={20} className="animate-spin" />
            ) : (
                <>
                <svg className="w-5 h-5" viewBox="0 0 24 24">
                <path d="M22.56 12.25c0-.78-.07-1.53-.2-2.25H12v4.26h5.92c-.26 1.37-1.04 2.53-2.21 3.31v2.77h3.57c2.08-1.92 3.28-4.74 3.28-8.09z" fill="#4285F4"/>
                <path d="M12 23c2.97 0 5.46-.98 7.28-2.66l-3.57-2.77c-.98.66-2.23 1.06-3.71 1.06-2.86 0-5.29-1.93-6.16-4.53H2.18v2.84C3.99 20.53 7.7 23 12 23z" fill="#34A853"/>
                <path d="M5.84 14.09c-.22-.66-.35-1.36-.35-2.09s.13-1.43.35-2.09V7.07H2.18C1.43 8.55 1 10.22 1 12s.43 3.45 1.18 4.93l2.85-2.84z" fill="#FBBC05"/>
                <path d="M12 5.38c1.62 0 3.06.56 4.21 1.64l3.15-3.15C17.45 2.09 14.97 1 12 1 7.7 1 3.99 3.47 2.18 7.07l3.66 2.84c.87-2.6 3.3-4.53 6.16-4.53z" fill="#EA4335"/>
                </svg>
                Zaloguj się przez Google
                </>
            )}
            </button>
            </div>
        );
    }

    return (
        <div className="flex flex-col h-full bg-slate-900 text-slate-100">
        {/* Header */}
        <div className="p-4 border-b border-white/5 bg-slate-900/50 flex items-center justify-between">
        <div className="flex items-center gap-3">
        <div className="p-2 bg-blue-600 rounded-lg shadow-lg shadow-blue-500/20">
        <Bot size={20} className="text-white" />
        </div>
        <div>
        <h3 className="font-semibold text-sm">Blue AI</h3>
        <p className="text-xs text-blue-300 flex items-center gap-1">
        <Sparkles size={10} /> Zalogowano jako {user?.name}
        </p>
        </div>
        </div>
        <div className="flex items-center gap-2">
        {user?.picture ? (
            <img src={user.picture} alt="avatar" className="w-8 h-8 rounded-full" />
        ) : (
            <div className="w-8 h-8 bg-blue-600 rounded-full flex items-center justify-center">
            <User size={16} />
            </div>
        )}
        <button onClick={handleLogout} className="text-xs text-slate-500 hover:text-white px-2 flex items-center gap-1">
        <LogOut size={12} /> Wyloguj
        </button>
        </div>
        </div>

        {/* Messages */}
        <div className="flex-1 overflow-y-auto p-4 space-y-4">
        {messages.map((msg, idx) => (
            <div key={idx} className={`flex ${msg.role === 'user' ? 'justify-end' : 'justify-start'}`}>
            <div className={`max-w-[85%] rounded-2xl px-4 py-3 text-sm leading-relaxed shadow-sm ${
                msg.role === 'user'
                ? 'bg-blue-600 text-white rounded-br-none'
                : msg.role === 'error'
                ? 'bg-red-500/10 border border-red-500/50 text-red-200 rounded-bl-none'
                : 'bg-slate-800 text-slate-100 rounded-bl-none border border-white/5'
            }`}>
            {msg.role === 'error' && <AlertTriangle size={14} className="inline mr-2 -mt-0.5" />}
            <div className="whitespace-pre-wrap">{msg.text}</div>
            </div>
            </div>
        ))}
        {isLoading && (
            <div className="flex justify-start">
            <div className="bg-slate-800 rounded-2xl rounded-bl-none px-4 py-3 border border-white/5 flex items-center gap-1">
            <span className="w-1.5 h-1.5 bg-blue-400 rounded-full animate-bounce" style={{ animationDelay: '0ms' }}></span>
            <span className="w-1.5 h-1.5 bg-blue-400 rounded-full animate-bounce" style={{ animationDelay: '150ms' }}></span>
            <span className="w-1.5 h-1.5 bg-blue-400 rounded-full animate-bounce" style={{ animationDelay: '300ms' }}></span>
            </div>
            </div>
        )}
        <div ref={bottomRef} />
        </div>

        {/* Input */}
        <div className="p-4 bg-slate-900 border-t border-white/5">
        <div className="relative flex items-center gap-2 bg-slate-800 rounded-xl border border-white/10 focus-within:border-blue-500/50 p-1 pr-2">
        <input
        type="text"
        className="flex-1 bg-transparent px-3 py-2.5 text-sm text-white placeholder-slate-500 focus:outline-none"
        placeholder="Zapytaj Blue AI..."
        value={input}
        onChange={(e) => setInput(e.target.value)}
        onKeyDown={(e) => e.key === 'Enter' && handleSend()}
        disabled={isLoading}
        />
        <button
        onClick={handleSend}
        disabled={!input.trim() || isLoading}
        className="p-2 rounded-lg bg-blue-600 hover:bg-blue-500 disabled:bg-slate-700 disabled:text-slate-500 text-white transition-all"
        >
        <Send size={16} />
        </button>
        </div>
        </div>
        </div>
    );
};

export default BlueAI;
