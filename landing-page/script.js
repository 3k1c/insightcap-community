window.tailwind = window.tailwind || {};
window.tailwind.config = {
            theme: {
                extend: {
                    fontFamily: {
                        sans: ['"Plus Jakarta Sans"', 'sans-serif'],
                        mono: ['"JetBrains Mono"', 'monospace'],
                    },
                    colors: {
                        brand: { 50: '#eef2ff', 100: '#e0e7ff', 500: '#6366f1', 600: '#4f46e5', 900: '#312e81' },
                        accent: { 500: '#0ea5e9' }
                    },
                    animation: {
                        'blob': 'blob 10s infinite alternate',
                        'pulse-slow': 'pulse 3s cubic-bezier(0.4, 0, 0.6, 1) infinite',
                    },
                    keyframes: {
                        blob: {
                            '0%': { transform: 'translate(0px, 0px) scale(1)' },
                            '33%': { transform: 'translate(30px, -50px) scale(1.1)' },
                            '66%': { transform: 'translate(-20px, 20px) scale(0.9)' },
                            '100%': { transform: 'translate(0px, 0px) scale(1)' },
                        }
                    }
                }
            }
        }

// Multi-language Dictionary
        const translations = {
            en: {
                nav_features: "Features",
                nav_workflow: "Workflow",
                nav_testimonials: "Testimonials",
                nav_faq: "FAQ",
                btn_download: "Download for Windows",
                btn_download_large: "Download Free — Windows",
                btn_github: "View on GitHub",
                
                hero_badge: "Community Edition — Free Download",
                hero_title: "Capture once.<br>Remember <span class=\"text-gradient\">forever.</span>",
                hero_subtitle: "A local-first knowledge system for researchers, writers, and developers. Turn scattered web pages and notes into a connected, permanent memory.",
                hero_microcopy: "<i data-lucide=\"shield-check\" class=\"w-3.5 h-3.5\"></i> 100% Local · No Sign-up",
                mockup_header: "<i data-lucide=\"lock\" class=\"w-3 h-3\"></i> Knowledge Base",
                
                audience_title: "Designed specifically for",
                audience_1: "Researchers",
                audience_2: "Developers",
                audience_3: "Content Creators",
                audience_4: "Officers",

                feat_badge: "Core Features",
                feat_title: "Everything you need.<br><span class=\"text-slate-400\">Nothing you don't.</span>",
                feat_desc: "Built for the way knowledge workers actually work — not how productivity gurus say they should.",
                feat_1_title: "Multi-source Capture",
                feat_1_desc: "Capture web pages, local documents, YouTube videos, clipboard content, and Telegram messages seamlessly into one base.",
                feat_1_tag: "Web · Docs · YouTube · Telegram",
                feat_2_title: "Long-term Memory",
                feat_2_desc: "Your knowledge base is encrypted and permanent. Search anything you've ever captured in milliseconds.",
                feat_2_tag: "Instant Search · Persistent",
                feat_3_title: "Projects & Context",
                feat_3_desc: "Create projects that link related knowledge. Research, planning, and writing stay natively connected to original material.",
                feat_3_tag: "Research · Planning · Writing",
                feat_4_title: "AI Editing & Export",
                feat_4_desc: "Turn captured content into polished drafts using AI-assisted editing. Export perfectly formatted documents in multiple formats.",
                feat_4_tag: "TXT · MD · HTML · DOCX · PDF",
                feat_5_title: "Telegram Bot Capture",
                feat_5_desc: "Send links or notes to your personal bot from anywhere. They automatically appear in your base when you're back at your desk.",
                feat_5_tag: "Mobile · On-the-go Sync",
                feat_6_title: "Reminders & Follow-up",
                feat_6_desc: "Set reminders on captured knowledge so important articles, ideas, and tasks resurface at the exact right time.",
                feat_6_tag: "Reminders · Q&A",
                
                wf_badge: "How It Works",
                wf_title: "A seamless workflow <br><span class=\"text-brand-400\">for modern thinkers</span>",
                wf_step1_title: "1. Universal Capture",
                wf_step1_desc: "Grab anything from the web, local documents, videos, or Telegram. One click is all it takes.",
                wf_step2_title: "2. Organize",
                wf_step2_desc: "Content is automatically extracted, indexed locally, and prepared for instant search.",
                wf_step3_title: "3. Connect",
                wf_step3_desc: "Create projects that link relevant knowledge with AI discussions to preserve important research context.",
                wf_step4_title: "4. Create & Export",
                wf_step4_desc: "Use the AI editor to draft reports or notes based on your captured knowledge. Export perfectly to Markdown, PDF, or Word.",
                tag_web: "Web", tag_docs: "Docs", tag_telegram: "Telegram",
                
                priv_badge: "Privacy First",
                priv_title: "Your knowledge.<br>Your <span class=\"text-brand-400\">device.</span>",
                priv_desc1: "InsightCAP is built local-first by design. Your data never touches our servers because we don't have any.",
                priv_box_title: "Fully Encrypted",
                priv_box_desc: "AES-256 encryption secures your database under your Windows account, protected by a 24-word recovery phrase.",
                
                testi_badge: "Wall of Love",
                testi_title: "Loved by knowledge workers",
                testi_1_desc: "\"InsightCAP completely changed how I organize my research. I no longer lose track of important articles, and the fact that it's 100% local gives me peace of mind.\"",
                testi_1_name: "John Doe",
                testi_1_role: "Software Engineer",
                testi_2_desc: "\"The Telegram bot integration is a game-changer. I just forward links to the bot while commuting, and they are indexed and searchable when I open my laptop.\"",
                testi_2_name: "Alice Smith",
                testi_2_role: "Content Writer",
                testi_3_desc: "\"I was tired of subscription-based cloud tools locking my data. InsightCAP's local-first architecture and instant search is exactly what I was looking for.\"",
                testi_3_name: "Michael R.",
                testi_3_role: "PhD Researcher",

                faq_title: "Frequently Asked Questions",
                faq_q1: "Is it really completely free?",
                faq_a1: "Yes. The Community Edition is completely free for both personal and commercial use. No hidden fees or usage limits.",
                faq_q2: "Does it support Mac or Linux?",
                faq_a2: "Currently, InsightCAP is optimized for Windows to utilize its local encryption protocols. Support for other platforms is being explored.",
                faq_q3: "How is this different from Notion or Obsidian?",
                faq_a3: "Unlike Notion, your data is 100% local and encrypted. Unlike Obsidian, InsightCAP focuses heavily on automated extraction from external sources (Web, YouTube, Telegram).",
                faq_q4: "What happens if I change my computer?",
                faq_a4_1: "InsightCAP provides a built-in Export/Import utility to safely migrate your knowledge base. Simply copy your local database folder to your new computer and use your 24-word recovery phrase to unlock it.",
                faq_a4_2: "This ensures your data remains completely offline and under your direct control at all times.",
                
                cta_title: "Stop losing<br>what you <span class=\"text-brand-600\">already know.</span>",
                cta_desc: "InsightCAP Community Edition is free to download. Get started building your personal memory today.",
                cta_microcopy: "100% Local · No Sign-up Required",
                
                footer_repo: "Repository",
                footer_releases: "Releases"
            },
            "zh-TW": {
                nav_features: "特色",
                nav_workflow: "工作流",
                nav_testimonials: "用戶見證",
                nav_faq: "常見問題",
                btn_download: "下載 Windows 版",
                btn_download_large: "免費下載 — Windows",
                btn_github: "在 GitHub 查看",
                
                hero_badge: "Community Edition — 免費下載",
                hero_title: "一次擷取，<span class=\"text-gradient\">永久銘記。</span>",
                hero_subtitle: "專為研究人員、作家與開發者設計的本地優先知識系統。將零散的網頁與筆記，轉化為彼此連結的永久記憶。",
                hero_microcopy: "<i data-lucide=\"shield-check\" class=\"w-3.5 h-3.5\"></i> 100% 本地儲存 · 無需註冊",
                mockup_header: "<i data-lucide=\"lock\" class=\"w-3 h-3\"></i> 知識庫",
                
                audience_title: "專為這些群體設計",
                audience_1: "研究人員",
                audience_2: "開發人員",
                audience_3: "內容創作者",
                audience_4: "辦公人員",

                feat_badge: "核心功能",
                feat_title: "需要的功能一次到位。<br><span class=\"text-slate-400\">不需要的負擔全部省去。</span>",
                feat_desc: "依照知識工作者真實的工作方式打造，而不是照著效率口號堆功能。",
                feat_1_title: "多來源擷取",
                feat_1_desc: "將網頁、本地文件、YouTube 影片、剪貼簿內容與 Telegram 訊息順暢收進同一個知識庫。",
                feat_1_tag: "Web · 文件 · YouTube · Telegram",
                feat_2_title: "長期記憶庫",
                feat_2_desc: "你的知識庫會被加密並長期保存。曾經擷取過的內容，都能在毫秒內搜尋出來。",
                feat_2_tag: "即時搜尋 · 持久保存",
                feat_3_title: "專案與脈絡",
                feat_3_desc: "建立專案來連結相關知識，讓研究、規劃與寫作始終保留原始資料脈絡。",
                feat_3_tag: "研究 · 規劃 · 寫作",
                feat_4_title: "AI 編輯與匯出",
                feat_4_desc: "使用 AI 輔助編輯，將擷取內容整理成完整草稿，並匯出為多種格式的文件。",
                feat_4_tag: "TXT · MD · HTML · DOCX · PDF",
                feat_5_title: "Telegram Bot 擷取",
                feat_5_desc: "在任何地方把連結或筆記傳給個人 bot，回到桌面後它們就會自動出現在知識庫中。",
                feat_5_tag: "Mobile · 隨手同步",
                feat_6_title: "提醒與追蹤",
                feat_6_desc: "為擷取的知識設定提醒，讓重要文章、想法與任務在正確時間重新浮現。",
                feat_6_tag: "提醒 · 問答",
                
                wf_badge: "運作方式",
                wf_title: "專為現代思考者打造的<br><span class=\"text-brand-400\">無縫工作流</span>",
                wf_step1_title: "1. 全方位擷取",
                wf_step1_desc: "從網頁、本地文件、影片或 Telegram 輕鬆獲取資訊。一切盡在掌握。",
                wf_step2_title: "2. 智能組織",
                wf_step2_desc: "內容自動提取並建立索引，為瞬間搜尋做好準備。",
                wf_step3_title: "3. 深度連結",
                wf_step3_desc: "建立專案，將相關知識與 AI 討論連結起來，保留重要的研究脈絡。",
                wf_step4_title: "4. 創作與匯出",
                wf_step4_desc: "使用 AI 編輯器草擬報告或筆記。完美匯出為 Markdown、PDF 或 Word。",
                tag_web: "網頁", tag_docs: "文件", tag_telegram: "Telegram",
                
                priv_badge: "隱私至上",
                priv_title: "你的知識，<br>存在你的 <span class=\"text-brand-400\">設備。</span>",
                priv_desc1: "InsightCAP 採用本地優先設計。你的資料永遠不會離開你的電腦，因為我們根本沒有伺服器。",
                priv_box_title: "完全加密",
                priv_box_desc: "採用 AES-256 加密，金鑰安全儲存於 Windows 帳戶下，配合 24 位元復原助記詞提供極致保護。",
                
                testi_badge: "用戶見證",
                testi_title: "深受知識工作者喜愛",
                testi_1_desc: "「InsightCAP 徹底改變了我組織研究資料的方式。我再也不會弄丟重要的文章，而且 100% 本地儲存讓我非常安心。」",
                testi_1_name: "John Doe",
                testi_1_role: "軟體工程師",
                testi_2_desc: "「Telegram 機器人的整合太棒了。我只需在通勤時將連結轉發給機器人，打開筆電時它們就已經建立好索引並可以搜尋了。」",
                testi_2_name: "Alice Smith",
                testi_2_role: "內容創作者",
                testi_3_desc: "「我受夠了訂閱制的雲端工具綁架我的資料。InsightCAP 的本地優先架構和即時搜尋，正是我一直在尋找的工具。」",
                testi_3_name: "Michael R.",
                testi_3_role: "博士研究員",

                faq_title: "常見問題",
                faq_q1: "這真的是完全免費的嗎？",
                faq_a1: "是的。社群版（Community Edition）對於個人和商業用途均為免費，沒有隱藏費用或使用限制。",
                faq_q2: "有支援 Mac 或 Linux 嗎？",
                faq_a2: "目前 InsightCAP 專為 Windows 優化，以充分利用其本地加密協議。我們正在評估支援其他平台的可能性。",
                faq_q3: "這跟 Notion 或 Obsidian 有什麼不同？",
                faq_a3: "與 Notion 不同，您的資料是 100% 本地且加密的（無雲端依賴）。與 Obsidian 不同，InsightCAP 更專注於從外部來源（網頁、YouTube、Telegram）進行自動化擷取與解析。",
                faq_q4: "如果我換電腦了怎麼辦？",
                faq_a4_1: "InsightCAP 內建匯出/匯入工具，讓您安全遷移知識庫。您只需將本地資料夾複製到新電腦，並使用您的 24 位元復原助記詞解鎖即可。",
                faq_a4_2: "這確保您的資料始終保持離線，且隨時在您的完全掌控之中。",
                
                cta_title: "停止遺忘<br>你 <span class=\"text-brand-600\">早已擁有的知識。</span>",
                cta_desc: "InsightCAP 社群版現可免費下載。今天就開始建立你的個人記憶庫。",
                cta_microcopy: "100% 本地儲存 · 無需註冊",
                
                footer_repo: "原始碼專案",
                footer_releases: "下載頁面"
            },
            "zh-CN": {
                nav_features: "特色",
                nav_workflow: "工作流",
                nav_testimonials: "用户评价",
                nav_faq: "常见问题",
                btn_download: "下载 Windows 版",
                btn_download_large: "免费下载 — Windows",
                btn_github: "在 GitHub 查看",
                
                hero_badge: "Community Edition — 免费下载",
                hero_title: "一次抓取，<span class=\"text-gradient\">永久铭记。</span>",
                hero_subtitle: "专为研究人员、作家与开发者设计的本地优先知识系统。将零散的网页与笔记，转化为彼此链接的永久记忆。",
                hero_microcopy: "<i data-lucide=\"shield-check\" class=\"w-3.5 h-3.5\"></i> 100% 本地存储 · 无需注册",
                mockup_header: "<i data-lucide=\"lock\" class=\"w-3 h-3\"></i> 知识库",
                
                audience_title: "专为这些群体设计",
                audience_1: "研究人员",
                audience_2: "开发人员",
                audience_3: "内容创作者",
                audience_4: "办公人员",

                feat_badge: "核心功能",
                feat_title: "需要的功能一次到位。<br><span class=\"text-slate-400\">不需要的负担全部省去。</span>",
                feat_desc: "按照知识工作者真实的工作方式打造，而不是照着效率口号堆功能。",
                feat_1_title: "多来源抓取",
                feat_1_desc: "将网页、本地文件、YouTube 视频、剪贴板内容与 Telegram 消息顺畅收进同一个知识库。",
                feat_1_tag: "Web · 文档 · YouTube · Telegram",
                feat_2_title: "长期记忆库",
                feat_2_desc: "你的知识库会被加密并长期保存。曾经抓取过的内容，都能在毫秒内搜索出来。",
                feat_2_tag: "即时搜索 · 持久保存",
                feat_3_title: "项目与脉络",
                feat_3_desc: "建立项目来链接相关知识，让研究、规划与写作始终保留原始资料脉络。",
                feat_3_tag: "研究 · 规划 · 写作",
                feat_4_title: "AI 编辑与导出",
                feat_4_desc: "使用 AI 辅助编辑，将抓取内容整理成完整草稿，并导出为多种格式的文档。",
                feat_4_tag: "TXT · MD · HTML · DOCX · PDF",
                feat_5_title: "Telegram Bot 抓取",
                feat_5_desc: "在任何地方把链接或笔记发给个人 bot，回到桌面后它们就会自动出现在知识库中。",
                feat_5_tag: "Mobile · 随手同步",
                feat_6_title: "提醒与跟进",
                feat_6_desc: "为抓取的知识设置提醒，让重要文章、想法与任务在正确时间重新浮现。",
                feat_6_tag: "提醒 · 问答",
                
                wf_badge: "运作方式",
                wf_title: "专为现代思考者打造的<br><span class=\"text-brand-400\">无缝工作流</span>",
                wf_step1_title: "1. 全方位抓取",
                wf_step1_desc: "从网页、本地文件、视频或 Telegram 轻松获取信息。一切尽在掌握。",
                wf_step2_title: "2. 智能组织",
                wf_step2_desc: "内容自动提取并建立索引，为瞬间搜索做好准备。",
                wf_step3_title: "3. 深度链接",
                wf_step3_desc: "建立项目，将相关知识与 AI 讨论链接起来，保留重要的研究脉络。",
                wf_step4_title: "4. 创作与导出",
                wf_step4_desc: "使用 AI 编辑器草拟报告或笔记。完美导出为 Markdown、PDF 或 Word。",
                tag_web: "网页", tag_docs: "文档", tag_telegram: "Telegram",
                
                priv_badge: "隐私至上",
                priv_title: "你的知识，<br>存在你的 <span class=\"text-brand-400\">设备。</span>",
                priv_desc1: "InsightCAP 采用本地优先设计。你的数据永远不会离开你的电脑，因为我们根本没有服务器。",
                priv_box_title: "完全加密",
                priv_box_desc: "采用 AES-256 加密，密钥安全存储于 Windows 账户下，配合 24 位恢复助记词提供极致保护。",
                
                testi_badge: "用户评价",
                testi_title: "深受知识工作者喜爱",
                testi_1_desc: "“InsightCAP 彻底改变了我组织研究资料的方式。我再也不会弄丢重要的文章，而且 100% 本地存储让我非常安心。”",
                testi_1_name: "John Doe",
                testi_1_role: "软件工程师",
                testi_2_desc: "“Telegram 机器人的整合太棒了。我只需在通勤时将链接转发给机器人，打开电脑时它们就已经建好索引并可以搜索了。”",
                testi_2_name: "Alice Smith",
                testi_2_role: "内容创作者",
                testi_3_desc: "“我受够了订阅制的云端工具绑架我的数据。InsightCAP 的本地优先架构和即时搜索，正是我一直在寻找的工具。”",
                testi_3_name: "Michael R.",
                testi_3_role: "博士研究员",

                faq_title: "常见问题",
                faq_q1: "这真的是完全免费的吗？",
                faq_a1: "是的。社区版（Community Edition）对于个人和商业用途均为免费，没有隐藏费用或使用限制。",
                faq_q2: "有支持 Mac 或 Linux 吗？",
                faq_a2: "目前 InsightCAP 专为 Windows 优化，以充分利用其本地加密协议。我们正在评估支持其他平台的可能性。",
                faq_q3: "这跟 Notion 或 Obsidian 有什么不同？",
                faq_a3: "与 Notion 不同，您的数据是 100% 本地且加密的（无云端依赖）。与 Obsidian 不同，InsightCAP 更专注于从外部来源（网页、YouTube、Telegram）进行自动化抓取与解析。",
                faq_q4: "如果我换电脑了怎么办？",
                faq_a4_1: "InsightCAP 内置导出/导入工具，让您安全迁移知识库。您只需将本地文件夹复制到新电脑，并使用您的 24 位恢复助记词解锁即可。",
                faq_a4_2: "这确保您的数据始终保持离线，且随时在您的完全掌控之中。",
                
                cta_title: "停止遗忘<br>你 <span class=\"text-brand-600\">早已拥有的知识。</span>",
                cta_desc: "InsightCAP 社区版现可免费下载。今天就开始建立你的个人记忆库。",
                cta_microcopy: "100% 本地存储 · 无需注册",
                
                footer_repo: "源码项目",
                footer_releases: "下载页面"
            }
        };

        // Custom Dropdown Logic
        function toggleLangMenu(event) {
            event.stopPropagation();
            document.getElementById('lang-menu').classList.toggle('hidden');
        }

        document.addEventListener('click', (e) => {
            const container = document.getElementById('lang-dropdown-container');
            const menu = document.getElementById('lang-menu');
            if (container && !container.contains(e.target)) {
                menu.classList.add('hidden');
            }
        });

        // i18n Language Setup
        function changeLanguage(lang) {
            document.documentElement.lang = lang;
            localStorage.setItem('preferredLang', lang);
            
            // Update SEO metadata
            const seo = {
                'en': {
                    title: 'InsightCAP Community - Local-First Knowledge System for Windows',
                    description: 'InsightCAP Community is a free local-first knowledge system for Windows. Capture web pages, documents, YouTube videos, clipboard content, and Telegram messages into an encrypted searchable memory.',
                    canonical: 'https://3k1c.github.io/insightcap-community/?lang=en',
                    locale: 'en_US'
                },
                'zh-TW': {
                    title: 'InsightCAP Community - 本地優先知識系統',
                    description: 'InsightCAP Community 是免費的 Windows 本地優先知識系統，可擷取網頁、文件、YouTube、剪貼簿與 Telegram 訊息，建立加密且可搜尋的個人記憶庫。',
                    canonical: 'https://3k1c.github.io/insightcap-community/?lang=zh-TW',
                    locale: 'zh_TW'
                },
                'zh-CN': {
                    title: 'InsightCAP Community - 本地优先知识系统',
                    description: 'InsightCAP Community 是免费的 Windows 本地优先知识系统，可抓取网页、文件、YouTube、剪贴板与 Telegram 消息，建立加密且可搜索的个人记忆库。',
                    canonical: 'https://3k1c.github.io/insightcap-community/?lang=zh-CN',
                    locale: 'zh_CN'
                }
            };
            const currentSeo = seo[lang] || seo.en;
            document.title = currentSeo.title;
            document.getElementById('page-title').innerText = currentSeo.title;
            document.getElementById('meta-description').setAttribute('content', currentSeo.description);
            document.getElementById('canonical-link').setAttribute('href', currentSeo.canonical);
            document.getElementById('og-title').setAttribute('content', currentSeo.title);
            document.getElementById('og-description').setAttribute('content', currentSeo.description);
            document.getElementById('og-url').setAttribute('content', currentSeo.canonical);
            document.getElementById('og-locale').setAttribute('content', currentSeo.locale);
            document.getElementById('twitter-title').setAttribute('content', currentSeo.title);
            document.getElementById('twitter-description').setAttribute('content', currentSeo.description);

            // Update all elements with data-i18n
            document.querySelectorAll('[data-i18n]').forEach(el => {
                const key = el.getAttribute('data-i18n');
                if (translations[lang] && translations[lang][key]) {
                    el.innerHTML = translations[lang][key];
                }
            });
            
            if (window.lucide) lucide.createIcons();
        }

        function selectLang(lang, label) {
            if (!lang || !label) return;
            document.getElementById('current-lang-label').innerText = label;
            changeLanguage(lang);
            document.getElementById('lang-menu').classList.add('hidden');
        }

        // Initialize language
        function initializeLanguage() {
        const savedLang = localStorage.getItem('preferredLang') || 'en';
        const langLabels = { 'en': 'English', 'zh-TW': '繁體中文', 'zh-CN': '简体中文' };
        document.getElementById('current-lang-label').innerText = langLabels[savedLang];
        changeLanguage(savedLang);
        }

        document.addEventListener('DOMContentLoaded', () => {
            const langToggle = document.getElementById('lang-menu-toggle');
            langToggle?.addEventListener('click', toggleLangMenu);
            document.querySelectorAll('[data-lang-option]').forEach((button) => {
                button.addEventListener('click', () => {
                    selectLang(button.getAttribute('data-lang'), button.getAttribute('data-label'));
                });
            });

            initializeLanguage();

            // UI Initialization
            const reveals = document.querySelectorAll('.reveal');
            const observer = new IntersectionObserver(entries => {
                entries.forEach(entry => { if (entry.isIntersecting) entry.target.classList.add('active'); });
            }, { threshold: 0.15 });
            reveals.forEach(r => observer.observe(r));

            const navbar = document.getElementById('navbar');
            window.addEventListener('scroll', () => {
                if (!navbar) return;
                if (window.scrollY > 20) navbar.classList.add('bg-white/80', 'backdrop-blur-xl', 'border-slate-200/50', 'shadow-sm');
                else navbar.classList.remove('bg-white/80', 'backdrop-blur-xl', 'border-slate-200/50', 'shadow-sm');
            });
        });
