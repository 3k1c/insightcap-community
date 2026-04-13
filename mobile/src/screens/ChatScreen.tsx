/**
 * InsightCAP Mobile — Chat Screen
 *
 * 預設首頁，支援三種推理模式（local / cloud / desktop）
 * header 切換模式，streaming token 即時顯示
 */

import React, { useState, useRef, useCallback, useEffect } from 'react';
import {
  View,
  Text,
  TextInput,
  TouchableOpacity,
  FlatList,
  KeyboardAvoidingView,
  Platform,
  ActivityIndicator,
  StyleSheet,
  SafeAreaView,
  Keyboard,
} from 'react-native';
import { ArrowUp, Square, ChevronDown } from 'lucide-react-native';
import type { DrawerScreenProps } from '@react-navigation/drawer';
import type { DrawerParamList } from '../../App';
import {
  createProvider,
  type InferenceMode,
  type InferenceProvider,
} from '../services/inference';
import { getInferenceMode } from '../services/settings-store';
import { captureToDesktop, fetchMessages } from '../services/desktop-api';

// ─── Types ──────────────────────────────────────────────────────────────────

type Props = DrawerScreenProps<DrawerParamList, 'Chat'>;

interface Message {
  id: string;
  role: 'user' | 'assistant';
  content: string;
  pending?: boolean;
}

// ─── Constants ──────────────────────────────────────────────────────────────

const MODE_LABELS: Record<InferenceMode, string> = {
  local: '本機 Gemma 4',
  cloud: '雲端 API',
  desktop: '桌面代理',
};

const MODE_COLORS: Record<InferenceMode, string> = {
  local: '#30d158',
  cloud: '#0a84ff',
  desktop: '#bf5af2',
};

// ─── ChatScreen ─────────────────────────────────────────────────────────────

export default function ChatScreen({ route }: Props) {
  const conversationId = route.params?.conversationId;

  const [messages, setMessages] = useState<Message[]>([]);
  const [input, setInput] = useState('');
  const [mode, setMode] = useState<InferenceMode>(getInferenceMode());
  const [isGenerating, setIsGenerating] = useState(false);
  const [modePickerOpen, setModePickerOpen] = useState(false);
  const [historyLoaded, setHistoryLoaded] = useState(false);

  const providerRef = useRef<InferenceProvider | null>(null);
  const listRef = useRef<FlatList<Message>>(null);
  const streamingIdRef = useRef<string | null>(null);

  // 切換模式時重建 provider
  useEffect(() => {
    createProvider(mode).then(p => {
      providerRef.current = p;
    });
  }, [mode]);

  // 從桌面載入歷史訊息（desktop 模式 + 有 conversationId）
  useEffect(() => {
    if (!conversationId || historyLoaded) return;
    fetchMessages(conversationId).then(msgs => {
      if (msgs.length > 0) {
        setMessages(
          msgs.map(m => ({
            id: m.id,
            role: m.role as 'user' | 'assistant',
            content: m.content,
          })),
        );
      }
      setHistoryLoaded(true);
    });
  }, [conversationId, historyLoaded]);

  // ─── Send ───────────────────────────────────────────────────────────────

  const send = useCallback(async () => {
    const text = input.trim();
    if (!text || isGenerating) return;

    Keyboard.dismiss();
    setInput('');
    setIsGenerating(true);

    const userMsg: Message = {
      id: `u_${Date.now()}`,
      role: 'user',
      content: text,
    };
    const pendingId = `a_${Date.now()}`;
    const pendingMsg: Message = {
      id: pendingId,
      role: 'assistant',
      content: '',
      pending: true,
    };

    setMessages(prev => [...prev, userMsg, pendingMsg]);
    streamingIdRef.current = pendingId;

    // 組裝 history（最近 6 輪）
    const history = messages
      .filter(m => !m.pending)
      .slice(-12)
      .map(m => ({ role: m.role, content: m.content }));

    const provider = providerRef.current;
    if (!provider) {
      updateMessage(pendingId, '[錯誤] 推理引擎未就緒', false);
      setIsGenerating(false);
      return;
    }

    await provider.generate(text, {
      history,
      onToken: token => {
        setMessages(prev =>
          prev.map(m =>
            m.id === pendingId ? { ...m, content: m.content + token } : m,
          ),
        );
        listRef.current?.scrollToEnd({ animated: false });
      },
      onDone: fullText => {
        updateMessage(pendingId, fullText, false);
        setIsGenerating(false);
        streamingIdRef.current = null;
      },
      onError: error => {
        updateMessage(pendingId, `[錯誤] ${error}`, false);
        setIsGenerating(false);
        streamingIdRef.current = null;
      },
    });
  }, [input, isGenerating, messages]);

  // ─── Cancel ─────────────────────────────────────────────────────────────

  const cancel = useCallback(() => {
    providerRef.current?.cancel();
    const id = streamingIdRef.current;
    if (id) {
      setMessages(prev =>
        prev.map(m =>
          m.id === id
            ? { ...m, pending: false, content: m.content || '（已取消）' }
            : m,
        ),
      );
    }
    setIsGenerating(false);
    streamingIdRef.current = null;
  }, []);

  // ─── Helpers ────────────────────────────────────────────────────────────

  function updateMessage(id: string, content: string, pending: boolean) {
    setMessages(prev =>
      prev.map(m => (m.id === id ? { ...m, content, pending } : m)),
    );
  }

  const saveToKB = useCallback(async (content: string) => {
    try {
      await captureToDesktop(content);
    } catch {
      // 靜默
    }
  }, []);

  // ─── Render ─────────────────────────────────────────────────────────────

  return (
    <SafeAreaView style={s.root}>
      {/* Mode Picker Toggle */}
      <View style={s.topBar}>
        <TouchableOpacity
          style={[s.modeBadge, { borderColor: MODE_COLORS[mode] }]}
          onPress={() => setModePickerOpen(v => !v)}
        >
          <View
            style={[s.modeDot, { backgroundColor: MODE_COLORS[mode] }]}
          />
          <Text style={[s.modeText, { color: MODE_COLORS[mode] }]}>
            {MODE_LABELS[mode]}
          </Text>
          <ChevronDown color={MODE_COLORS[mode]} size={14} />
        </TouchableOpacity>
      </View>

      {/* Mode Picker Dropdown */}
      {modePickerOpen && (
        <View style={s.modePicker}>
          {(Object.keys(MODE_LABELS) as InferenceMode[]).map(m => (
            <TouchableOpacity
              key={m}
              style={[
                s.modePickerItem,
                m === mode && s.modePickerItemActive,
              ]}
              onPress={() => {
                setMode(m);
                setModePickerOpen(false);
              }}
            >
              <View
                style={[s.modeDot, { backgroundColor: MODE_COLORS[m] }]}
              />
              <Text style={s.modePickerText}>{MODE_LABELS[m]}</Text>
            </TouchableOpacity>
          ))}
        </View>
      )}

      {/* Messages + Input */}
      <KeyboardAvoidingView
        style={s.flex}
        behavior={Platform.OS === 'ios' ? 'padding' : undefined}
        keyboardVerticalOffset={90}
      >
        <FlatList
          ref={listRef}
          data={messages}
          keyExtractor={m => m.id}
          style={s.list}
          contentContainerStyle={s.listContent}
          onContentSizeChange={() =>
            listRef.current?.scrollToEnd({ animated: false })
          }
          renderItem={({ item }) => (
            <MessageBubble
              message={item}
              onSaveToKB={
                item.role === 'assistant' && !item.pending
                  ? saveToKB
                  : undefined
              }
            />
          )}
          ListEmptyComponent={<EmptyState mode={mode} />}
        />

        {/* Input Bar */}
        <View style={s.inputRow}>
          <TextInput
            style={s.input}
            value={input}
            onChangeText={setInput}
            placeholder="輸入問題…"
            placeholderTextColor="#48484a"
            multiline
            maxLength={2000}
            onSubmitEditing={send}
          />
          {isGenerating ? (
            <TouchableOpacity style={s.cancelBtn} onPress={cancel}>
              <Square color="#ff453a" size={16} fill="#ff453a" />
            </TouchableOpacity>
          ) : (
            <TouchableOpacity
              style={[s.sendBtn, !input.trim() && s.sendBtnDisabled]}
              onPress={send}
              disabled={!input.trim()}
            >
              <ArrowUp color="#fff" size={20} strokeWidth={3} />
            </TouchableOpacity>
          )}
        </View>
      </KeyboardAvoidingView>
    </SafeAreaView>
  );
}

// ─── MessageBubble ──────────────────────────────────────────────────────────

function MessageBubble({
  message,
  onSaveToKB,
}: {
  message: Message;
  onSaveToKB?: (content: string) => void;
}) {
  const isUser = message.role === 'user';

  return (
    <View style={[s.bubbleRow, isUser && s.bubbleRowUser]}>
      <View
        style={[s.bubble, isUser ? s.bubbleUser : s.bubbleAssistant]}
      >
        <Text style={[s.bubbleText, isUser && s.bubbleTextUser]}>
          {message.content}
        </Text>
        {message.pending && (
          <ActivityIndicator
            size="small"
            color="#636366"
            style={{ marginTop: 4 }}
          />
        )}
        {onSaveToKB && (
          <TouchableOpacity
            style={s.saveBtn}
            onPress={() => onSaveToKB(message.content)}
          >
            <Text style={s.saveBtnText}>存入知識庫</Text>
          </TouchableOpacity>
        )}
      </View>
    </View>
  );
}

// ─── EmptyState ─────────────────────────────────────────────────────────────

function EmptyState({ mode }: { mode: InferenceMode }) {
  const hints: Record<InferenceMode, string> = {
    local: '使用手機本地 Gemma 4 推理\n離線也可使用',
    cloud: '直接呼叫雲端 LLM API\n需要 API Key 和網路',
    desktop: '透過桌面端推理並存取知識庫\n需要連線到同一 WiFi',
  };
  return (
    <View style={s.empty}>
      <Text style={s.emptyTitle}>InsightCAP</Text>
      <Text style={s.emptyHint}>{hints[mode]}</Text>
    </View>
  );
}

// ─── Styles ─────────────────────────────────────────────────────────────────

const s = StyleSheet.create({
  root: { flex: 1, backgroundColor: '#0f0f11' },
  flex: { flex: 1 },
  topBar: {
    alignItems: 'center',
    paddingVertical: 8,
    borderBottomWidth: StyleSheet.hairlineWidth,
    borderBottomColor: '#1c1c1e',
  },
  modeBadge: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 6,
    borderWidth: 1,
    borderRadius: 20,
    paddingHorizontal: 10,
    paddingVertical: 5,
  },
  modeDot: { width: 7, height: 7, borderRadius: 4 },
  modeText: { fontSize: 12, fontWeight: '600' },
  modePicker: {
    backgroundColor: '#1c1c1e',
    marginHorizontal: 12,
    borderRadius: 12,
    overflow: 'hidden',
    borderWidth: StyleSheet.hairlineWidth,
    borderColor: '#2c2c2e',
  },
  modePickerItem: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 10,
    padding: 14,
    borderBottomWidth: StyleSheet.hairlineWidth,
    borderBottomColor: '#2c2c2e',
  },
  modePickerItemActive: { backgroundColor: '#2c2c2e' },
  modePickerText: { fontSize: 15, color: '#e5e5ea' },
  list: { flex: 1 },
  listContent: { padding: 12, gap: 12, paddingBottom: 20 },
  bubbleRow: { flexDirection: 'row' },
  bubbleRowUser: { justifyContent: 'flex-end' },
  bubble: { maxWidth: '80%', borderRadius: 16, padding: 12 },
  bubbleUser: { backgroundColor: '#0a84ff', borderBottomRightRadius: 4 },
  bubbleAssistant: {
    backgroundColor: '#1c1c1e',
    borderBottomLeftRadius: 4,
    borderWidth: StyleSheet.hairlineWidth,
    borderColor: '#2c2c2e',
  },
  bubbleText: { fontSize: 15, color: '#e5e5ea', lineHeight: 22 },
  bubbleTextUser: { color: '#fff' },
  saveBtn: { marginTop: 8, alignSelf: 'flex-end' },
  saveBtnText: { fontSize: 12, color: '#636366' },
  inputRow: {
    flexDirection: 'row',
    alignItems: 'flex-end',
    gap: 8,
    padding: 12,
    borderTopWidth: StyleSheet.hairlineWidth,
    borderTopColor: '#2c2c2e',
    backgroundColor: '#0f0f11',
  },
  input: {
    flex: 1,
    backgroundColor: '#1c1c1e',
    borderRadius: 20,
    paddingHorizontal: 14,
    paddingVertical: 10,
    color: '#fff',
    fontSize: 15,
    maxHeight: 120,
  },
  sendBtn: {
    width: 36,
    height: 36,
    borderRadius: 18,
    backgroundColor: '#0a84ff',
    alignItems: 'center',
    justifyContent: 'center',
  },
  sendBtnDisabled: { backgroundColor: '#2c2c2e' },
  cancelBtn: {
    width: 36,
    height: 36,
    borderRadius: 18,
    backgroundColor: '#3a1c1c',
    alignItems: 'center',
    justifyContent: 'center',
  },
  empty: {
    flex: 1,
    alignItems: 'center',
    justifyContent: 'center',
    paddingTop: 80,
    gap: 12,
  },
  emptyTitle: { fontSize: 24, fontWeight: '700', color: '#fff' },
  emptyHint: {
    fontSize: 14,
    color: '#636366',
    textAlign: 'center',
    lineHeight: 22,
  },
});
