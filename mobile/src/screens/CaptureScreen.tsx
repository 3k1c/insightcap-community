/**
 * InsightCAP Mobile — Capture Screen
 *
 * 快速擷取：文字輸入 → POST /api/capture → 桌面知識庫 inbox
 * 支援 Android Share Intent（文字、網頁連結）
 */

import React, { useState, useEffect } from 'react';
import {
  View,
  Text,
  TextInput,
  TouchableOpacity,
  StyleSheet,
  SafeAreaView,
  KeyboardAvoidingView,
  Platform,
  Alert,
  NativeModules,
  AppState,
} from 'react-native';
import { Mic, Camera } from 'lucide-react-native';
import { captureToDesktop } from '../services/desktop-api';
import { getDesktopSettings } from '../services/settings-store';

const { ShareIntentModule } = NativeModules;

export default function CaptureScreen() {
  const [content, setContent] = useState('');
  const [isSending, setIsSending] = useState(false);

  const isDesktopConfigured = !!getDesktopSettings()?.url;

  // 檢查 Share Intent
  useEffect(() => {
    checkIntent();
    const sub = AppState.addEventListener('change', next => {
      if (next === 'active') checkIntent();
    });
    return () => sub.remove();
  }, []);

  async function checkIntent() {
    if (!ShareIntentModule) return;
    try {
      const data = await ShareIntentModule.getSharedData();
      if (data?.value) {
        setContent(prev => (prev ? prev + '\n' + data.value : data.value));
      }
    } catch {
      // ignore
    }
  }

  async function handleSend() {
    if (!content.trim()) return;
    if (!isDesktopConfigured) {
      Alert.alert('尚未連線', '請先前往設定頁面填入桌面端 URL');
      return;
    }

    setIsSending(true);
    try {
      await captureToDesktop(content);
      setContent('');
      Alert.alert('已擷取', '內容已成功儲存至桌面知識庫 inbox');
    } catch {
      Alert.alert('擷取失敗', '無法連線到桌面端，請確認雙方在同一 WiFi 下');
    } finally {
      setIsSending(false);
    }
  }

  return (
    <SafeAreaView style={s.root}>
      <KeyboardAvoidingView
        style={s.container}
        behavior={Platform.OS === 'ios' ? 'padding' : undefined}
      >
        <View style={s.content}>
          <TextInput
            style={s.input}
            multiline
            placeholder="貼上文字、網址，或輸入想法..."
            placeholderTextColor="#636366"
            value={content}
            onChangeText={setContent}
            autoFocus
          />

          <View style={s.toolbar}>
            <View style={s.tools}>
              <TouchableOpacity
                style={s.toolBtn}
                onPress={() => Alert.alert('提示', '語音辨識開發中')}
              >
                <Mic color="#a1a1aa" size={22} />
              </TouchableOpacity>
              <TouchableOpacity
                style={s.toolBtn}
                onPress={() => Alert.alert('提示', '相機快拍開發中')}
              >
                <Camera color="#a1a1aa" size={22} />
              </TouchableOpacity>
            </View>

            <TouchableOpacity
              style={[
                s.sendBtn,
                (!content.trim() || isSending) && s.sendBtnDisabled,
              ]}
              onPress={handleSend}
              disabled={!content.trim() || isSending}
            >
              <Text style={s.sendBtnText}>
                {isSending ? '擷取中...' : '發送'}
              </Text>
            </TouchableOpacity>
          </View>
        </View>
      </KeyboardAvoidingView>
    </SafeAreaView>
  );
}

// ─── Styles ─────────────────────────────────────────────────────────────────

const s = StyleSheet.create({
  root: { flex: 1, backgroundColor: '#0f0f11' },
  container: { flex: 1 },
  content: { flex: 1, padding: 16, gap: 16 },
  input: {
    flex: 1,
    backgroundColor: '#1c1c1e',
    borderRadius: 16,
    padding: 20,
    color: '#fff',
    fontSize: 17,
    lineHeight: 24,
    textAlignVertical: 'top',
  },
  toolbar: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'space-between',
    paddingVertical: 8,
  },
  tools: { flexDirection: 'row', gap: 12 },
  toolBtn: {
    width: 44,
    height: 44,
    borderRadius: 22,
    backgroundColor: '#1c1c1e',
    alignItems: 'center',
    justifyContent: 'center',
  },
  sendBtn: {
    backgroundColor: '#0a84ff',
    paddingHorizontal: 24,
    paddingVertical: 12,
    borderRadius: 20,
  },
  sendBtnDisabled: { backgroundColor: '#2c2c2e' },
  sendBtnText: { color: '#fff', fontSize: 16, fontWeight: '600' },
});
