path = r'C:\Users\ckkong\InsightCAP\InsightCAP_3\src\components\chat\EditorPane.tsx'
with open(path, 'rb') as f: raw = f.read()
lines_arr = raw.split(b'\n')
fixes = 0

# ── Fix 1: Line 1407 - Bubble menu Highlighter title (unterminated) ────────────
# Find SECOND occurrence of toggleHighlight (first is toolbar, second is bubble menu)
first_hl = raw.find(b'toggleHighlight')
second_hl = raw.find(b'toggleHighlight', first_hl + 1)
print(f'First toggleHighlight at byte: {first_hl}')
print(f'Second toggleHighlight at byte: {second_hl}')

if second_hl >= 0:
    # Look FORWARD for title=" within 400 bytes (title comes AFTER onClick in this structure)
    title_idx = raw.find(b'title="', second_hl)
    if title_idx >= 0 and title_idx - second_hl < 500:
        title_end = raw.find(b'\r\n', title_idx + 7)
        old_tit = raw[title_idx:title_end+2]
        new_tit = '                                    title="\u87a2\u5149\u7b46"\r\n'.encode('utf-8')
        if old_tit != new_tit and (b'\xef\xbf\xbd' in old_tit or not old_tit.rstrip(b'\r\n').endswith(b'"')):
            raw = raw[:title_idx] + new_tit + raw[title_end+2:]
            fixes += 1
            print('[OK] Fixed bubble menu Highlighter title (螢光筆)')
        else:
            print(f'[==] Already OK: {old_tit!r}')
    else:
        print('[!!] Could not find title= near second toggleHighlight')

# ── Fix 2: Line 1292 - Japanese button missing > and className ─────────────────
# Current: <button onClick={() => handleAiImprove('...')}</button>
# Needed:  <button onClick={() => handleAiImprove('...')} className="..." >{t(...)}</button>
ja_text = '以下のテキストを日本語に翻訳して出力してください。翻訳文のみを出力してください。'
old_btn = ("                                                    <button onClick={() => handleAiImprove('" + 
           ja_text + "')}</button>\r\n").encode('utf-8')
new_btn_str = ('                                                    <button onClick={() => handleAiImprove(\'' + 
               ja_text + '\')}'
               ' className="w-full px-2.5 py-1.5 text-fs-xs font-medium text-text-secondary hover:bg-surface-subtle hover:text-text-primary rounded-md transition-colors text-left">'
               "{t('editor.ai_translate_ja')}</button>\r\n")
new_btn = new_btn_str.encode('utf-8')

if old_btn in raw:
    raw = raw.replace(old_btn, new_btn, 1)
    fixes += 1
    print('[OK] Fixed Japanese button (added className + > + content)')
else:
    # Show what's there
    idx = raw.find(('handleAiImprove(\'' + ja_text + '\')').encode('utf-8'))
    if idx >= 0:
        line_start = raw.rfind(b'\n', 0, idx) + 1
        line_end = raw.find(b'\n', idx)
        print(f'[!!] Found Japanese button but pattern mismatch: {raw[line_start:line_end][:100]!r}')
    else:
        print('[!!] Japanese button not found - maybe already fixed?')

print(f'\nTotal fixes: {fixes}')
with open(path, 'wb') as f:
    f.write(raw)
print('Done.')
