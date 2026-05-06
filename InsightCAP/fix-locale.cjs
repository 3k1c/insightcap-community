const fs = require('fs');

// Fix en.json
let en = fs.readFileSync('src/i18n/locales/en.json', 'utf8');
let enObj = JSON.parse(en);
const enSetup = enObj.auth.setup;
enSetup.step1_title = "Welcome to InsightCAP";
enSetup.step2_title = "Choose AI Provider";
enSetup.step3_title = "Choose Workspace";
enSetup.step4_title = "Set Password";
enSetup.step5_title = "Save Recovery Phrase";
delete enSetup.step6_title;
enSetup.welcome_desc = "InsightCAP is an AI-powered knowledge assistant that helps you capture, organize, and query knowledge.";
enSetup.welcome_hotkey_title = "Global Hotkeys";
enSetup.welcome_hotkey_hint = "You can customize hotkeys in Settings after setup is complete.";
delete enSetup.services_check_desc;
delete enSetup.web_search_warning;
delete enSetup.bilibili_warning;
delete enSetup.go_to_settings;
delete enSetup.provider_empty;
fs.writeFileSync('src/i18n/locales/en.json', JSON.stringify(enObj, null, 4));
JSON.parse(fs.readFileSync('src/i18n/locales/en.json', 'utf8'));
console.log('en.json OK');

// Fix zh-CN.json
let cn = fs.readFileSync('src/i18n/locales/zh-CN.json', 'utf8');
let cnObj = JSON.parse(cn);
const cnSetup = cnObj.auth.setup;
cnSetup.step1_title = "欢迎使用 InsightCAP";
cnSetup.step2_title = "选择 AI 服务商";
cnSetup.step3_title = "选择工作区";
cnSetup.step4_title = "设置密码";
cnSetup.step5_title = "保存恢复码";
delete cnSetup.step6_title;
cnSetup.welcome_desc = "InsightCAP 是一款 AI 知识库辅助工具，帮助您捕捉、整理与查询知识。";
cnSetup.welcome_hotkey_title = "全局快捷键";
cnSetup.welcome_hotkey_hint = "安装完成后可在设置页面自定义更改。";
cnSetup.ai_provider_desc = "选择默认的 AI 服务商与聊天模型，日后可随时在设置中更改。";
cnSetup.hotkeys_desc = "以下是 InsightCAP 的默认全局快捷键，安装完成后可在「设置 > 快捷键」中自定义更改。";
cnSetup.hotkeys_hint = "若需更改快捷键，请完成设置后前往主程序的设置页面修改。";
delete cnSetup.services_check_desc;
delete cnSetup.web_search_warning;
delete cnSetup.bilibili_warning;
delete cnSetup.go_to_settings;
delete cnSetup.provider_empty;
fs.writeFileSync('src/i18n/locales/zh-CN.json', JSON.stringify(cnObj, null, 4));
JSON.parse(fs.readFileSync('src/i18n/locales/zh-CN.json', 'utf8'));
console.log('zh-CN.json OK');
