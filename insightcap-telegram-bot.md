# InsightCAP Telegram Bot 設計規劃

## 概覽

Telegram Bot 作為 InsightCAP 的手機端入口，取代原生 app 開發。用戶透過 Telegram 進行擷取、提問、查閱對話，電腦端 InsightCAP Server 負責所有重型處理。

---

## 安全設計

### 指定用戶限制

Bot 必須只回應授權用戶，拒絕所有未授權請求。

**實作方式：白名單 Chat ID**

```python
ALLOWED_CHAT_IDS = [123456789, 987654321]  # 在設定檔中管理

async def auth_check(update: Update) -> bool:
    return update.effective_chat.id in ALLOWED_CHAT_IDS
```

每個 Telegram 帳號有唯一的 `chat_id`，在設定檔中維護白名單。未授權用戶發送任何訊息，Bot 一律不回應（靜默拒絕，不洩露 Bot 存在）。

### 不被搜尋到

建立 Bot 後，透過 BotFather 設定：

```
/setjoingroups → Disable
/setprivacy → Enable
```

關鍵設定：**不要對 BotFather 設定 Bot 描述或公開名稱，並且不提交到任何 Bot 目錄。**

Telegram Bot 預設可被搜尋，必須主動關閉：透過 BotFather 發送 `/setprivacy`，選擇 `Disable`，讓 Bot 無法被公開搜尋到。Bot 連結（`t.me/your_bot`）只透過私下分享給授權用戶。

---

## 架構

```
手機 Telegram App
       │
       ▼
Telegram Server（訊息中繼）
       │
       ▼
InsightCAP Bot Server（電腦端常駐）
       │
       ├── 擷取處理 → KB 寫入
       ├── RAG 查詢 → LLM 回覆
       └── 對話管理 → 歷史紀錄
```

**電腦端需常駐：** InsightCAP Server + Bot Polling

---

## 互動設計

### 核心原則

**零指令摩擦**——直接打字就是提問，傳檔就是擷取，不需要任何前綴指令。

```
用戶輸入文字   →  RAG 提問，Bot 回覆
用戶傳送檔案   →  自動擷取存入 KB
用戶傳送 URL   →  爬取內容存入 KB
用戶傳送圖片   →  OCR 提取後存入 KB
```

### 指令（僅管理用途）

| 指令 | 功能 |
|------|------|
| `/new` | 開始新對話 |
| `/list` | 顯示對話列表，可點擊進入並繼續對話 |
| `/status` | 查看 Server 狀態、KB 大小 |
| `/recent` | 最近存入的 KB 條目 |

---

## 對話語境提示

### 設計原則：Option B + Option C

**打開 Bot 時（Option B）**：發送一條提示訊息，讓用戶立即知道當前語境：

```
👋 目前在「量子力學筆記」，直接輸入即可
輸入 /list 切換對話，/new 開始新對話
```

**Pinned Message（Option C）**：同步更新置頂訊息，作為持續提示，不干擾對話流：

```
📌 目前對話：量子力學筆記
```

切換對話或開始新對話時，自動更新 Pinned Message。用戶隨時往上看 pin 即可確認語境，不需要翻閱訊息流。

### 實作

```python
async def update_context(chat_id, conv_title, context):
    # 發送提示訊息
    await context.bot.send_message(chat_id, f"👋 目前在「{conv_title}」，直接輸入即可")

    # 更新 Pinned Message
    msg = await context.bot.send_message(chat_id, f"📌 目前對話：{conv_title}")
    await context.bot.pin_chat_message(chat_id, msg.message_id)
```

---

## 對話列表與切換

### `/list` 互動流程

用戶輸入 `/list`，Bot 回覆帶有 Inline Keyboard 的訊息：

```
📋 最近對話

[量子力學筆記]    2小時前
[專案規劃討論]    昨天
[Python 學習]     3天前
[讀書摘要]        上週
```

每個條目是可點擊的 Button。

### 點擊後行為

```
用戶點擊 [量子力學筆記]
       │
       ▼
Bot 載入該對話歷史，顯示最近幾條訊息摘要
Bot 提示：「已進入『量子力學筆記』，繼續輸入即可」
       │
       ▼
用戶直接打字  →  繼續此對話語境的 RAG 問答
```

### 當前對話提示

由於 Telegram 是單一對話串，切換對話後需要明確提示用戶目前的語境：

```
[量子力學筆記] 🔵
用戶：波函數是什麼？
Bot：根據你的筆記...
```

訊息前綴顯示當前對話名稱，避免用戶忘記自己在哪個語境。輸入 `/list` 可隨時切換到其他對話，或開始新對話。

---

## 擷取流程

```
用戶傳送文件 / 圖片 / URL
       │
       ▼
Bot 回覆「收到，處理中...」
       │
       ▼
InsightCAP 處理（embedding、清洗）
       │
       ▼
Bot 回覆「已存入 [Space 名稱]，共 N 個片段」
```

### 支援的擷取類型

| 類型 | Telegram 操作 | 處理方式 |
|------|--------------|---------|
| 文字 | 直接傳送訊息（擷取模式）| 存入 KB |
| 網址 | 傳送 URL | 爬取內容後存入 KB |
| 文件 | 上傳檔案（PDF、DOCX、TXT）| 解析後存入 KB |
| 截圖 / 圖片 | 上傳圖片 | OCR 提取文字後存入 KB |
| 影片字幕 | 傳送 YouTube / 影片連結 | 提取字幕後存入 KB |
| 語音備忘 | 錄音訊息 | STT 轉文字後存入 KB |

---

## 技術實作

### Bot 框架

**Python + python-telegram-bot v20+**

```python
from telegram.ext import Application, CommandHandler, MessageHandler, CallbackQueryHandler, filters

app = Application.builder().token(BOT_TOKEN).build()

# 指令
app.add_handler(CommandHandler("list", list_handler))
app.add_handler(CommandHandler("status", status_handler))
app.add_handler(CommandHandler("recent", recent_handler))

# 自動判斷：文字 / 檔案 / 圖片 / URL
app.add_handler(MessageHandler(filters.TEXT & ~filters.COMMAND, message_handler))
app.add_handler(MessageHandler(filters.Document.ALL, document_handler))
app.add_handler(MessageHandler(filters.PHOTO, photo_handler))

# Inline button 回調
app.add_handler(CallbackQueryHandler(button_handler))
```

### 連線模式

**Polling（推薦，無需公網 IP）**

```python
app.run_polling()
```

Bot Server 主動向 Telegram 拉取訊息，不需要對外開放 port，適合家用電腦。

### 授權中介

```python
from functools import wraps

def restricted(func):
    @wraps(func)
    async def wrapper(update, context):
        if update.effective_chat.id not in ALLOWED_CHAT_IDS:
            return  # 靜默拒絕
        return await func(update, context)
    return wrapper
```

### 對話列表 Inline Keyboard

```python
from telegram import InlineKeyboardButton, InlineKeyboardMarkup

@restricted
async def list_handler(update, context):
    conversations = get_recent_conversations()  # 從 InsightCAP 取得
    buttons = [
        [InlineKeyboardButton(f"{c.title}    {c.time_ago}", callback_data=f"conv_{c.id}")]
        for c in conversations
    ]
    await update.message.reply_text("📋 最近對話", reply_markup=InlineKeyboardMarkup(buttons))

async def button_handler(update, context):
    query = update.callback_query
    conv_id = query.data.replace("conv_", "")
    context.user_data["current_conv"] = conv_id  # 記住當前對話語境
    await query.answer()
    await query.edit_message_text(f"已進入對話，繼續輸入即可")
```

### Session 狀態管理

```python
@restricted
async def message_handler(update, context):
    user_text = update.message.text
    conv_id = context.user_data.get("current_conv")  # 當前對話語境

    # RAG 查詢，帶入對話語境
    response = insightcap.ask(query=user_text, conversation_id=conv_id)
    await update.message.reply_text(f"[{current_conv_title}] 🔵\n{response}")
```

---

## 設定檔結構

```toml
[telegram]
bot_token = "YOUR_BOT_TOKEN"
allowed_chat_ids = [123456789]

[insightcap]
kb_path = "/path/to/kb"
default_space = "inbox"

[processing]
auto_process_inbox = true
inbox_check_interval = 300  # 秒
```

---

## 限制與注意事項

- **電腦需常開**：Bot Polling 需要電腦持續運行，電腦關機則 Bot 無回應
- **Telegram 訊息長度限制**：單次回覆上限 4096 字，長回答自動分段傳送
- **檔案大小限制**：Telegram Bot API 接收檔案上限 20MB，傳送上限 50MB
- **隱私**：KB 資料仍在本機，Telegram 只傳遞訊息內容，不儲存 KB

---

## 開發優先序

**Phase 1** — 授權 + 文字提問（RAG）+ 自動擷取文字

**Phase 2** — 文件 / 圖片 / URL 擷取

**Phase 3** — `/new` + `/list` 對話列表 + Inline Keyboard 切換語境 + 語境提示（Pinned Message）

**Phase 4** — `/status` / `/recent` + 語音備忘 + 影片字幕
