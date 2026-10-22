import re, pathlib, sys
pat = re.compile(r'^(\s*)let (\w+): (.+?) = \$state\((.*)\);\s*$')
n = 0
for p in pathlib.Path('src').rglob('*.svelte'):
    lines = p.read_text().split('\n'); out = []; ch = False
    for ln in lines:
        m = pat.match(ln)
        if m:
            ind, name, typ, arg = m.groups()
            if arg.strip() == '':
                if 'undefined' in typ: new = f'{ind}let {name} = $state<{typ}>();'
                else: new = f'{ind}let {name} = $state() as {typ};'
            else:
                new = f'{ind}let {name} = $state<{typ}>({arg});'
            out.append(new); ch = True; n += 1
        else: out.append(ln)
    if ch: p.write_text('\n'.join(out))
print('rewrote', n)
