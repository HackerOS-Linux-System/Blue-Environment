import React from 'react';
import { Check, Globe } from 'lucide-react';
import { useLanguage } from '../../../../contexts/LanguageContext';

const LANGUAGES: { code: 'en' | 'pl'; label: string; flag: string }[] = [
    { code: 'en', label: 'English', flag: '🇬🇧' },
    { code: 'pl', label: 'Polski', flag: '🇵🇱' },
];

const LanguageSection: React.FC = () => {
    const { language, setLanguage } = useLanguage();

    return (
        <div className="space-y-6 animate-in fade-in slide-in-from-bottom-2 duration-300">
            <h2 className="text-2xl font-bold text-white">Language</h2>
            <div className="bg-slate-800 p-6 rounded-2xl border border-white/5 space-y-2">
                <div className="flex items-center gap-3 text-slate-400 mb-2">
                    <Globe size={16} className="text-blue-400" />
                    <span className="text-sm">Interface language</span>
                </div>
                {LANGUAGES.map(l => (
                    <button
                        key={l.code}
                        onClick={() => setLanguage(l.code)}
                        className={`w-full flex items-center justify-between px-4 py-3 rounded-xl border transition-colors ${
                            language === l.code
                                ? 'bg-blue-600/15 border-blue-500/40 text-white'
                                : 'bg-slate-900/40 border-white/5 text-slate-300 hover:bg-white/5'
                        }`}
                    >
                        <span className="flex items-center gap-3">
                            <span className="text-lg">{l.flag}</span>
                            {l.label}
                        </span>
                        {language === l.code && <Check size={16} className="text-blue-400" />}
                    </button>
                ))}
            </div>
        </div>
    );
};

export default LanguageSection;
