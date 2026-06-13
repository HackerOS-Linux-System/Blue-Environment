import React, { useState, useRef } from 'react';
import { AppProps } from '../../types';
import { Globe, ArrowLeft, ArrowRight, RefreshCw, X, Lock, Unlock, Home, Search } from 'lucide-react';

const BlueWebApp: React.FC<AppProps> = () => {
    const [url, setUrl] = useState('https://start.duckduckgo.com/');
    const [inputUrl, setInputUrl] = useState(url);
    const [isLoading, setIsLoading] = useState(false);
    const [isSecure, setIsSecure] = useState(true);
    const iframeRef = useRef<HTMLIFrameElement>(null);

    const navigate = (target: string) => {
        let finalUrl = target.trim();
        if (!finalUrl.startsWith('http://') && !finalUrl.startsWith('https://')) {
            // Check if it looks like a domain
            if (finalUrl.includes('.') && !finalUrl.includes(' ')) {
                finalUrl = 'https://' + finalUrl;
            } else {
                // Treat as search query
                finalUrl = `https://duckduckgo.com/?q=${encodeURIComponent(finalUrl)}`;
            }
        }
        setUrl(finalUrl);
        setInputUrl(finalUrl);
        setIsSecure(finalUrl.startsWith('https://'));
        setIsLoading(true);
    };

    const handleKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
        if (e.key === 'Enter') {
            navigate(inputUrl);
        }
    };

    const handleBack = () => {
        if (iframeRef.current?.contentWindow) {
            iframeRef.current.contentWindow.history.back();
        }
    };

    const handleForward = () => {
        if (iframeRef.current?.contentWindow) {
            iframeRef.current.contentWindow.history.forward();
        }
    };

    const handleRefresh = () => {
        if (iframeRef.current) {
            iframeRef.current.src = url;
            setIsLoading(true);
        }
    };

    return (
        <div className="flex flex-col h-full bg-slate-900 text-white">
            {/* Browser toolbar */}
            <div className="h-12 bg-slate-800 border-b border-white/5 flex items-center px-2 gap-1.5 shrink-0">
                <button
                    onClick={handleBack}
                    className="p-2 hover:bg-white/10 rounded-lg transition-colors"
                    title="Back"
                >
                    <ArrowLeft size={16} />
                </button>
                <button
                    onClick={handleForward}
                    className="p-2 hover:bg-white/10 rounded-lg transition-colors"
                    title="Forward"
                >
                    <ArrowRight size={16} />
                </button>
                <button
                    onClick={handleRefresh}
                    className="p-2 hover:bg-white/10 rounded-lg transition-colors"
                    title="Refresh"
                >
                    <RefreshCw size={16} className={isLoading ? 'animate-spin' : ''} />
                </button>
                <button
                    onClick={() => navigate('https://start.duckduckgo.com/')}
                    className="p-2 hover:bg-white/10 rounded-lg transition-colors"
                    title="Home"
                >
                    <Home size={16} />
                </button>

                {/* URL bar */}
                <div className="flex-1 flex items-center gap-2 bg-slate-900 border border-white/10 rounded-lg px-3 py-1.5 mx-1">
                    {isSecure ? (
                        <Lock size={13} className="text-green-400 shrink-0" />
                    ) : (
                        <Unlock size={13} className="text-yellow-400 shrink-0" />
                    )}
                    <input
                        type="text"
                        value={inputUrl}
                        onChange={e => setInputUrl(e.target.value)}
                        onKeyDown={handleKeyDown}
                        onFocus={e => e.target.select()}
                        className="flex-1 bg-transparent text-sm text-white focus:outline-none"
                        placeholder="Search or enter URL..."
                    />
                    {inputUrl && (
                        <button
                            onClick={() => { setInputUrl(''); }}
                            className="text-slate-400 hover:text-white"
                        >
                            <X size={12} />
                        </button>
                    )}
                </div>
            </div>

            {/* Web content */}
            <div className="flex-1 relative">
                <iframe
                    ref={iframeRef}
                    src={url}
                    className="w-full h-full border-none bg-white"
                    sandbox="allow-scripts allow-same-origin allow-forms allow-popups allow-top-navigation"
                    onLoad={() => setIsLoading(false)}
                    title="Blue Web Browser"
                />
                {isLoading && (
                    <div className="absolute top-0 left-0 right-0 h-1 bg-blue-600/30">
                        <div className="h-full bg-blue-500 animate-pulse" style={{ width: '60%' }} />
                    </div>
                )}
            </div>
        </div>
    );
};

export default BlueWebApp;
