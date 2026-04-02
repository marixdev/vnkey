with open('VNKEY/BangMa/Unicode built-in.txt', encoding='utf-8') as f:
    lines = [l.rstrip('\n\r') for l in f.readlines()]
print(f'Total lines: {len(lines)}')
for i in range(145, len(lines)):
    ch = lines[i]
    cps = ' '.join(f'U+{ord(c):04X}' for c in ch)
    print(f'  [{i:3d}] = "{ch}" ({cps})')
print('---')
for i in range(12):
    ch = lines[i]
    cps = ' '.join(f'U+{ord(c):04X}' for c in ch)
    print(f'  [{i:3d}] = "{ch}" ({cps})')
