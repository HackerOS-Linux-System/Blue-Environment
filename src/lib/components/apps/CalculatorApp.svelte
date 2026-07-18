<script lang="ts">
  let display = '0';
  let equation = '';
  let isNewNumber = true;

  function handleNumber(num: string) {
    if (isNewNumber) { display = num; isNewNumber = false; }
    else display = display === '0' ? num : display + num;
  }
  function handleOperator(op: string) { equation = display + ' ' + op + ' '; isNewNumber = true; }
  function handleEqual() {
    try {
      const fullEq = equation + display;
      // eslint-disable-next-line no-eval
      const result = eval(fullEq.replace('x', '*').replace('÷', '/'));
      display = String(result); equation = ''; isNewNumber = true;
    } catch { display = 'Error'; }
  }
  function handleClear() { display = '0'; equation = ''; isNewNumber = true; }

  const btnBase = 'h-14 rounded-full font-medium text-lg transition-all active:scale-95 flex items-center justify-center shadow-lg';
  const btnNum = `${btnBase} bg-slate-800 hover:bg-slate-700 text-white`;
  const btnOp = `${btnBase} bg-blue-600 hover:bg-blue-500 text-white`;
  const btnSec = `${btnBase} bg-slate-700 hover:bg-slate-600 text-slate-200`;
</script>

<div class="flex flex-col h-full bg-slate-950 p-4">
  <div class="flex-1 flex flex-col justify-end items-end mb-4 px-2 space-y-1">
    <div class="text-slate-500 text-sm h-6">{equation}</div>
    <div class="text-5xl font-light text-white tracking-tight break-all">{display}</div>
  </div>
  <div class="grid grid-cols-4 gap-3">
    <button on:click={handleClear} class="{btnSec} text-red-300">AC</button>
    <button on:click={() => handleNumber('(')} class={btnSec}>(</button>
    <button on:click={() => handleNumber(')')} class={btnSec}>)</button>
    <button on:click={() => handleOperator('/')} class={btnOp}>÷</button>
    {#each ['7', '8', '9'] as n (n)}<button on:click={() => handleNumber(n)} class={btnNum}>{n}</button>{/each}
    <button on:click={() => handleOperator('*')} class={btnOp}>×</button>
    {#each ['4', '5', '6'] as n (n)}<button on:click={() => handleNumber(n)} class={btnNum}>{n}</button>{/each}
    <button on:click={() => handleOperator('-')} class={btnOp}>−</button>
    {#each ['1', '2', '3'] as n (n)}<button on:click={() => handleNumber(n)} class={btnNum}>{n}</button>{/each}
    <button on:click={() => handleOperator('+')} class={btnOp}>+</button>
    <button on:click={() => handleNumber('0')} class="{btnNum} col-span-2 rounded-2xl">0</button>
    <button on:click={() => handleNumber('.')} class={btnNum}>.</button>
    <button on:click={handleEqual} class="{btnOp} bg-green-600 hover:bg-green-500">=</button>
  </div>
</div>
