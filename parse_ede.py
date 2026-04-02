import re

with open('vnkey-decompiled/frmVnKey_full.txt', 'r', encoding='utf-8') as f:
    text = f.read()

idx = text.find('unicodeDungSan = new string[170]')
start = text.find('{', idx)
depth = 0
end = start
for i, c in enumerate(text[start:]):
    if c == '{': depth += 1
    elif c == '}': 
        depth -= 1
        if depth == 0:
            end = start + i + 1
            break
block = text[start:end]
items = re.findall(r'"((?:[^"\\]|\\.)*?)"', block)
print(f'Total items: {len(items)}')
for i, item in enumerate(items):
    if i >= 140:
        codepoints = ' '.join(f'U+{ord(c):04X}' for c in item)
        print(f'  [{i:3d}] = "{item}" ({codepoints})')
