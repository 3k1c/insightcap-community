/**
 * InsightCAP Mobile — Settings Screen
 *
 * 全域：桌面連線（URL + Token）— 所有模式均用於知識庫 / 對話歷史
 * 推理引擎選擇：
 *   A — 本機 Gemma 4（LiteRT-LM）
 *   B — 雲端 API（Claude / OpenAI / Gemini）
 *   C — 桌面端 AI 代理（透過桌面推理）
 */

import React, { useEffect, useState, useCallback } from 'react';
import {
  View,
  Text,
  TextInput,
  TouchableOpacity,
  ScrollView,
  Alert,
  StyleSheet,
  SafeAreaView,
  ActivityIndicator,
  Modal,
  Linking,
} from 'react-native';
import { CheckCircle2, Download, FolderOpen, RefreshCw } from 'lucide-react-native';
import DocumentPicker from 'react-native-document-picker';
import RNFS from 'react-native-fs';
import { Camera, useCameraDevice, useCodeScanner } from 'react-native-vision-camera';
import type { InferenceMode } from '../services/inference';
import {
  loadSettings,
  saveSettings,
  type MobileSettings,
  type CloudSettings,
} from '../services/settings-store';
import { loadGemmaModel, unloadModel } from '../services/inference/local-litert-lm';
import { checkDesktopHealth } from '../services/desktop-api';

// ─── Constants ──────────────────────────────────────────────────────────────

type CloudProvider = CloudSettings['provider'];

const MODE_LABELS: Record<InferenceMode, { label: string; sub: string }> = {
  local:   { label: 'Gemma 4 本機',   sub: 'LiteRT-LM · 完全離線' },
  cloud:   { label: '雲端 API',       sub: 'Claude / OpenAI / Gemini' },
  desktop: { label: '桌面端 AI 代理', sub: '透過桌面電腦推理' },
};

const CLOUD_PROVIDERS: { value: CloudProvider; label: string }[] = [
  { value: 'claude', label: 'Anthropic Claude' },
  { value: 'openai', label: 'OpenAI' },
  { value: 'gemini', label: 'Google Gemini' },
];

const DEFAULT_MODELS: Record<CloudProvider, string> = {
  claude: 'claude-sonnet-4-6',
  openai: 'gpt-4o',
  gemini: 'gemini-2.5-flash',
};

// ─── SettingsScreen ─────────────────────────────────────────────────────────

export default function SettingsScreen() {
  const [settings, setSettings] = useState<MobileSettings | null>(null);
  const [modelLoading, setModelLoading] = useState(false);
  const [modelLoaded, setModelLoaded] = useState(false);
  const [foundTaskFiles, setFoundTaskFiles] = useState<string[]>([]);
  const [scanningFiles, setScanningFiles] = useState(false);
  const [downloadProgress, setDownloadProgress] = useState<Record<string, number>>({});
  const [desktopOnline, setDesktopOnline] = useState<boolean | null>(null);
  const [saving, setSaving] = useState(false);
  const [scanning, setScanning] = useState(false);

  useEffect(() => {
    loadSettings().then(setSettings);
  }, []);

  // ─── Update Helpers ───────────────────────────────────────────────────
  // 全部定義在 conditional return 之前，符合 hooks 規則

  function update<K extends keyof MobileSettings>(
    key: K,
    value: MobileSettings[K],
  ) {
    setSettings(prev => (prev ? { ...prev, [key]: value } : prev));
  }

  function updateLocal(patch: Partial<MobileSettings['local']>) {
    setSettings(prev =>
      prev ? { ...prev, local: { ...prev.local, ...patch } } : prev,
    );
  }

  function updateCloud(patch: Partial<MobileSettings['cloud']>) {
    setSettings(prev =>
      prev ? { ...prev, cloud: { ...prev.cloud, ...patch } } : prev,
    );
  }

  function updateDesktop(patch: Partial<MobileSettings['desktop']>) {
    setSettings(prev =>
      prev ? { ...prev, desktop: { ...prev.desktop, ...patch } } : prev,
    );
  }

  // ─── QR Code ─────────────────────────────────────────────────────────
  // useCallback 必須在 conditional return 之前宣告（hooks 規則）

  const handleQRScanned = useCallback(
    (url: string, token: string) => {
      setSettings(prev =>
        prev
          ? { ...prev, desktop: { ...prev.desktop, url, token } }
          : prev,
      );
      setScanning(false);
      Alert.alert('配對成功', `已連線至 ${url}`);
    },
    [],
  );

  if (!settings) return null;

  async function handleSave() {
    if (!settings) return;
    setSaving(true);
    await saveSettings(settings);
    setSaving(false);
    Alert.alert('已儲存');
  }

  // ─── Local Model ──────────────────────────────────────────────────────

  async function handleDownloadModel(modelKey: string, filename: string, hfPath: string) {
    const token = settings?.local.hfToken.trim();
    if (!token) {
      Alert.alert('需要 HuggingFace Token', '請先填入 HuggingFace Access Token。');
      return;
    }
    const destPath = `${RNFS.DownloadDirectoryPath}/${filename}`;
    const url = `https://huggingface.co/${hfPath}/resolve/main/${filename}`;
    setDownloadProgress(p => ({ ...p, [modelKey]: 0 }));
    try {
      const { promise } = RNFS.downloadFile({
        fromUrl: url,
        toFile: destPath,
        headers: { Authorization: `Bearer ${token}` },
        progressDivider: 5,
        begin: () => {},
        progress: ({ bytesWritten, contentLength }) => {
          const pct = contentLength > 0 ? (bytesWritten / contentLength) * 100 : 0;
          setDownloadProgress(p => ({ ...p, [modelKey]: Math.floor(pct) }));
        },
      });
      await promise;
      updateLocal({ modelPath: destPath });
      setDownloadProgress(p => {
        const next = { ...p };
        delete next[modelKey];
        return next;
      });
      Alert.alert('下載完成', `已儲存至 Downloads/${filename}`);
    } catch {
      setDownloadProgress(p => {
        const next = { ...p };
        delete next[modelKey];
        return next;
      });
      Alert.alert('下載失敗', '請確認 Token 正確且已在 HuggingFace 接受 Gemma 授權協議。');
    }
  }

  async function handleScanDownloads() {
    setScanningFiles(true);
    try {
      const dir = RNFS.DownloadDirectoryPath;
      const items = await RNFS.readDir(dir);
      const tasks = items
        .filter(f => f.isFile() && f.name.endsWith('.task'))
        .map(f => f.path);
      setFoundTaskFiles(tasks);
      if (tasks.length === 0) {
        Alert.alert('未找到模型', '請先下載 .task 格式的 Gemma 4 模型至 Downloads 資料夾。');
      }
    } catch {
      Alert.alert('掃描失敗', '無法讀取 Downloads 資料夾');
    } finally {
      setScanningFiles(false);
    }
  }

  async function handleLoadModel() {
    if (!settings) return;
    const path = settings.local.modelPath.trim();
    if (!path) {
      Alert.alert('請先輸入模型路徑');
      return;
    }
    setModelLoading(true);
    try {
      await loadGemmaModel(path, settings.local.maxTokens);
      setModelLoaded(true);
      Alert.alert('模型載入成功');
    } catch (e: unknown) {
      Alert.alert('載入失敗', (e as Error)?.message ?? '請確認路徑正確');
    } finally {
      setModelLoading(false);
    }
  }

  async function handleUnloadModel() {
    await unloadModel();
    setModelLoaded(false);
  }

  // ─── Desktop Test ─────────────────────────────────────────────────────

  async function handleTestDesktop() {
    setDesktopOnline(null);
    const ok = await checkDesktopHealth();
    setDesktopOnline(ok);
  }

  // ─── Render ───────────────────────────────────────────────────────────

  return (
    <SafeAreaView style={s.root}>
      <ScrollView
        contentContainerStyle={s.scroll}
        keyboardShouldPersistTaps="handled"
      >
        {/* ① 桌面連線 — 全域前置，所有模式均使用 */}
        <SectionTitle title="桌面連線" />
        <Text style={s.globalNote}>
          知識庫、對話歷史均來自桌面端，所有推理模式皆需設定。
        </Text>
        <View style={s.card}>
          <View style={s.tailscaleBadge}>
            <Text style={s.tailscaleBadgeText}>✦ 支援 Tailscale 跨網路連線</Text>
            <Text style={s.tailscaleHint}>
              手機與電腦均安裝 Tailscale 後，URL 填{' '}
              <Text style={{ color: '#e5e5ea' }}>http://100.x.x.x:3030</Text>
            </Text>
          </View>

          <PrimaryButton
            label="掃描桌面 QR Code"
            onPress={async () => {
              const status = await Camera.getCameraPermissionStatus();
              if (status === 'granted') {
                setScanning(true);
              } else {
                const result = await Camera.requestCameraPermission();
                if (result === 'granted') {
                  setScanning(true);
                } else {
                  Alert.alert('需要相機權限', '請至系統設定 → InsightCAP → 權限，開啟相機存取。');
                }
              }
            }}
          />

          <Label text="桌面 URL（同網路或 Tailscale）" />
          <TextInput
            style={s.input}
            value={settings.desktop.url}
            onChangeText={v => updateDesktop({ url: v })}
            placeholder="http://192.168.1.x:3030 或 http://100.x.x.x:3030"
            placeholderTextColor="#48484a"
            autoCapitalize="none"
            keyboardType="url"
          />

          <Label text="API Token" />
          <TextInput
            style={s.input}
            value={settings.desktop.token}
            onChangeText={v => updateDesktop({ token: v })}
            placeholder="32 位 hex token"
            placeholderTextColor="#48484a"
            secureTextEntry
            autoCapitalize="none"
          />

          <View style={s.btnRow}>
            <OutlineButton
              label="測試連線"
              onPress={handleTestDesktop}
              color="#0a84ff"
              style={{ flex: 1 }}
            />
            {desktopOnline !== null && (
              <View style={s.statusRow}>
                {desktopOnline ? (
                  <>
                    <CheckCircle2 color="#30d158" size={16} />
                    <Text style={[s.statusText, { color: '#30d158' }]}>連線成功</Text>
                  </>
                ) : (
                  <Text style={[s.statusText, { color: '#ff453a' }]}>無法連線</Text>
                )}
              </View>
            )}
          </View>
        </View>

        {/* ② 推理引擎選擇 */}
        <SectionTitle title="推理引擎" />
        <View style={s.card}>
          {(Object.keys(MODE_LABELS) as InferenceMode[]).map(m => {
            const { label, sub } = MODE_LABELS[m];
            const selected = settings.inferenceMode === m;
            return (
              <TouchableOpacity
                key={m}
                style={s.radioRow}
                onPress={() => update('inferenceMode', m)}
              >
                <View style={[s.radioCircle, selected && s.radioCircleSelected]} />
                <View style={{ flex: 1 }}>
                  <Text style={[s.radioLabel, selected && { color: '#fff' }]}>{label}</Text>
                  <Text style={s.radioSub}>{sub}</Text>
                </View>
              </TouchableOpacity>
            );
          })}
        </View>

        {/* ③ 僅顯示當前選中的推理引擎設定 */}

        {/* Mode A — Gemma 4 本機 */}
        {settings.inferenceMode === 'local' && (
          <>
            <SectionTitle title="Gemma 4 本機設定" />
            <View style={s.card}>
              <View style={s.downloadSection}>
                <Text style={s.downloadTitle}>從 HuggingFace 下載模型</Text>
                <Label text="HuggingFace Access Token" />
                <View style={s.hfTokenRow}>
                  <TextInput
                    style={[s.input, { flex: 1 }]}
                    value={settings.local.hfToken}
                    onChangeText={v => updateLocal({ hfToken: v })}
                    placeholder="hf_..."
                    placeholderTextColor="#48484a"
                    secureTextEntry
                    autoCapitalize="none"
                  />
                  <TouchableOpacity
                    style={s.hfTokenLink}
                    onPress={() => Linking.openURL('https://huggingface.co/settings/tokens')}
                  >
                    <Text style={s.hfTokenLinkText}>取得</Text>
                  </TouchableOpacity>
                </View>
                <Text style={s.downloadNote}>需先在 HuggingFace 接受 Gemma 授權協議</Text>
                {([
                  { key: '1b', label: 'Gemma 4 1B', sub: '≈ 600 MB · int8', filename: 'gemma-4-1b-it-int8.task', hfPath: 'litert-community/Gemma-4-1B-IT' },
                  { key: '4b', label: 'Gemma 4 4B', sub: '≈ 2.5 GB · int8', filename: 'gemma-4-4b-it-int8.task', hfPath: 'litert-community/Gemma-4-4B-IT' },
                ] as const).map(m => {
                  const pct = downloadProgress[m.key];
                  const isDownloading = pct !== undefined;
                  return (
                    <TouchableOpacity
                      key={m.key}
                      style={s.downloadBtn}
                      onPress={() => handleDownloadModel(m.key, m.filename, m.hfPath)}
                      disabled={isDownloading}
                    >
                      {isDownloading
                        ? <ActivityIndicator size="small" color="#0a84ff" />
                        : <Download color="#0a84ff" size={14} />}
                      <View style={{ flex: 1 }}>
                        <Text style={s.downloadBtnLabel}>{m.label}</Text>
                        <Text style={s.downloadBtnSub}>{isDownloading ? `下載中 ${pct}%` : m.sub}</Text>
                        {isDownloading && (
                          <View style={s.progressBar}>
                            <View style={[s.progressFill, { width: `${pct}%` }]} />
                          </View>
                        )}
                      </View>
                    </TouchableOpacity>
                  );
                })}
              </View>

              <Label text="模型檔案（.task）" />
              <View style={s.fileRow}>
                <TouchableOpacity
                  style={[s.filePickerBtn, { flex: 1 }]}
                  onPress={async () => {
                    try {
                      const result = await DocumentPicker.pickSingle({ type: [DocumentPicker.types.allFiles] });
                      if (result.uri) { updateLocal({ modelPath: result.uri }); setFoundTaskFiles([]); }
                    } catch (e) {
                      if (!DocumentPicker.isCancel(e)) Alert.alert('無法開啟檔案選擇器');
                    }
                  }}
                >
                  <FolderOpen color="#0a84ff" size={18} />
                  <Text style={s.filePickerText} numberOfLines={1}>
                    {settings.local.modelPath ? settings.local.modelPath.split('/').pop() ?? settings.local.modelPath : '選擇 .task 檔案'}
                  </Text>
                </TouchableOpacity>
                <TouchableOpacity style={s.scanBtn} onPress={handleScanDownloads} disabled={scanningFiles}>
                  {scanningFiles ? <ActivityIndicator size="small" color="#0a84ff" /> : <RefreshCw color="#0a84ff" size={18} />}
                </TouchableOpacity>
              </View>

              {foundTaskFiles.length > 0 && (
                <View style={s.foundFilesBox}>
                  <Text style={s.foundFilesTitle}>Downloads 中找到的模型：</Text>
                  {foundTaskFiles.map(path => (
                    <TouchableOpacity key={path} style={s.foundFileRow} onPress={() => { updateLocal({ modelPath: path }); setFoundTaskFiles([]); }}>
                      <Text style={s.foundFileName} numberOfLines={1}>{path.split('/').pop()}</Text>
                      <Text style={s.foundFileSelect}>選用</Text>
                    </TouchableOpacity>
                  ))}
                </View>
              )}

              <Label text="模型版本" />
              <View style={s.segmented}>
                {(['gemma4-1b', 'gemma4-4b'] as const).map(v => (
                  <TouchableOpacity key={v} style={[s.segItem, settings.local.modelVariant === v && s.segItemActive]} onPress={() => updateLocal({ modelVariant: v })}>
                    <Text style={[s.segText, settings.local.modelVariant === v && s.segTextActive]}>
                      {v === 'gemma4-1b' ? 'Gemma 4 1B (輕量)' : 'Gemma 4 4B (多模態)'}
                    </Text>
                  </TouchableOpacity>
                ))}
              </View>

              <Label text={`最大輸出 Token：${settings.local.maxTokens}`} />
              <View style={s.tokenRow}>
                {[512, 1024, 2048, 4096].map(n => (
                  <TouchableOpacity key={n} style={[s.tokenChip, settings.local.maxTokens === n && s.tokenChipActive]} onPress={() => updateLocal({ maxTokens: n })}>
                    <Text style={[s.tokenText, settings.local.maxTokens === n && s.tokenTextActive]}>{n}</Text>
                  </TouchableOpacity>
                ))}
              </View>

              <View style={s.btnRow}>
                {modelLoaded
                  ? <OutlineButton label="卸載模型" onPress={handleUnloadModel} color="#ff453a" style={{ flex: 1 }} />
                  : <PrimaryButton label={modelLoading ? '載入中...' : '載入 Gemma 4'} onPress={handleLoadModel} loading={modelLoading} style={{ flex: 1 }} />}
                {modelLoaded && (
                  <View style={s.statusRow}>
                    <CheckCircle2 color="#30d158" size={16} />
                    <Text style={[s.statusText, { color: '#30d158' }]}>模型已就緒</Text>
                  </View>
                )}
              </View>
            </View>
          </>
        )}

        {/* Mode B — 雲端 API */}
        {settings.inferenceMode === 'cloud' && (
          <>
            <SectionTitle title="雲端 API 設定" />
            <View style={s.card}>
              <Label text="供應商" />
              <View style={s.segmented}>
                {CLOUD_PROVIDERS.map(({ value, label }) => (
                  <TouchableOpacity
                    key={value}
                    style={[s.segItem, settings.cloud.provider === value && s.segItemActive]}
                    onPress={() => updateCloud({ provider: value, model: DEFAULT_MODELS[value] })}
                  >
                    <Text style={[s.segText, settings.cloud.provider === value && s.segTextActive]}>{label}</Text>
                  </TouchableOpacity>
                ))}
              </View>

              <Label text="API Key" />
              <TextInput
                style={s.input}
                value={settings.cloud.apiKey}
                onChangeText={v => updateCloud({ apiKey: v })}
                placeholder="sk-ant-... / sk-... / AIza..."
                placeholderTextColor="#48484a"
                secureTextEntry
                autoCapitalize="none"
              />

              <Label text="Model" />
              <TextInput
                style={s.input}
                value={settings.cloud.model}
                onChangeText={v => updateCloud({ model: v })}
                placeholder={DEFAULT_MODELS[settings.cloud.provider]}
                placeholderTextColor="#48484a"
                autoCapitalize="none"
              />
            </View>
          </>
        )}

        {/* Mode C — 桌面端 AI 代理 */}
        {settings.inferenceMode === 'desktop' && (
          <>
            <SectionTitle title="桌面端 AI 代理" />
            <View style={[s.card, { gap: 6 }]}>
              <Text style={s.desktopTip}>
                推理由桌面電腦執行，手機僅作為介面。連線資訊已在上方「桌面連線」設定。
              </Text>
              {settings.desktop.url ? (
                <View style={s.statusRow}>
                  <CheckCircle2 color={desktopOnline === false ? '#ff453a' : '#30d158'} size={14} />
                  <Text style={[s.statusText, { color: desktopOnline === false ? '#ff453a' : '#8e8e93' }]}>
                    {settings.desktop.url}
                  </Text>
                </View>
              ) : (
                <Text style={[s.hint, { color: '#ff9f0a' }]}>⚠ 尚未設定桌面 URL，請至上方「桌面連線」填入。</Text>
              )}
            </View>
          </>
        )}

        {/* QR Scanner Modal */}
        <Modal visible={scanning} animationType="slide" onRequestClose={() => setScanning(false)}>
          {scanning && <QRScanner onScanned={handleQRScanned} onClose={() => setScanning(false)} />}
        </Modal>

        {/* Save Button */}
        <PrimaryButton
          label={saving ? '儲存中...' : '儲存設定'}
          onPress={handleSave}
          loading={saving}
          style={{ marginTop: 16, marginBottom: 32 }}
        />
      </ScrollView>
    </SafeAreaView>
  );
}

// ─── QRScanner Component (lazy mount — camera hooks only run when visible) ──

function QRScanner({
  onScanned,
  onClose,
}: {
  onScanned: (url: string, token: string) => void;
  onClose: () => void;
}) {
  const [permissionDenied, setPermissionDenied] = useState(false);
  const backCamera = useCameraDevice('back');

  useEffect(() => {
    Camera.getCameraPermissionStatus().then(status => {
      if (status !== 'granted') setPermissionDenied(true);
    });
  }, []);

  const codeScanner = useCodeScanner({
    codeTypes: ['qr'],
    onCodeScanned: (codes: { value?: string }[]) => {
      const raw = codes[0]?.value;
      if (!raw) return;
      try {
        const parsed = JSON.parse(raw) as { url: string; token: string };
        if (parsed.url && parsed.token) {
          onScanned(parsed.url, parsed.token);
        }
      } catch {
        // 非 InsightCAP QR Code
      }
    },
  });

  return (
    <SafeAreaView style={s.scannerRoot}>
      <View style={s.scannerHeader}>
        <Text style={s.scannerTitle}>掃描桌面 QR Code</Text>
        <TouchableOpacity onPress={onClose}>
          <Text style={s.scannerClose}>取消</Text>
        </TouchableOpacity>
      </View>
      {permissionDenied ? (
        <View style={[s.camera, { justifyContent: 'center', alignItems: 'center', padding: 32 }]}>
          <Text style={{ color: '#ff453a', fontSize: 16, fontWeight: '600', marginBottom: 12 }}>
            相機權限已被拒絕
          </Text>
          <Text style={{ color: '#636366', fontSize: 14, textAlign: 'center' }}>
            請至系統設定 → 應用程式 → InsightCAP → 權限，開啟相機存取。
          </Text>
        </View>
      ) : backCamera ? (
        <Camera
          style={s.camera}
          device={backCamera}
          isActive
          codeScanner={codeScanner}
        />
      ) : (
        <View style={[s.camera, { justifyContent: 'center', alignItems: 'center' }]}>
          <Text style={{ color: '#636366' }}>找不到後置相機</Text>
        </View>
      )}
      <Text style={s.scannerHint}>對準桌面端「其他設定」頁的 QR Code</Text>
    </SafeAreaView>
  );
}

// ─── Small Components ───────────────────────────────────────────────────────

function SectionTitle({ title }: { title: string }) {
  return <Text style={s.sectionTitle}>{title}</Text>;
}

function Label({ text }: { text: string }) {
  return <Text style={s.label}>{text}</Text>;
}

function PrimaryButton({
  label,
  onPress,
  loading,
  style,
}: {
  label: string;
  onPress: () => void;
  loading?: boolean;
  style?: object;
}) {
  return (
    <TouchableOpacity
      style={[s.primaryBtn, style]}
      onPress={onPress}
      disabled={loading}
    >
      {loading ? (
        <ActivityIndicator color="#fff" />
      ) : (
        <Text style={s.primaryBtnText}>{label}</Text>
      )}
    </TouchableOpacity>
  );
}

function OutlineButton({
  label,
  onPress,
  color,
  style,
}: {
  label: string;
  onPress: () => void;
  color: string;
  style?: object;
}) {
  return (
    <TouchableOpacity
      style={[s.outlineBtn, { borderColor: color }, style]}
      onPress={onPress}
    >
      <Text style={[s.outlineBtnText, { color }]}>{label}</Text>
    </TouchableOpacity>
  );
}

// ─── Styles ─────────────────────────────────────────────────────────────────

const s = StyleSheet.create({
  root: { flex: 1, backgroundColor: '#0f0f11' },
  scroll: { padding: 16, paddingTop: 4 },
  sectionTitle: {
    fontSize: 11,
    fontWeight: '600',
    color: '#636366',
    textTransform: 'uppercase',
    letterSpacing: 0.5,
    marginTop: 20,
    marginBottom: 8,
  },
  globalNote: {
    fontSize: 12,
    color: '#636366',
    marginBottom: 8,
    lineHeight: 18,
  },
  card: {
    backgroundColor: '#1c1c1e',
    borderRadius: 12,
    padding: 16,
    gap: 10,
  },
  radioRow: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 12,
    paddingVertical: 4,
  },
  radioCircle: {
    width: 20,
    height: 20,
    borderRadius: 10,
    borderWidth: 2,
    borderColor: '#48484a',
  },
  radioCircleSelected: {
    borderColor: '#0a84ff',
    backgroundColor: '#0a84ff',
  },
  radioLabel: { fontSize: 15, color: '#e5e5ea' },
  radioSub: { fontSize: 12, color: '#636366', marginTop: 2 },
  label: { fontSize: 12, color: '#636366', marginBottom: -4 },
  input: {
    backgroundColor: '#2c2c2e',
    borderRadius: 8,
    paddingHorizontal: 12,
    paddingVertical: 10,
    color: '#fff',
    fontSize: 15,
  },
  segmented: { gap: 6 },
  segItem: {
    backgroundColor: '#2c2c2e',
    borderRadius: 8,
    paddingVertical: 10,
    paddingHorizontal: 12,
  },
  segItemActive: { backgroundColor: '#0a84ff' },
  segText: { fontSize: 14, color: '#8e8e93' },
  segTextActive: { color: '#fff', fontWeight: '600' },
  tokenRow: { flexDirection: 'row', gap: 8 },
  tokenChip: {
    flex: 1,
    backgroundColor: '#2c2c2e',
    borderRadius: 8,
    paddingVertical: 8,
    alignItems: 'center',
  },
  tokenChipActive: { backgroundColor: '#0a84ff' },
  tokenText: { fontSize: 13, color: '#8e8e93' },
  tokenTextActive: { color: '#fff', fontWeight: '600' },
  btnRow: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 12,
    marginTop: 4,
  },
  statusRow: { flexDirection: 'row', alignItems: 'center', gap: 6 },
  statusText: { fontSize: 13, fontWeight: '500' },
  downloadSection: {
    backgroundColor: '#0a1628',
    borderRadius: 8,
    borderWidth: 1,
    borderColor: '#0a84ff30',
    padding: 12,
    gap: 8,
  },
  downloadTitle: { fontSize: 13, fontWeight: '600', color: '#e5e5ea' },
  downloadNote: { fontSize: 11, color: '#636366' },
  hfTokenRow: { flexDirection: 'row', gap: 8, alignItems: 'center' },
  hfTokenLink: {
    backgroundColor: '#2c2c2e',
    borderRadius: 8,
    paddingHorizontal: 12,
    paddingVertical: 10,
  },
  hfTokenLinkText: { fontSize: 13, color: '#0a84ff', fontWeight: '600' },
  downloadBtn: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 10,
    backgroundColor: '#0a84ff18',
    borderRadius: 8,
    borderWidth: 1,
    borderColor: '#0a84ff40',
    paddingHorizontal: 12,
    paddingVertical: 10,
  },
  downloadBtnLabel: { fontSize: 13, fontWeight: '600', color: '#e5e5ea' },
  downloadBtnSub: { fontSize: 11, color: '#636366', marginTop: 2 },
  progressBar: {
    height: 3,
    backgroundColor: '#0a84ff30',
    borderRadius: 2,
    marginTop: 6,
    overflow: 'hidden',
  },
  progressFill: { height: 3, backgroundColor: '#0a84ff', borderRadius: 2 },
  fileRow: { flexDirection: 'row', gap: 8 },
  filePickerBtn: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 10,
    backgroundColor: '#2c2c2e',
    borderRadius: 8,
    paddingHorizontal: 12,
    paddingVertical: 12,
  },
  filePickerText: {
    flex: 1,
    fontSize: 14,
    color: '#e5e5ea',
  },
  scanBtn: {
    backgroundColor: '#2c2c2e',
    borderRadius: 8,
    paddingHorizontal: 14,
    justifyContent: 'center',
    alignItems: 'center',
  },
  foundFilesBox: {
    backgroundColor: '#1c2a1c',
    borderRadius: 8,
    borderWidth: 1,
    borderColor: '#30d15840',
    padding: 10,
    gap: 6,
  },
  foundFilesTitle: { fontSize: 11, color: '#30d158', fontWeight: '600' },
  foundFileRow: {
    flexDirection: 'row',
    justifyContent: 'space-between',
    alignItems: 'center',
    paddingVertical: 6,
    borderTopWidth: 1,
    borderTopColor: '#2c2c2e',
  },
  foundFileName: { flex: 1, fontSize: 13, color: '#e5e5ea' },
  foundFileSelect: { fontSize: 12, color: '#0a84ff', fontWeight: '600', paddingLeft: 12 },
  hint: { fontSize: 12, color: '#48484a', lineHeight: 18 },
  desktopTip: { fontSize: 13, color: '#636366', lineHeight: 20 },
  tailscaleBadge: {
    backgroundColor: '#1a2a1a',
    borderRadius: 8,
    borderWidth: 1,
    borderColor: '#30d15840',
    padding: 12,
    gap: 4,
  },
  tailscaleBadgeText: {
    fontSize: 12,
    fontWeight: '600',
    color: '#30d158',
  },
  tailscaleHint: {
    fontSize: 12,
    color: '#636366',
    lineHeight: 18,
  },
  primaryBtn: {
    backgroundColor: '#0a84ff',
    borderRadius: 12,
    paddingVertical: 14,
    alignItems: 'center',
  },
  primaryBtnText: { color: '#fff', fontSize: 16, fontWeight: '600' },
  outlineBtn: {
    borderWidth: 1,
    borderRadius: 8,
    paddingVertical: 9,
    paddingHorizontal: 14,
  },
  outlineBtnText: { fontSize: 14, fontWeight: '600' },
  scannerRoot: { flex: 1, backgroundColor: '#000' },
  scannerHeader: {
    flexDirection: 'row',
    justifyContent: 'space-between',
    alignItems: 'center',
    padding: 16,
  },
  scannerTitle: { fontSize: 17, fontWeight: '600', color: '#fff' },
  scannerClose: { fontSize: 16, color: '#0a84ff' },
  camera: { flex: 1 },
  scannerHint: {
    textAlign: 'center',
    padding: 20,
    fontSize: 14,
    color: '#636366',
  },
});
