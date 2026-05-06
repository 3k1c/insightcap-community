const fs = require('fs');
let content = fs.readFileSync('src/pages/SetupPage.tsx', 'utf-8');

// Use regex instead of exact string replacement to handle whitespace differences
content = content.replace(
    /const steps: Step\[\] = \['workspace', 'password', 'recovery'\];\s*const stepIndex = steps.indexOf\(step\);\s*const stepTitles: Record<Step, string> = {[\s\S]*?};/,
    `const [settings, setSettings] = React.useState<any>(null);
    const [setupProvider, setSetupProvider] = React.useState('openai');
    const [setupModel, setSetupModel] = React.useState('gpt-4o-mini');
    const [setupApiKey, setSetupApiKey] = React.useState('');
    const [setupWebSearchApi, setSetupWebSearchApi] = React.useState('');
    const [bilibiliLoggingIn, setBilibiliLoggingIn] = React.useState(false);

    React.useEffect(() => {
        invoke('get_settings').then(s => setSettings(s)).catch(console.error);
    }, []);

    const steps: Step[] = ['workspace', 'password', 'ai_provider', 'hotkeys', 'services_check', 'recovery'];
    const stepIndex = steps.indexOf(step);

    const stepTitles: Record<Step, string> = {
        workspace: t('auth.setup.step1_title'),
        password: t('auth.setup.step2_title'),
        ai_provider: t('auth.setup.step3_title'),
        hotkeys: t('auth.setup.step4_title'),
        services_check: t('auth.setup.step5_title'),
        recovery: t('auth.setup.step6_title'),
    };`
);

content = content.replace(
    /setRecoveryPhrase\(mnemonic\);\s*setStep\('recovery'\);\s*\} catch \(e\) \{/,
    `setRecoveryPhrase(mnemonic);
            setStep('ai_provider');
        } catch (e) {`
);

content = content.replace(
    /async function handleCopyRecovery\(\) \{[\s\S]*?\}\s*catch \{ \}\s*\}/,
    `async function handleCopyRecovery() {
        try {
            await navigator.clipboard.writeText(recoveryPhrase);
            setCopied(true);
            setTimeout(() => setCopied(false), 2000);
        } catch { }
    }

    async function handleAiProviderNext() {
        if (!settings) { setStep('hotkeys'); return; }
        const newSettings = { ...settings };
        newSettings.aiModels.chatLlm = { provider: setupProvider, model: setupModel };
        const profileIdx = newSettings.aiModels.providerProfiles.findIndex((p: any) => p.provider === setupProvider);
        if (profileIdx >= 0) {
            newSettings.aiModels.providerProfiles[profileIdx].apiKey = setupApiKey;
        } else {
            newSettings.aiModels.providerProfiles.push({
                id: crypto.randomUUID(),
                name: 'Setup ' + setupProvider,
                provider: setupProvider,
                apiKey: setupApiKey,
                baseUrl: setupProvider === 'ollama' ? 'http://localhost:11434' : ''
            });
        }
        await invoke('save_settings', { settings: newSettings });
        setSettings(newSettings);
        setStep('hotkeys');
    }

    async function handleHotkeysNext() {
        if (settings) {
            await invoke('save_settings', { settings });
        }
        setStep('services_check');
    }

    async function handleBilibiliLogin() {
        setBilibiliLoggingIn(true);
        try {
            const sessdata = await invoke('open_bilibili_login');
            const newSettings = { ...settings, bilibiliSessdata: sessdata };
            await invoke('save_settings', { settings: newSettings });
            setSettings(newSettings);
        } catch (e) {
            console.error(e);
        } finally {
            setBilibiliLoggingIn(false);
        }
    }

    async function handleServicesNext() {
        if (settings) {
            const newSettings = { ...settings };
            if (!newSettings.webSearch) newSettings.webSearch = { enabled: false, provider: 'duckduckgo', apiKey: '' };
            if (setupWebSearchApi) {
                newSettings.webSearch.apiKey = setupWebSearchApi;
                newSettings.webSearch.enabled = true;
            }
            await invoke('save_settings', { settings: newSettings });
            setSettings(newSettings);
        }
        setStep('recovery');
    }`
);

content = content.replace(
    /<Button className="flex-1" loading=\{loading\} onClick=\{handleSetupSubmit\}>\s*\{t\('common\.next'\)\} <ChevronRight className="h-4 w-4" \/>\s*<\/Button>\s*<\/div>\s*<\/div>\s*\)\}/,
    `<Button className="flex-1" loading={loading} onClick={handleSetupSubmit}>
                                {t('common.next')} <ChevronRight className="h-4 w-4" />
                            </Button>
                        </div>
                    </div>
                )}

                {step === 'ai_provider' && (
                    <div className="space-y-4">
                        <p className="text-fs-sm text-text-secondary mb-2">{t('auth.setup.ai_provider_desc')}</p>
                        <div>
                            <label className="text-fs-xs text-text-secondary block mb-1">{t('settings.provider_type')}</label>
                            <select
                                value={setupProvider}
                                onChange={e => setSetupProvider(e.target.value)}
                                className="w-full bg-surface-base border border-stroke-divider rounded-lg px-3 py-1.5 text-fs-sm text-text-primary focus:outline-none"
                            >
                                <option value="openai">OpenAI</option>
                                <option value="anthropic">Anthropic</option>
                                <option value="google">Google</option>
                                <option value="ollama">Ollama (Local)</option>
                            </select>
                        </div>
                        <div>
                            <label className="text-fs-xs text-text-secondary block mb-1">{t('settings.model_name')}</label>
                            <input
                                type="text"
                                value={setupModel}
                                onChange={e => setSetupModel(e.target.value)}
                                placeholder="gpt-4o-mini / claude-3-5-sonnet..."
                                className="w-full bg-surface-base border border-stroke-divider rounded-lg px-3 py-1.5 text-fs-sm text-text-primary focus:outline-none"
                            />
                        </div>
                        {setupProvider !== 'ollama' && (
                            <div>
                                <label className="text-fs-xs text-text-secondary block mb-1">{t('settings.provider_apikey')}</label>
                                <input
                                    type="password"
                                    value={setupApiKey}
                                    onChange={e => setSetupApiKey(e.target.value)}
                                    placeholder="sk-..."
                                    className="w-full bg-surface-base border border-stroke-divider rounded-lg px-3 py-1.5 text-fs-sm text-text-primary focus:outline-none"
                                />
                            </div>
                        )}
                        <Button className="w-full" onClick={handleAiProviderNext}>
                            {t('common.next')} <ChevronRight className="h-4 w-4" />
                        </Button>
                    </div>
                )}

                {step === 'hotkeys' && (
                    <div className="space-y-4">
                        <p className="text-fs-sm text-text-secondary mb-2">{t('auth.setup.hotkeys_desc')}</p>
                        {settings && (
                            <>
                                <div>
                                    <label className="text-fs-xs text-text-secondary block mb-1">{t('settings.capture_clipboard')}</label>
                                    <HotkeyInput
                                        value={settings.hotkeys?.captureClipboard || ''}
                                        onChange={v => setSettings({ ...settings, hotkeys: { ...settings.hotkeys, captureClipboard: v } })}
                                    />
                                </div>
                                <div className="mt-3">
                                    <label className="text-fs-xs text-text-secondary block mb-1">{t('settings.quick_input')}</label>
                                    <HotkeyInput
                                        value={settings.hotkeys?.quickInput || ''}
                                        onChange={v => setSettings({ ...settings, hotkeys: { ...settings.hotkeys, quickInput: v } })}
                                    />
                                </div>
                            </>
                        )}
                        <Button className="w-full mt-4" onClick={handleHotkeysNext}>
                            {t('common.next')} <ChevronRight className="h-4 w-4" />
                        </Button>
                    </div>
                )}

                {step === 'services_check' && (
                    <div className="space-y-4">
                        <p className="text-fs-sm text-text-secondary mb-2">{t('auth.setup.services_check_desc')}</p>

                        <div className="bg-surface-base border border-stroke-divider rounded-lg p-4 space-y-4">
                            <div>
                                <h4 className="text-fs-sm font-medium text-text-primary flex items-center gap-2">
                                    🌐 {t('settings.web_search_title')}
                                    {settings?.webSearch?.apiKey ? (
                                        <span className="text-fs-xs text-green-500 bg-green-500/10 px-1.5 py-0.5 rounded-full">OK</span>
                                    ) : null}
                                </h4>
                                {!settings?.webSearch?.apiKey && (
                                    <>
                                        <p className="text-fs-xs text-color-warning mt-1">{t('auth.setup.web_search_warning')}</p>
                                        <input
                                            type="text"
                                            value={setupWebSearchApi}
                                            onChange={e => setSetupWebSearchApi(e.target.value)}
                                            placeholder="Tavily API Key..."
                                            className="w-full bg-surface-base border border-stroke-divider rounded-lg px-3 py-1.5 text-fs-sm text-text-primary mt-2 focus:outline-none"
                                        />
                                    </>
                                )}
                            </div>

                            <div className="h-px bg-stroke-divider"></div>

                            <div>
                                <h4 className="text-fs-sm font-medium text-text-primary flex items-center gap-2">
                                    📺 {t('settings.bilibili_sessdata')}
                                    {settings?.bilibiliSessdata ? (
                                        <span className="text-fs-xs text-green-500 bg-green-500/10 px-1.5 py-0.5 rounded-full">OK</span>
                                    ) : null}
                                </h4>
                                {!settings?.bilibiliSessdata && (
                                    <>
                                        <p className="text-fs-xs text-color-warning mt-1">{t('auth.setup.bilibili_warning')}</p>
                                        <Button
                                            variant="secondary"
                                            size="sm"
                                            className="mt-2 w-full"
                                            loading={bilibiliLoggingIn}
                                            onClick={handleBilibiliLogin}
                                        >
                                            {t('settings.login_bilibili')}
                                        </Button>
                                    </>
                                )}
                            </div>
                        </div>

                        <Button className="w-full mt-4" onClick={handleServicesNext}>
                            {t('common.next')} <ChevronRight className="h-4 w-4" />
                        </Button>
                    </div>
                )}`
);

fs.writeFileSync('src/pages/SetupPage.tsx', content);
console.log('Patched');
