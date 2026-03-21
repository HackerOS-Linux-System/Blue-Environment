import React from 'react';

export const EditorSettings: React.FC<{ onClose: () => void }> = ({ onClose }) => {
    return (
        <div className="absolute top-12 right-4 w-80 bg-slate-800 border border-white/10 rounded-xl p-4 z-50">
        <h3 className="font-bold mb-2">Editor Settings</h3>
        {/* miejsce na przełączniki motywu, rozmiaru czcionki, itp. */}
        <button onClick={onClose} className="mt-2 text-sm text-blue-400">Close</button>
        </div>
    );
};
