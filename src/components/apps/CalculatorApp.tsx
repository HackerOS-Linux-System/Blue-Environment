import React, { useState } from 'react';
import { AppProps } from '../../types';

const CalculatorApp: React.FC<AppProps> = () => {
    const [display, setDisplay] = useState('0');
    const [equation, setEquation] = useState('');
    const [isNewNumber, setIsNewNumber] = useState(true);

    const handleNumber = (num: string) => {
        if (isNewNumber) { setDisplay(num); setIsNewNumber(false); }
        else setDisplay(display === '0' ? num : display + num);
    };
    const handleOperator = (op: string) => { setEquation(display + ' ' + op + ' '); setIsNewNumber(true); };
    const handleEqual = () => {
        try {
            const fullEq = equation + display;
            // eslint-disable-next-line no-eval
            const result = eval(fullEq.replace('x', '*').replace('÷', '/'));
            setDisplay(String(result)); setEquation(''); setIsNewNumber(true);
        } catch { setDisplay('Error'); }
    };
    const handleClear = () => { setDisplay('0'); setEquation(''); setIsNewNumber(true); };

    const btnBase = 'h-14 rounded-full font-medium text-lg transition-all active:scale-95 flex items-center justify-center shadow-lg';
    const btnNum = `${btnBase} bg-slate-800 hover:bg-slate-700 text-white`;
    const btnOp = `${btnBase} bg-blue-600 hover:bg-blue-500 text-white`;
    const btnSec = `${btnBase} bg-slate-700 hover:bg-slate-600 text-slate-200`;

    return (
        <div className="flex flex-col h-full bg-slate-950 p-4">
            <div className="flex-1 flex flex-col justify-end items-end mb-4 px-2 space-y-1">
                <div className="text-slate-500 text-sm h-6">{equation}</div>
                <div className="text-5xl font-light text-white tracking-tight break-all">{display}</div>
            </div>
            <div className="grid grid-cols-4 gap-3">
                <button onClick={handleClear} className={`${btnSec} text-red-300`}>AC</button>
                <button onClick={() => handleNumber('(')} className={btnSec}>(</button>
                <button onClick={() => handleNumber(')')} className={btnSec}>)</button>
                <button onClick={() => handleOperator('/')} className={btnOp}>÷</button>
                {['7','8','9'].map(n => <button key={n} onClick={() => handleNumber(n)} className={btnNum}>{n}</button>)}
                <button onClick={() => handleOperator('*')} className={btnOp}>×</button>
                {['4','5','6'].map(n => <button key={n} onClick={() => handleNumber(n)} className={btnNum}>{n}</button>)}
                <button onClick={() => handleOperator('-')} className={btnOp}>−</button>
                {['1','2','3'].map(n => <button key={n} onClick={() => handleNumber(n)} className={btnNum}>{n}</button>)}
                <button onClick={() => handleOperator('+')} className={btnOp}>+</button>
                <button onClick={() => handleNumber('0')} className={`${btnNum} col-span-2 rounded-2xl`}>0</button>
                <button onClick={() => handleNumber('.')} className={btnNum}>.</button>
                <button onClick={handleEqual} className={`${btnOp} bg-green-600 hover:bg-green-500`}>=</button>
            </div>
        </div>
    );
};
export default CalculatorApp;
