const fs = require('fs');
let content = fs.readFileSync('src/pages/SetupPage.tsx', 'utf-8');
content = content.replace("import type { Language } from '../i18n';\r\n\r\nimport type { Language } from '../i18n';", "import type { Language } from '../i18n';");
content = content.replace("import type { Language } from '../i18n';\n\nimport type { Language } from '../i18n';", "import type { Language } from '../i18n';");
fs.writeFileSync('src/pages/SetupPage.tsx', content);
