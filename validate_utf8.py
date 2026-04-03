path = r'C:\Users\ckkong\InsightCAP\InsightCAP_3\src\components\chat\EditorPane.tsx'
with open(path, 'rb') as f: raw = f.read()

# Validate UTF-8 encoding
errors = []
i = 0
char_count = 0
line = 1
col = 0
last_error_line = -1

while i < len(raw):
    b = raw[i]
    col += 1
    
    if b == 0x0a:  # newline
        line += 1
        col = 0
        i += 1
        continue
    
    if b < 0x80:  # single byte char
        i += 1
        continue
    
    # Multi-byte sequence
    if b & 0xE0 == 0xC0:  # 2-byte sequence
        expected = 2
        code_bits = b & 0x1F
    elif b & 0xF0 == 0xE0:  # 3-byte sequence
        expected = 3
        code_bits = b & 0x0F
    elif b & 0xF8 == 0xF0:  # 4-byte sequence
        expected = 4
        code_bits = b & 0x07
    elif b & 0xC0 == 0x80:  # continuation byte -- unexpected start
        if line != last_error_line:
            errors.append(f'L{line} col {col}: Standalone continuation byte 0x{b:02x}')
            last_error_line = line
        i += 1
        continue
    else:
        if line != last_error_line:
            errors.append(f'L{line} col {col}: Invalid byte 0x{b:02x}')
            last_error_line = line
        i += 1
        continue
    
    # Check continuation bytes
    valid = True
    for k in range(1, expected):
        if i + k >= len(raw):
            errors.append(f'L{line} col {col}: Truncated UTF-8 sequence at byte {i}')
            valid = False
            break
        cont = raw[i + k]
        if cont & 0xC0 != 0x80:
            if line != last_error_line:
                errors.append(f'L{line} col {col}: Invalid continuation byte 0x{cont:02x} at offset {i+k}')
                last_error_line = line
            valid = False
            break
        code_bits = (code_bits << 6) | (cont & 0x3F)
    
    if valid:
        # Check for overlong encodings and surrogate half
        if expected == 2 and code_bits < 0x80:
            errors.append(f'L{line} col {col}: Overlong 2-byte sequence, codepoint U+{code_bits:04X}')
        elif expected == 3:
            if code_bits < 0x800:
                errors.append(f'L{line} col {col}: Overlong 3-byte sequence, codepoint U+{code_bits:04X}')
            elif 0xD800 <= code_bits <= 0xDFFF:
                errors.append(f'L{line} col {col}: Surrogate half U+{code_bits:04X}')
        i += expected
    else:
        i += 1

if errors:
    print(f'UTF-8 validation found {len(errors)} issues:')
    for e in errors[:30]:
        print(f'  {e}')
    if len(errors) > 30:
        print(f'  ... and {len(errors)-30} more')
else:
    print('UTF-8 validation: CLEAN')
print(f'Total lines: {line}')
