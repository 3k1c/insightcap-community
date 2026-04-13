/**
 * InsightCAP Mobile — Conversation List Screen
 *
 * Drawer 側邊欄內容：從桌面端 GET /api/conversations 取得對話列表
 * 點擊 → ChatScreen（帶入 conversationId + title）
 */

import React, { useState, useCallback } from 'react';
import {
  View,
  Text,
  FlatList,
  TouchableOpacity,
  StyleSheet,
  SafeAreaView,
  ActivityIndicator,
  Alert,
  RefreshControl,
} from 'react-native';
import { useFocusEffect, useNavigation } from '@react-navigation/native';
import {
  MessageSquare,
  MonitorOff,
  MessageCircleMore,
  Plus,
} from 'lucide-react-native';
import type { DrawerNavigationProp } from '@react-navigation/drawer';
import type { DrawerParamList } from '../../App';
import {
  fetchConversations,
  createConversation,
  type ConversationItem,
} from '../services/desktop-api';
import { getDesktopSettings } from '../services/settings-store';

// ─── Types ──────────────────────────────────────────────────────────────────

type NavProp = DrawerNavigationProp<DrawerParamList, 'Chat'>;

// ─── Helpers ────────────────────────────────────────────────────────────────

function formatDate(iso: string): string {
  try {
    const d = new Date(iso);
    const now = new Date();
    const diffDays = Math.floor(
      (now.getTime() - d.getTime()) / 86_400_000,
    );

    if (diffDays === 0) {
      return d.toLocaleTimeString('zh-TW', {
        hour: '2-digit',
        minute: '2-digit',
      });
    }
    if (diffDays === 1) return '昨天';
    if (diffDays < 7) return `${diffDays} 天前`;
    return d.toLocaleDateString('zh-TW', {
      month: 'short',
      day: 'numeric',
    });
  } catch {
    return '';
  }
}

// ─── ConversationListScreen ─────────────────────────────────────────────────

export default function ConversationListScreen() {
  const navigation = useNavigation<NavProp>();
  const [conversations, setConversations] = useState<ConversationItem[]>(
    [],
  );
  const [loading, setLoading] = useState(false);
  const [creating, setCreating] = useState(false);

  const isDesktopConfigured = !!getDesktopSettings()?.url;

  useFocusEffect(
    useCallback(() => {
      if (isDesktopConfigured) load();
    }, [isDesktopConfigured]),
  );

  async function load() {
    setLoading(true);
    const data = await fetchConversations();
    setConversations(data);
    setLoading(false);
  }

  async function handleNewConversation() {
    if (!isDesktopConfigured) {
      Alert.alert('未連線', '請先在設定頁填入桌面端連線資訊');
      return;
    }
    setCreating(true);
    const result = await createConversation();
    setCreating(false);
    if (result) {
      navigation.navigate('Chat', {
        conversationId: result.id,
        title: result.title,
      });
    } else {
      Alert.alert('建立失敗', '無法連線到桌面端');
    }
  }

  // ─── Render ─────────────────────────────────────────────────────────────

  const renderHeader = () => (
    <View style={s.header}>
      <Text style={s.headerTitle}>對話歷史</Text>
      <TouchableOpacity
        style={s.newBtn}
        onPress={handleNewConversation}
        disabled={creating}
      >
        {creating ? (
          <ActivityIndicator size="small" color="#0a84ff" />
        ) : (
          <Plus color="#0a84ff" size={24} />
        )}
      </TouchableOpacity>
    </View>
  );

  if (!isDesktopConfigured) {
    return (
      <SafeAreaView style={s.root}>
        {renderHeader()}
        <View style={s.empty}>
          <MonitorOff color="#e5e5ea" size={40} />
          <Text style={s.emptyTitle}>尚未連線桌面端</Text>
          <Text style={s.emptyHint}>請在設定頁填入桌面 URL</Text>
        </View>
      </SafeAreaView>
    );
  }

  return (
    <SafeAreaView style={s.root}>
      {renderHeader()}
      <FlatList
        data={conversations}
        keyExtractor={item => item.id}
        contentContainerStyle={s.list}
        refreshControl={
          <RefreshControl
            refreshing={loading}
            onRefresh={load}
            tintColor="#636366"
          />
        }
        renderItem={({ item }) => (
          <ConversationRow
            item={item}
            onPress={() => {
              navigation.navigate('Chat', {
                conversationId: item.id,
                title: item.title,
              });
            }}
          />
        )}
        ListEmptyComponent={
          loading ? null : (
            <View style={s.empty}>
              <MessageCircleMore color="#636366" size={40} />
              <Text style={s.emptyTitle}>沒有對話記錄</Text>
            </View>
          )
        }
      />
    </SafeAreaView>
  );
}

// ─── ConversationRow ────────────────────────────────────────────────────────

function ConversationRow({
  item,
  onPress,
}: {
  item: ConversationItem;
  onPress: () => void;
}) {
  return (
    <TouchableOpacity
      style={s.row}
      onPress={onPress}
      activeOpacity={0.7}
    >
      <View style={s.rowIcon}>
        <MessageSquare color="#a1a1aa" size={18} />
      </View>
      <View style={s.rowBody}>
        <View style={s.rowTop}>
          <Text style={s.rowTitle} numberOfLines={1}>
            {item.title}
          </Text>
          <Text style={s.rowDate}>{formatDate(item.updated_at)}</Text>
        </View>
        {item.summary ? (
          <Text style={s.rowSummary} numberOfLines={2}>
            {item.summary}
          </Text>
        ) : null}
      </View>
    </TouchableOpacity>
  );
}

// ─── Styles ─────────────────────────────────────────────────────────────────

const s = StyleSheet.create({
  root: { flex: 1, backgroundColor: '#0f0f11' },
  header: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'space-between',
    paddingHorizontal: 16,
    paddingVertical: 14,
    borderBottomWidth: StyleSheet.hairlineWidth,
    borderBottomColor: '#2c2c2e',
  },
  headerTitle: { fontSize: 20, fontWeight: '700', color: '#fff' },
  newBtn: {
    width: 34,
    height: 34,
    borderRadius: 17,
    backgroundColor: '#1c1c1e',
    alignItems: 'center',
    justifyContent: 'center',
  },
  list: { paddingBottom: 20 },
  row: {
    flexDirection: 'row',
    alignItems: 'flex-start',
    paddingHorizontal: 12,
    paddingVertical: 12,
    gap: 12,
  },
  rowIcon: {
    width: 32,
    height: 32,
    borderRadius: 8,
    backgroundColor: '#1c1c1e',
    alignItems: 'center',
    justifyContent: 'center',
    flexShrink: 0,
  },
  rowBody: { flex: 1 },
  rowTop: {
    flexDirection: 'row',
    justifyContent: 'space-between',
    alignItems: 'baseline',
    gap: 8,
    marginBottom: 4,
  },
  rowTitle: {
    flex: 1,
    fontSize: 15,
    fontWeight: '600',
    color: '#e5e5ea',
  },
  rowDate: { fontSize: 12, color: '#636366', flexShrink: 0 },
  rowSummary: { fontSize: 13, color: '#636366', lineHeight: 18 },
  empty: { paddingTop: 100, alignItems: 'center', gap: 10 },
  emptyTitle: { fontSize: 17, fontWeight: '600', color: '#e5e5ea' },
  emptyHint: {
    fontSize: 13,
    color: '#636366',
    textAlign: 'center',
    lineHeight: 20,
  },
});
