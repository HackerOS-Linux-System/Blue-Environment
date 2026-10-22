import json, sys, pathlib
LANGS = {'en':'english','pl':'polish','de':'german','fr':'french','es':'spanish','ru':'russian','uk':'ukrainian','cs':'czech',
         'it':'italian','pt':'portuguese','nl':'dutch','sv':'swedish','tr':'turkish','ja':'japanese','zh':'chinese','ar':'arabic'}
ANCHOR = "    'topbar.clipboard_history':"
def q(s): return "'" + s.replace("\\", "\\\\").replace("'", "\\'") + "'"
data = json.load(open(sys.argv[1], encoding='utf-8'))
for code, name in LANGS.items():
    p = pathlib.Path(f'src/lib/translations/{name}.ts'); t = p.read_text(encoding='utf-8')
    add = ''
    for key, tr in data.items():
        if f"'{key}':" in t: continue
        val = tr.get(code) or tr['en']
        add += f"    '{key}': {q(val)},\n"
    if add:
        assert ANCHOR in t, (name, 'anchor missing')
        p.write_text(t.replace(ANCHOR, add + ANCHOR, 1), encoding='utf-8')
print('ok')
