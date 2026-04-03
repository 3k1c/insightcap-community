# InsightCAP Design System
## 完整規範文件 v1.1

> 本文件涵蓋 InsightCAP 四套完整主題的設計規範與 CSS Token。
> 所有主題共享相同的 Typography、Component、動效與知識類型視覺語言，
> 僅透過 CSS class 覆寫各自的色彩 token。

### 主題總覽

| 主題 | Class | 調性 | Accent | 適用場景 |
|------|-------|------|--------|---------|
| **Frost Glass** | `.theme-frost` | 亮色・毛玻璃・冰藍 | `#2B7FD4` | 白天主力工作 |
| **Deep Void** | `.theme-void` | 暗色・極深黑・靛紫 | `#6366F1` | 夜間・深度專注 |
| **Warm Parchment** | `.theme-warm` | 亮色・羊皮紙米・琥珀 | `#B45309` | 閱讀・典藏感 |
| **Sage Breeze** | `.theme-sage` | 亮色・薄荷白・草地綠 | `#528F44` | 休閒・戶外感 |

---

## 目錄

1. [設計哲學](#1-設計哲學)
2. [Color Tokens — 色彩規範](#2-color-tokens)
3. [Typography Scale — 字型規範](#3-typography-scale)
4. [Component Library — 元件規範](#4-component-library)
5. [知識類型視覺語言](#5-知識類型視覺語言)
6. [動效規範](#6-動效規範)
7. [CSS 完整 Token 宣告](#7-css-完整-token-宣告)

---

## 1. 設計哲學

### 跨主題共用原則

| 原則 | 說明 |
|------|------|
| **層次靠深度** | 卡片、面板、浮層透過不同透明度與模糊度區分，不依賴陰影堆疊 |
| **Accent 要剋制** | 主色只出現在互動元素（按鈕、連結、Active 狀態），不作背景裝飾 |
| **4px 網格絕對** | 所有 spacing、sizing 均為 4 的倍數 |
| **知識語義不變** | Data / Pattern / Log 三色在所有主題下語義不變，只調整背景透明度 |
| **毛玻璃是材質** | `backdrop-filter` 效果是功能性層次感知，而非純裝飾 |

### 各主題設計哲學

**Frost Glass**：「知識的通透感」。亮色底加冰藍 accent，資訊像冰霜清晰可見，介面像玻璃輕盈不遮擋思考。

**Deep Void**：「知識在黑暗中發光」。極深黑底，靛紫 accent，強調沉浸與聚焦，適合需要高度專注的夜間工作。

**Warm Parchment**：「知識典藏的溫度」。羊皮紙米底，琥珀棕 accent，如同翻開實體筆記本，沉穩不刺眼，帶有時間積累的質感。

**Sage Breeze**：「思考需要呼吸空間」。薄荷白底，草地綠 accent，自然有機的休閒感，讓長時間使用不疲勞。

### 光線來源假設

亮色主題（Frost Glass、Warm Parchment、Sage Breeze）假設光線從**左上方**照入，卡片上邊與左邊使用稍亮邊框色（`--stroke-card-highlight`），陰影向右下偏移。暗色主題（Deep Void）邊框色均勻，不套用此規則。

---

## 2. Color Tokens

### 2.1 Surface 層次結構（所有主題通用）

```
Layer 0：App Background   → --surface-base
Layer 1：Sidebar / Panel  → --surface-layer
Layer 2：Card / Section   → --surface-card
Layer 3：Hover / Subtle   → --surface-subtle
Layer 4：Flyout / Popover → --surface-flyout
```

---

### 2.2 Frost Glass 色彩

| Token | 值 | 用途 |
|-------|-----|------|
| `--surface-base` | `#F5F4F1` | App 主背景（帶微暖調的 off-white）|
| `--surface-layer` | `rgba(255, 255, 255, 0.72)` | 側欄、面板（毛玻璃底層）|
| `--surface-card` | `rgba(255, 255, 255, 0.60)` | 卡片、對話氣泡容器 |
| `--surface-subtle` | `rgba(0, 0, 0, 0.03)` | Hover 背景 |
| `--surface-flyout` | `rgba(250, 250, 248, 0.92)` | 選單、Tooltip |
| `--surface-control` | `rgba(255, 255, 255, 0.80)` | Input 背景 |
| `--text-primary` | `rgba(0, 0, 0, 0.88)` | 主要文字 |
| `--text-secondary` | `rgba(0, 0, 0, 0.56)` | 副文字 |
| `--text-tertiary` | `rgba(0, 0, 0, 0.38)` | 時間戳、Hint |
| `--text-link` | `#0066CC` | 超連結 |
| `--accent-default` | `#2B7FD4` | 主色 |
| `--accent-light1` | `#4A95DE` | Hover 主色 |
| `--accent-light2` | `#EBF4FF` | 主色極淡背景 |
| `--accent-dark1` | `#1A6CBD` | Pressed 主色 |
| `--stroke-card` | `rgba(0, 0, 0, 0.07)` | 卡片外框 |
| `--stroke-card-highlight` | `rgba(255, 255, 255, 0.80)` | 卡片光面邊框 |
| `--stroke-control` | `rgba(0, 0, 0, 0.14)` | Input 外框 |
| `--stroke-divider` | `rgba(0, 0, 0, 0.06)` | 分隔線 |

---

### 2.3 Deep Void 色彩

| Token | 值 | 用途 |
|-------|-----|------|
| `--surface-base` | `#131416` | App 主背景（極深黑）|
| `--surface-layer` | `rgba(38, 38, 42, 0.85)` | 側欄、面板 |
| `--surface-card` | `rgba(255, 255, 255, 0.05)` | 卡片 |
| `--surface-subtle` | `rgba(255, 255, 255, 0.04)` | Hover 背景 |
| `--surface-flyout` | `rgba(32, 32, 36, 0.92)` | 選單、Tooltip |
| `--surface-control` | `rgba(255, 255, 255, 0.07)` | Input 背景 |
| `--text-primary` | `rgba(255, 255, 255, 0.92)` | 主要文字 |
| `--text-secondary` | `rgba(255, 255, 255, 0.56)` | 副文字 |
| `--text-tertiary` | `rgba(255, 255, 255, 0.38)` | 時間戳、Hint |
| `--text-link` | `#818CF8` | 超連結 |
| `--accent-default` | `#6366F1` | 主色（靛紫）|
| `--accent-light1` | `#818CF8` | Hover 主色 |
| `--accent-light2` | `rgba(99, 102, 241, 0.14)` | 主色極淡背景 |
| `--accent-dark1` | `#4F46E5` | Pressed 主色 |
| `--stroke-card` | `rgba(255, 255, 255, 0.08)` | 卡片外框 |
| `--stroke-card-highlight` | `rgba(255, 255, 255, 0.14)` | 卡片光面邊框 |
| `--stroke-control` | `rgba(255, 255, 255, 0.12)` | Input 外框 |
| `--stroke-divider` | `rgba(255, 255, 255, 0.06)` | 分隔線 |

**知識類型色彩（Deep Void 調整版）**：

| 類型 | 色條 | Badge 背景 | Badge 文字 |
|------|------|-----------|-----------|
| Data | `#818CF8` | `rgba(99,102,241,0.15)` | `#A5B4FC` |
| Pattern | `#34D399` | `rgba(52,211,153,0.15)` | `#6EE7B7` |
| Log | `#FBBF24` | `rgba(251,191,36,0.15)` | `#FCD34D` |

---

### 2.4 Warm Parchment 色彩

| Token | 值 | 用途 |
|-------|-----|------|
| `--surface-base` | `#F0E8D6` | App 主背景（羊皮紙米）|
| `--surface-layer` | `rgba(255, 252, 240, 0.80)` | 側欄、面板（暖白毛玻璃）|
| `--surface-card` | `rgba(255, 252, 244, 0.75)` | 卡片 |
| `--surface-subtle` | `rgba(120, 90, 40, 0.04)` | Hover 背景 |
| `--surface-flyout` | `rgba(248, 244, 232, 0.94)` | 選單、Tooltip |
| `--surface-control` | `rgba(255, 252, 240, 0.90)` | Input 背景 |
| `--text-primary` | `rgba(40, 28, 12, 0.88)` | 主要文字（深暖棕）|
| `--text-secondary` | `rgba(80, 60, 30, 0.58)` | 副文字 |
| `--text-tertiary` | `rgba(100, 75, 35, 0.40)` | 時間戳、Hint |
| `--text-link` | `#7C4A0A` | 超連結（深琥珀）|
| `--accent-default` | `#B45309` | 主色（琥珀棕）|
| `--accent-light1` | `#CA6D1A` | Hover 主色 |
| `--accent-light2` | `rgba(180, 83, 9, 0.10)` | 主色極淡背景 |
| `--accent-dark1` | `#92400E` | Pressed 主色 |
| `--stroke-card` | `rgba(120, 90, 40, 0.08)` | 卡片外框（暖棕調）|
| `--stroke-card-highlight` | `rgba(255, 252, 240, 0.85)` | 卡片光面邊框 |
| `--stroke-control` | `rgba(120, 90, 40, 0.16)` | Input 外框 |
| `--stroke-divider` | `rgba(120, 90, 40, 0.07)` | 分隔線 |

> **側欄背景補充**：sidebar 使用 `rgba(228, 216, 194, 0.92)` 與主背景形成可見層次差。

---

### 2.5 Sage Breeze 色彩

| Token | 值 | 用途 |
|-------|-----|------|
| `--surface-base` | `#F2F7F0` | App 主背景（薄荷白）|
| `--surface-layer` | `rgba(234, 242, 232, 0.85)` | 側欄、面板（淡鼠尾草）|
| `--surface-card` | `rgba(255, 255, 255, 0.75)` | 卡片 |
| `--surface-subtle` | `rgba(80, 110, 70, 0.04)` | Hover 背景 |
| `--surface-flyout` | `rgba(244, 250, 242, 0.94)` | 選單、Tooltip |
| `--surface-control` | `rgba(255, 255, 255, 0.85)` | Input 背景 |
| `--text-primary` | `rgba(30, 55, 25, 0.88)` | 主要文字（深綠灰）|
| `--text-secondary` | `rgba(50, 80, 40, 0.56)` | 副文字 |
| `--text-tertiary` | `rgba(60, 90, 50, 0.38)` | 時間戳、Hint |
| `--text-link` | `#286019` | 超連結（深草綠）|
| `--accent-default` | `#528F44` | 主色（草地綠）|
| `--accent-light1` | `#62A852` | Hover 主色 |
| `--accent-light2` | `rgba(82, 143, 68, 0.10)` | 主色極淡背景 |
| `--accent-dark1` | `#3D7A2A` | Pressed 主色 |
| `--stroke-card` | `rgba(80, 110, 70, 0.08)` | 卡片外框（有機綠調）|
| `--stroke-card-highlight` | `rgba(255, 255, 255, 0.82)` | 卡片光面邊框 |
| `--stroke-control` | `rgba(80, 110, 70, 0.15)` | Input 外框 |
| `--stroke-divider` | `rgba(80, 110, 70, 0.07)` | 分隔線 |

**知識類型色彩（Sage Breeze 調整版）**：
Pattern 色條改為草地深綠（`#3D7A2A`）與整體綠調協調，Data 冷藍和 Log 琥珀維持語義不變。

| 類型 | Badge 背景 | Badge 文字 |
|------|-----------|-----------|
| Data | `rgba(59,130,246,0.09)` | `#1D4ED8` |
| Pattern | `rgba(82,143,68,0.10)` | `#286019` |
| Log | `rgba(217,119,6,0.10)` | `#92400E` |

---

### 2.6 Elevation（陰影，所有主題）

亮色主題使用輕薄陰影；暗色主題加重不透明度。

| Token | 亮色主題值 | Deep Void 值 |
|-------|-----------|-------------|
| `--shadow-card` | `0 1px 3px rgba(0,0,0,0.06), 0 1px 2px rgba(0,0,0,0.04)` | `0 1px 3px rgba(0,0,0,0.24), 0 1px 2px rgba(0,0,0,0.16)` |
| `--shadow-card-hover` | `0 4px 12px rgba(0,0,0,0.10), 0 1px 3px rgba(0,0,0,0.06)` | `0 4px 12px rgba(0,0,0,0.32), 0 1px 3px rgba(0,0,0,0.20)` |
| `--shadow-flyout` | `0 8px 24px rgba(0,0,0,0.12), 0 2px 6px rgba(0,0,0,0.08)` | `0 8px 24px rgba(0,0,0,0.40), 0 2px 6px rgba(0,0,0,0.24)` |
| `--shadow-dialog` | `0 24px 48px rgba(0,0,0,0.16), 0 4px 12px rgba(0,0,0,0.08)` | `0 24px 48px rgba(0,0,0,0.56), 0 4px 12px rgba(0,0,0,0.28)` |

---

## 3. Typography Scale

### 3.1 字型家族

```
--font-display: "Noto Serif TC", "Georgia", serif;
--font-ui:      "Segoe UI Variable", "PingFang TC", "Microsoft JhengHei UI", system-ui, sans-serif;
--font-mono:    "JetBrains Mono", "Cascadia Code", "Consolas", monospace;
```

> **設計意圖**：標題使用 Noto Serif TC 帶來「知識典藏」的氣質，UI 文字使用 Segoe UI Variable 保持平台一致性，程式碼使用 JetBrains Mono 提升可讀性。四套主題共用同一套字型，僅 Warm Parchment 可選擇將 Display 字體加重（`font-weight: 700`）以強化典藏感。

---

### 3.2 Type Scale

| Token | Font | Weight | Size | Line Height | 用途 |
|-------|------|--------|------|-------------|------|
| `--type-display` | Display | 600 | 40px | 52px | 歡迎頁標題、空狀態標題 |
| `--type-title` | Display | 600 | 28px | 36px | 頁面標題、Space 名稱 |
| `--type-title-sm` | UI | 600 | 20px | 28px | 卡片標題、Section 標題 |
| `--type-subtitle` | UI | 600 | 16px | 24px | 對話標題、清單群組名 |
| `--type-body-lg` | UI | 400 | 16px | 24px | 主要閱讀文字（較舒適）|
| `--type-body` | UI | 400 | 14px | 20px | 預設 UI 文字 |
| `--type-body-strong` | UI | 600 | 14px | 20px | 強調文字、Label |
| `--type-caption` | UI | 400 | 12px | 16px | 輔助說明、時間戳 |
| `--type-caption-strong` | UI | 600 | 12px | 16px | 標籤文字、Badge |
| `--type-overline` | UI | 600 | 11px | 16px | 分類標籤（全大寫）|
| `--type-code` | Mono | 400 | 13px | 20px | 程式碼、指令 |

---

### 3.3 字型使用規則

```
標題層（Display, Title）→ --font-display（Noto Serif TC）
                          僅用於大型展示文字，不用於 UI 控件

UI 層（Subtitle 以下）  → --font-ui
                          所有互動元素、表單、導覽均使用此族

程式碼層               → --font-mono
                          chunk 預覽中的程式碼片段、設定值
```

**禁止行為**：
- ❌ 不得在 12px 以下使用繁體中文
- ❌ 不得在 Body 文字使用 weight 300（細體在毛玻璃背景上難以閱讀）
- ❌ 不得對 UI 文字使用斜體（`font-style: italic`）

---

## 4. Component Library

### 4.1 Border Radius（圓角）

| Token | 值 | 適用元件 |
|-------|-----|---------|
| `--radius-micro` | `2px` | Toggle、Checkbox、小 Tag |
| `--radius-sm` | `4px` | Input、小 Button、Chip |
| `--radius-md` | `8px` | Card、Panel、大 Button |
| `--radius-lg` | `12px` | Dialog、Drawer、Tooltip |
| `--radius-xl` | `16px` | Feature Card、Hero Block |
| `--radius-full` | `9999px` | Avatar、Pill Badge |

> **Sage Breeze 特別說明**：休閒感主題建議 Card 使用 `--radius-lg`（12px），Button 使用 `--radius-md`（8px），讓整體更圓潤柔和。

---

### 4.2 Spacing（間距）

基於 4px 網格：

```
--space-1:  4px    微間距（圖示與文字間）
--space-2:  8px    元素內 padding（Chip, Tag）
--space-3:  12px   Button padding-x
--space-4:  16px   卡片內 padding、行間距
--space-5:  20px   Section 間距（小）
--space-6:  24px   Section 間距（標準）
--space-8:  32px   區塊間距
--space-10: 40px   頁面水平 padding（舒適模式）
--space-12: 48px   大區塊間距
--space-16: 64px   頁面頂部留白
```

---

### 4.3 Button

**Anatomy**：`[Icon?] + Label`

#### 樣式規格

| 變體 | 背景 | 文字 | 邊框 | 用途 |
|------|------|------|------|------|
| **Primary** | `--accent-default` | `--text-on-accent` | 無 | 主要操作（發送、確認）|
| **Secondary** | `--surface-control` | `--text-primary` | `--stroke-control` | 次要操作 |
| **Subtle** | 透明 | `--text-secondary` | 無 | 低強度操作（取消、返回）|
| **Danger** | `#DC2626` | `#FFFFFF` | 無 | 刪除、清除 |

#### 尺寸規格

| 尺寸 | Height | Padding X | Font |
|------|--------|-----------|------|
| `sm` | 28px | 12px | `--type-caption-strong` |
| `md` | 36px | 16px | `--type-body-strong` |
| `lg` | 44px | 20px | `--type-subtitle` |

#### 狀態

```
rest    → 基礎樣式
hover   → background 亮 8%；transition 150ms ease-out
pressed → background 暗 8%；transform: scale(0.98)；transition 80ms
focused → outline: 2px solid --stroke-focus；outline-offset: 2px
disabled→ opacity: 0.4；cursor: not-allowed
```

---

### 4.4 Input / TextBox

**Anatomy**：`[Icon?] + Placeholder/Value + [Action?]`

```css
/* 基礎樣式 */
background:    var(--surface-control);
border:        1px solid var(--stroke-control);
border-radius: var(--radius-sm);
padding:       8px 12px;
font:          var(--type-body);
color:         var(--text-primary);

/* Focus */
border-color:  var(--stroke-focus);
outline:       none;
box-shadow:    0 0 0 1px var(--stroke-focus);

/* Error */
border-color:  #DC2626;
box-shadow:    0 0 0 1px #DC2626;
```

**Chat Input（特殊）**：

```css
background:      var(--surface-card);
backdrop-filter: blur(20px);
border:          1px solid var(--stroke-card-highlight);
border-bottom:   1px solid var(--stroke-control);
border-radius:   var(--radius-md);
padding:         12px 16px;
box-shadow:      var(--shadow-card);
min-height:      52px;
max-height:      200px;
resize:          none;
```

---

### 4.5 Card

Card 是 InsightCAP 中知識單元的核心視覺容器，對應 Chunk 展示。

**Anatomy**：`[Header: Icon + Title + Badge] + [Body: Content] + [Footer: Tags + Timestamp]`

```css
/* Base Card */
background:      var(--surface-card);
backdrop-filter: blur(20px) saturate(180%);
border:          1px solid var(--stroke-card);
border-top:      1px solid var(--stroke-card-highlight);
border-left:     1px solid var(--stroke-card-highlight);
border-radius:   var(--radius-md);
padding:         var(--space-4);
box-shadow:      var(--shadow-card);
transition:      box-shadow 150ms ease-out, transform 150ms ease-out;

/* Hover */
box-shadow:  var(--shadow-card-hover);
transform:   translateY(-1px);

/* Selected */
border-color:  var(--accent-default);
background:    var(--accent-light2);
```

**Card 知識類型變體**：左側 3px 色條標示類型，其餘邊框維持 `--stroke-card`。

| 變體 | 左側邊框色 |
|------|----------|
| `card--data` | `var(--knowledge-data)` |
| `card--pattern` | `var(--knowledge-pattern)` |
| `card--log` | `var(--knowledge-log)` |

---

### 4.6 Navigation（導覽列）

InsightCAP 使用**頂部 Tab Bar + 左側 Pane** 的雙層導覽結構。

#### 頂部 Tab Bar

```css
background:      var(--surface-layer);
backdrop-filter: blur(12px);
border-bottom:   1px solid var(--stroke-divider);
height:          48px;
padding:         0 var(--space-4);
```

Tab Item 狀態：

```css
/* Rest */
color: var(--text-secondary);
font:  var(--type-body);

/* Active */
background: var(--accent-light2);
color:      var(--accent-default);
font:       var(--type-body-strong);
border-radius: var(--radius-sm);
```

#### 左側 Pane

```css
width:           240px;
background:      var(--surface-layer);
backdrop-filter: blur(16px);
border-right:    1px solid var(--stroke-divider);
```

Nav Item 狀態：

```css
/* Rest */
height:        36px;
padding:       0 var(--space-3);
border-radius: var(--radius-sm);
margin:        2px var(--space-2);
color:         var(--text-secondary);

/* Hover */
background: var(--surface-subtle);
color:      var(--text-primary);

/* Active */
background: var(--accent-light2);
color:      var(--accent-default);
font:       var(--type-body-strong);
```

---

### 4.7 Badge / Tag

#### Knowledge Type Badge（知識類型徽章）

```css
display:       inline-flex;
align-items:   center;
gap:           var(--space-1);
height:        22px;
padding:       0 var(--space-2);
border-radius: var(--radius-micro);
font:          var(--type-caption-strong);
```

| 類型 | 背景 Token | 文字 Token |
|------|-----------|-----------|
| Data | `--knowledge-data-bg` | `--knowledge-data-text` |
| Pattern | `--knowledge-pattern-bg` | `--knowledge-pattern-text` |
| Log | `--knowledge-log-bg` | `--knowledge-log-text` |

#### Tag Chip（一般標籤）

```css
height:        20px;
padding:       0 var(--space-2);
border-radius: var(--radius-full);
background:    var(--surface-subtle);
border:        1px solid var(--stroke-card);
font:          var(--type-caption);
color:         var(--text-secondary);
```

---

### 4.8 ContextHintBanner

對話開啟時的主動盤點結果，非阻塞展示（對應 `ContextHintBanner.tsx`）。

```css
background:      var(--surface-card);
backdrop-filter: blur(12px);
border:          1px solid var(--stroke-card);
border-radius:   var(--radius-md);
padding:         var(--space-3) var(--space-4);
margin:          var(--space-3) var(--space-4);
```

標題文字用 `--type-caption` + `--text-tertiary`，知識類型 Chip 繼承 4.7 Badge 樣式。

---

### 4.9 InlineCitationBadge

AI 回答中引用知識來源的 inline 標記（對應 `InlineCitationBadge`）。

```css
display:        inline-flex;
align-items:    center;
height:         18px;
padding:        0 6px;
border-radius:  var(--radius-micro);
font:           var(--type-overline);
font-size:      10px;
vertical-align: middle;
margin:         0 2px;
cursor:         pointer;
transition:     background 100ms ease-out;
```

| 來源類型 | 背景 | 文字 |
|---------|------|------|
| Data chunk | `--knowledge-data-bg` | `--knowledge-data-text` |
| Pattern chunk | `--knowledge-pattern-bg` | `--knowledge-pattern-text` |
| Log chunk | `--knowledge-log-bg` | `--knowledge-log-text` |
| External KB | `rgba(139, 92, 246, 0.08)` | `#6D28D9` |
| Web search | `rgba(107, 114, 128, 0.08)` | `var(--text-secondary)` |

---

### 4.10 Titlebar

```css
.titlebar {
  height:          32px;
  background:      var(--surface-layer);
  backdrop-filter: blur(20px);
  border-bottom:   1px solid var(--stroke-divider);
  display:         flex;
  align-items:     center;
  padding:         0 0 0 12px;
  position:        fixed;
  top: 0; left: 0; right: 0;
  z-index:         9999;
}

.titlebar-btn { width: 46px; height: 32px; color: var(--text-secondary); background: transparent; }
.titlebar-btn:hover { background: var(--surface-subtle); color: var(--text-primary); }
.btn-close:hover { background: #C42B1C; color: #FFFFFF; }
```

---

### 4.11 Scrollbar

```css
::-webkit-scrollbar        { width: 6px; height: 6px; }
::-webkit-scrollbar-track  { background: transparent; }
::-webkit-scrollbar-thumb  { background: var(--stroke-control); border-radius: var(--radius-full); }
::-webkit-scrollbar-thumb:hover { background: var(--stroke-strong); }
```

---

### 4.12 Toast / Notification

```css
background:      var(--surface-flyout);
backdrop-filter: blur(20px);
border:          1px solid var(--stroke-card);
border-radius:   var(--radius-md);
box-shadow:      var(--shadow-flyout);
padding:         var(--space-3) var(--space-4);
min-width:       280px;
max-width:       400px;
```

左側 accent 色條：Info = `--accent-default`、Success = `#16A34A`、Warning = `--knowledge-log`、Error = `#DC2626`。

---

## 5. 知識類型視覺語言

InsightCAP 三層記憶理論在視覺層的完整對應，**跨所有主題保持語義不變**。

### 5.1 色彩語義（亮色主題通用）

| 知識類型 | Token | 色值 | 心理意象 |
|---------|-------|------|---------|
| **Data** | `--knowledge-data` | `#3B82F6`（冷藍）| 事實、資料、可信賴 |
| **Pattern** | `--knowledge-pattern` | `#14B8A6`（青綠）| 積累、成長、可複用 |
| **Log** | `--knowledge-log` | `#F59E0B`（琥珀）| 警示、經驗、值得注意 |

Deep Void 在亮度上調整（見 2.3），但語義圖示與色系方向不變。

### 5.2 圖示語義（所有主題）

| 知識類型 | 圖示 | 含義 |
|---------|------|------|
| Data | `●`（實心圓）| 穩定的事實，有邊界 |
| Pattern | `◆`（菱形）| 可旋轉的框架，多角度應用 |
| Log | `▲`（三角）| 警示符號，引人注意 |

### 5.3 知識類型在各元件的一致表現

```
Card          → 左側 3px 色條
Badge         → 彩色背景徽章（Card header）
ContextHint   → 彩色邊框 Chip
Citation      → 彩色 inline 標記
Filter Button → 彩色 dot + 文字
RAG 搜尋結果  → 左側色條 + 類型 Badge
```

**設計原則**：三種知識類型的顏色在任何情境下必須保持一致，不得因元件不同而改變。使用者學習一次，終身識別。

---

## 6. 動效規範

### 6.1 Timing Tokens（所有主題共用）

```css
--duration-instant: 80ms;   /* 按下反饋、顏色切換 */
--duration-fast:    150ms;  /* Hover 狀態、圖示動畫 */
--duration-normal:  200ms;  /* 卡片出現、Panel 展開 */
--duration-slow:    300ms;  /* Dialog 進場、頁面切換 */
--duration-crawl:   400ms;  /* 教學動畫、引導動效 */

--ease-fluent:   cubic-bezier(0.1, 0.9, 0.2, 1);
--ease-out:      cubic-bezier(0, 0, 0.2, 1);
--ease-in-out:   cubic-bezier(0.4, 0, 0.2, 1);
--ease-spring:   cubic-bezier(0.34, 1.56, 0.64, 1);
```

### 6.2 互動動效規則

| 場景 | 時長 | Easing | 效果 |
|------|------|--------|------|
| Button Hover | 150ms | ease-out | background 變化 |
| Button Press | 80ms | ease-out | scale(0.98) |
| Card Hover | 150ms | ease-out | translateY(-1px) + shadow 增強 |
| Panel 展開 | 250ms | --ease-fluent | height 展開 + opacity 0→1 |
| Dialog 進場 | 300ms | --ease-fluent | scale(0.96→1) + opacity 0→1 |
| Toast 進場 | 200ms | --ease-spring | translateY(8px→0) + opacity 0→1 |
| ContextHint 進場 | 200ms | --ease-fluent | opacity 0→1 + translateY(-4px→0) |
| 主題切換 | 250ms | ease-in-out | 所有 CSS 變數平滑過渡 |

### 6.3 主題切換動效

```css
/* 在 :root 加入，讓主題切換時所有 token 平滑過渡 */
*, *::before, *::after {
  transition:
    background-color var(--duration-slow) var(--ease-in-out),
    border-color     var(--duration-slow) var(--ease-in-out),
    color            var(--duration-slow) var(--ease-in-out);
}
```

### 6.4 減少動效模式

```css
@media (prefers-reduced-motion: reduce) {
  *, *::before, *::after {
    animation-duration:   0.01ms !important;
    transition-duration:  0.01ms !important;
  }
}
```

---

## 7. CSS 完整 Token 宣告

可直接複製到 `src/styles/design-tokens.css`：

```css
/* ============================================================
   InsightCAP Design System — 完整 Token 宣告 v1.1
   支援四套主題：theme-frost / theme-void / theme-warm / theme-sage
   ============================================================ */

/* === 共用基礎（Radius / Spacing / Typography / Motion / Knowledge） === */
:root {
  /* RADIUS */
  --radius-micro: 2px;
  --radius-sm:    4px;
  --radius-md:    8px;
  --radius-lg:    12px;
  --radius-xl:    16px;
  --radius-full:  9999px;

  /* SPACING */
  --space-1:  4px;
  --space-2:  8px;
  --space-3:  12px;
  --space-4:  16px;
  --space-5:  20px;
  --space-6:  24px;
  --space-8:  32px;
  --space-10: 40px;
  --space-12: 48px;
  --space-16: 64px;

  /* TYPOGRAPHY */
  --font-display: "Noto Serif TC", "Georgia", serif;
  --font-ui:      "Segoe UI Variable", "PingFang TC", "Microsoft JhengHei UI", system-ui, sans-serif;
  --font-mono:    "JetBrains Mono", "Cascadia Code", "Consolas", monospace;

  --type-display:        600 40px/52px var(--font-display);
  --type-title:          600 28px/36px var(--font-display);
  --type-title-sm:       600 20px/28px var(--font-ui);
  --type-subtitle:       600 16px/24px var(--font-ui);
  --type-body-lg:        400 16px/24px var(--font-ui);
  --type-body:           400 14px/20px var(--font-ui);
  --type-body-strong:    600 14px/20px var(--font-ui);
  --type-caption:        400 12px/16px var(--font-ui);
  --type-caption-strong: 600 12px/16px var(--font-ui);
  --type-overline:       600 11px/16px var(--font-ui);
  --type-code:           400 13px/20px var(--font-mono);

  /* MOTION */
  --duration-instant: 80ms;
  --duration-fast:    150ms;
  --duration-normal:  200ms;
  --duration-slow:    300ms;
  --duration-crawl:   400ms;
  --ease-fluent:      cubic-bezier(0.1, 0.9, 0.2, 1);
  --ease-out:         cubic-bezier(0, 0, 0.2, 1);
  --ease-in-out:      cubic-bezier(0.4, 0, 0.2, 1);
  --ease-spring:      cubic-bezier(0.34, 1.56, 0.64, 1);

  /* KNOWLEDGE TYPE（亮色主題通用） */
  --knowledge-data:         #3B82F6;
  --knowledge-data-bg:      rgba(59, 130, 246, 0.10);
  --knowledge-data-text:    #1D4ED8;
  --knowledge-pattern:      #14B8A6;
  --knowledge-pattern-bg:   rgba(20, 184, 166, 0.10);
  --knowledge-pattern-text: #0F766E;
  --knowledge-log:          #F59E0B;
  --knowledge-log-bg:       rgba(245, 158, 11, 0.10);
  --knowledge-log-text:     #B45309;

  /* SHARED SEMANTIC */
  --text-on-accent:   #FFFFFF;
  --stroke-focus:     var(--accent-default);
}

/* ============================================================
   THEME: Frost Glass（預設 / 亮色・冰藍）
   ============================================================ */
.theme-frost {
  --surface-base:          #F5F4F1;
  --surface-layer:         rgba(255, 255, 255, 0.72);
  --surface-card:          rgba(255, 255, 255, 0.60);
  --surface-subtle:        rgba(0, 0, 0, 0.03);
  --surface-flyout:        rgba(250, 250, 248, 0.92);
  --surface-control:       rgba(255, 255, 255, 0.80);
  --surface-control-hover: rgba(255, 255, 255, 0.92);

  --text-primary:          rgba(0, 0, 0, 0.88);
  --text-secondary:        rgba(0, 0, 0, 0.56);
  --text-tertiary:         rgba(0, 0, 0, 0.38);
  --text-disabled:         rgba(0, 0, 0, 0.28);
  --text-link:             #0066CC;
  --text-link-hover:       #0052A3;

  --accent-default:        #2B7FD4;
  --accent-light1:         #4A95DE;
  --accent-light2:         #EBF4FF;
  --accent-dark1:          #1A6CBD;
  --accent-dark2:          #155A9E;

  --stroke-card:           rgba(0, 0, 0, 0.07);
  --stroke-card-highlight: rgba(255, 255, 255, 0.80);
  --stroke-control:        rgba(0, 0, 0, 0.14);
  --stroke-divider:        rgba(0, 0, 0, 0.06);
  --stroke-strong:         rgba(0, 0, 0, 0.40);

  --shadow-card:           0 1px 3px rgba(0,0,0,0.06), 0 1px 2px rgba(0,0,0,0.04);
  --shadow-card-hover:     0 4px 12px rgba(0,0,0,0.10), 0 1px 3px rgba(0,0,0,0.06);
  --shadow-flyout:         0 8px 24px rgba(0,0,0,0.12), 0 2px 6px rgba(0,0,0,0.08);
  --shadow-dialog:         0 24px 48px rgba(0,0,0,0.16), 0 4px 12px rgba(0,0,0,0.08);
}

/* ============================================================
   THEME: Deep Void（暗色・極深黑・靛紫）
   ============================================================ */
.theme-void {
  --surface-base:          #131416;
  --surface-layer:         rgba(38, 38, 42, 0.85);
  --surface-card:          rgba(255, 255, 255, 0.05);
  --surface-subtle:        rgba(255, 255, 255, 0.04);
  --surface-flyout:        rgba(32, 32, 36, 0.92);
  --surface-control:       rgba(255, 255, 255, 0.07);
  --surface-control-hover: rgba(255, 255, 255, 0.10);

  --text-primary:          rgba(255, 255, 255, 0.92);
  --text-secondary:        rgba(255, 255, 255, 0.56);
  --text-tertiary:         rgba(255, 255, 255, 0.38);
  --text-disabled:         rgba(255, 255, 255, 0.28);
  --text-link:             #818CF8;
  --text-link-hover:       #A5B4FC;

  --accent-default:        #6366F1;
  --accent-light1:         #818CF8;
  --accent-light2:         rgba(99, 102, 241, 0.14);
  --accent-dark1:          #4F46E5;
  --accent-dark2:          #4338CA;

  --stroke-card:           rgba(255, 255, 255, 0.08);
  --stroke-card-highlight: rgba(255, 255, 255, 0.14);
  --stroke-control:        rgba(255, 255, 255, 0.12);
  --stroke-divider:        rgba(255, 255, 255, 0.06);
  --stroke-strong:         rgba(255, 255, 255, 0.45);

  --shadow-card:           0 1px 3px rgba(0,0,0,0.24), 0 1px 2px rgba(0,0,0,0.16);
  --shadow-card-hover:     0 4px 12px rgba(0,0,0,0.32), 0 1px 3px rgba(0,0,0,0.20);
  --shadow-flyout:         0 8px 24px rgba(0,0,0,0.40), 0 2px 6px rgba(0,0,0,0.24);
  --shadow-dialog:         0 24px 48px rgba(0,0,0,0.56), 0 4px 12px rgba(0,0,0,0.28);

  /* 知識類型覆寫（深色調整）*/
  --knowledge-data:         #818CF8;
  --knowledge-data-bg:      rgba(99, 102, 241, 0.15);
  --knowledge-data-text:    #A5B4FC;
  --knowledge-pattern:      #34D399;
  --knowledge-pattern-bg:   rgba(52, 211, 153, 0.15);
  --knowledge-pattern-text: #6EE7B7;
  --knowledge-log:          #FBBF24;
  --knowledge-log-bg:       rgba(251, 191, 36, 0.15);
  --knowledge-log-text:     #FCD34D;
}

/* ============================================================
   THEME: Warm Parchment（亮色・羊皮紙米・琥珀）
   ============================================================ */
.theme-warm {
  --surface-base:          #F0E8D6;
  --surface-layer:         rgba(255, 252, 240, 0.80);
  --surface-card:          rgba(255, 252, 244, 0.75);
  --surface-subtle:        rgba(120, 90, 40, 0.04);
  --surface-flyout:        rgba(248, 244, 232, 0.94);
  --surface-control:       rgba(255, 252, 240, 0.90);
  --surface-control-hover: rgba(255, 255, 248, 0.96);

  --text-primary:          rgba(40, 28, 12, 0.88);
  --text-secondary:        rgba(80, 60, 30, 0.58);
  --text-tertiary:         rgba(100, 75, 35, 0.40);
  --text-disabled:         rgba(100, 75, 35, 0.30);
  --text-link:             #7C4A0A;
  --text-link-hover:       #92400E;

  --accent-default:        #B45309;
  --accent-light1:         #CA6D1A;
  --accent-light2:         rgba(180, 83, 9, 0.10);
  --accent-dark1:          #92400E;
  --accent-dark2:          #78350F;

  --stroke-card:           rgba(120, 90, 40, 0.08);
  --stroke-card-highlight: rgba(255, 252, 240, 0.85);
  --stroke-control:        rgba(120, 90, 40, 0.16);
  --stroke-divider:        rgba(120, 90, 40, 0.07);
  --stroke-strong:         rgba(80, 60, 30, 0.40);

  --shadow-card:           0 1px 3px rgba(80,50,10,0.06), 0 1px 2px rgba(80,50,10,0.04);
  --shadow-card-hover:     0 4px 12px rgba(80,50,10,0.10), 0 1px 3px rgba(80,50,10,0.06);
  --shadow-flyout:         0 8px 24px rgba(80,50,10,0.12), 0 2px 6px rgba(80,50,10,0.08);
  --shadow-dialog:         0 24px 48px rgba(80,50,10,0.16), 0 4px 12px rgba(80,50,10,0.08);
}

/* ============================================================
   THEME: Sage Breeze（亮色・薄荷白・草地綠）
   ============================================================ */
.theme-sage {
  --surface-base:          #F2F7F0;
  --surface-layer:         rgba(234, 242, 232, 0.85);
  --surface-card:          rgba(255, 255, 255, 0.75);
  --surface-subtle:        rgba(80, 110, 70, 0.04);
  --surface-flyout:        rgba(244, 250, 242, 0.94);
  --surface-control:       rgba(255, 255, 255, 0.85);
  --surface-control-hover: rgba(255, 255, 255, 0.96);

  --text-primary:          rgba(30, 55, 25, 0.88);
  --text-secondary:        rgba(50, 80, 40, 0.56);
  --text-tertiary:         rgba(60, 90, 50, 0.38);
  --text-disabled:         rgba(60, 90, 50, 0.30);
  --text-link:             #286019;
  --text-link-hover:       #1A4A10;

  --accent-default:        #528F44;
  --accent-light1:         #62A852;
  --accent-light2:         rgba(82, 143, 68, 0.10);
  --accent-dark1:          #3D7A2A;
  --accent-dark2:          #2D6020;

  --stroke-card:           rgba(80, 110, 70, 0.08);
  --stroke-card-highlight: rgba(255, 255, 255, 0.82);
  --stroke-control:        rgba(80, 110, 70, 0.15);
  --stroke-divider:        rgba(80, 110, 70, 0.07);
  --stroke-strong:         rgba(50, 80, 40, 0.40);

  --shadow-card:           0 1px 3px rgba(40,70,30,0.06), 0 1px 2px rgba(40,70,30,0.04);
  --shadow-card-hover:     0 4px 12px rgba(40,70,30,0.10), 0 1px 3px rgba(40,70,30,0.06);
  --shadow-flyout:         0 8px 24px rgba(40,70,30,0.12), 0 2px 6px rgba(40,70,30,0.08);
  --shadow-dialog:         0 24px 48px rgba(40,70,30,0.16), 0 4px 12px rgba(40,70,30,0.08);

  /* 知識類型覆寫（Pattern 改草地深綠）*/
  --knowledge-pattern:      #3D7A2A;
  --knowledge-pattern-bg:   rgba(82, 143, 68, 0.10);
  --knowledge-pattern-text: #286019;
}

/* ============================================================
   全域共用樣式
   ============================================================ */

/* Focus Ring */
:focus-visible {
  outline:        2px solid var(--stroke-focus);
  outline-offset: 2px;
}

/* Scrollbar */
::-webkit-scrollbar        { width: 6px; height: 6px; }
::-webkit-scrollbar-track  { background: transparent; }
::-webkit-scrollbar-thumb  {
  background:    var(--stroke-control);
  border-radius: var(--radius-full);
}
::-webkit-scrollbar-thumb:hover { background: var(--stroke-strong); }

/* 主題切換平滑過渡 */
*, *::before, *::after {
  transition:
    background-color var(--duration-slow) var(--ease-in-out),
    border-color     var(--duration-slow) var(--ease-in-out),
    color            var(--duration-normal) var(--ease-in-out);
}

/* 減少動效模式 */
@media (prefers-reduced-motion: reduce) {
  *, *::before, *::after {
    animation-duration:  0.01ms !important;
    transition-duration: 0.01ms !important;
  }
}
```

---

## 附錄：themeStore 更新建議

現有 `themeStore` 的型別從 `'dark' | 'light' | 'warm'` 更新為：

```typescript
type ThemeMode = 'frost' | 'void' | 'warm' | 'sage';
```

`<html>` 套用 class 的邏輯不需改動，直接對應：
- `theme-frost` → Frost Glass
- `theme-void` → Deep Void
- `theme-warm` → Warm Parchment
- `theme-sage` → Sage Breeze

---

*InsightCAP Design System v1.1*
*更新日期：2026-03-29*
*下次更新時機：Phase 4 手機版啟動前*
