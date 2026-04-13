/**
 * InsightCAP Mobile — Knowledge Screen
 *
 * 從桌面端 GET /api/sources 取得知識庫來源列表
 * 支援搜尋過濾，顯示媒體類型、chunk 數、日期
 */

import React, { useState, useCallback } from 'react';
import {
  View,
  Text,
  FlatList,
  TouchableOpacity,
  StyleSheet,
  SafeAreaView,
  RefreshControl,
  TextInput,
} from 'react-native';
import { useFocusEffect } from '@react-navigation/native';
import {
  FileText,
  Link,
  Video,
  Image as ImageIcon,
  Folder,
  MonitorOff,
  Database,
} from 'lucide-react-native';
import { fetchSources, type SourceItem } from '../services/desktop-api';
import { getDesktopSettings } from '../services/settings-store';

// ─── Helpers ────────────────────────────────────────────────────────────────

function formatDate(iso: string): string {
  try {
    const d = new Date(iso);
    return d.toLocaleDateString('zh-TW', {
      month: 'short',
      day: 'numeric',
      year: 'numeric',
    });
  } catch {
    return '';
  }
}

function MediaTypeIcon({
  mediaType,
  category,
}: {
  mediaType?: string;
  category?: string;
}) {
  const color = '#a1a1aa';
  const size = 20;
  if (category === 'editor_doc') return <FileText color={color} size={size} />;
  switch (mediaType) {
    case 'pdf':
      return <FileText color={color} size={size} />;
    case 'url':
      return <Link color={color} size={size} />;
    case 'video':
      return <Video color={color} size={size} />;
    case 'image':
      return <ImageIcon color={color} size={size} />;
    case 'file':
      return <Folder color={color} size={size} />;
    default:
      return <FileText color={color} size={size} />;
  }
}

function mediaTypeLabel(mediaType?: string, category?: string): string {
  if (category === 'editor_doc') return '編輯器文件';
  switch (mediaType) {
    case 'pdf':
      return 'PDF';
    case 'url':
      return '網頁';
    case 'video':
      return '影片';
    case 'image':
      return '圖片';
    case 'file':
      return '文件';
    case 'text':
      return '文字';
    case 'markdown':
      return 'Markdown';
    default:
      return '來源';
  }
}

// ─── KnowledgeScreen ────────────────────────────────────────────────────────

export default function KnowledgeScreen() {
  const [sources, setSources] = useState<SourceItem[]>([]);
  const [filtered, setFiltered] = useState<SourceItem[]>([]);
  const [loading, setLoading] = useState(false);
  const [query, setQuery] = useState('');

  const isDesktopConfigured = !!getDesktopSettings()?.url;

  useFocusEffect(
    useCallback(() => {
      if (isDesktopConfigured) load();
    }, [isDesktopConfigured]),
  );

  async function load() {
    setLoading(true);
    const data = await fetchSources();
    setSources(data);
    setFiltered(data);
    setLoading(false);
  }

  function handleSearch(text: string) {
    setQuery(text);
    if (!text.trim()) {
      setFiltered(sources);
    } else {
      const lower = text.toLowerCase();
      setFiltered(sources.filter(s => s.title.toLowerCase().includes(lower)));
    }
  }

  // ─── Not Connected ────────────────────────────────────────────────────

  if (!isDesktopConfigured) {
    return (
      <SafeAreaView style={s.root}>
        <View style={s.header}>
          <Text style={s.headerTitle}>知識庫</Text>
        </View>
        <View style={s.empty}>
          <MonitorOff color="#636366" size={40} />
          <Text style={s.emptyTitle}>尚未連線桌面端</Text>
          <Text style={s.emptyHint}>
            請前往「設定」填入桌面 URL 與 Token
          </Text>
        </View>
      </SafeAreaView>
    );
  }

  // ─── Connected ────────────────────────────────────────────────────────

  return (
    <SafeAreaView style={s.root}>
      <View style={s.header}>
        <Text style={s.headerTitle}>知識庫</Text>
        <Text style={s.headerCount}>
          {sources.length > 0 ? `${sources.length} 個來源` : ''}
        </Text>
      </View>

      {/* Search */}
      <View style={s.searchRow}>
        <TextInput
          style={s.searchInput}
          value={query}
          onChangeText={handleSearch}
          placeholder="搜尋來源標題…"
          placeholderTextColor="#48484a"
          returnKeyType="search"
          clearButtonMode="while-editing"
        />
      </View>

      {/* List */}
      <FlatList
        data={filtered}
        keyExtractor={item => item.id}
        contentContainerStyle={s.list}
        refreshControl={
          <RefreshControl
            refreshing={loading}
            onRefresh={load}
            tintColor="#636366"
          />
        }
        renderItem={({ item }) => <SourceRow item={item} />}
        ListEmptyComponent={
          loading ? null : (
            <View style={s.empty}>
              <Database color="#636366" size={40} />
              <Text style={s.emptyTitle}>
                {query ? '找不到符合的來源' : '知識庫是空的'}
              </Text>
              <Text style={s.emptyHint}>
                {query
                  ? '試試其他關鍵字'
                  : '在桌面端導入文件或擷取網頁後會顯示在這裡'}
              </Text>
            </View>
          )
        }
      />
    </SafeAreaView>
  );
}

// ─── SourceRow ──────────────────────────────────────────────────────────────

function SourceRow({ item }: { item: SourceItem }) {
  const typeLabel = mediaTypeLabel(item.media_type, item.source_category);

  return (
    <TouchableOpacity style={s.row} activeOpacity={0.7}>
      <View style={s.rowIcon}>
        <MediaTypeIcon
          mediaType={item.media_type}
          category={item.source_category}
        />
      </View>
      <View style={s.rowBody}>
        <Text style={s.rowTitle} numberOfLines={2}>
          {item.title}
        </Text>
        <View style={s.rowMeta}>
          <View style={s.typeBadge}>
            <Text style={s.typeText}>{typeLabel}</Text>
          </View>
          {item.capture_count > 0 && (
            <Text style={s.chunkCount}>{item.capture_count} chunks</Text>
          )}
          <Text style={s.dateText}>{formatDate(item.captured_at)}</Text>
        </View>
      </View>
    </TouchableOpacity>
  );
}

// ─── Styles ─────────────────────────────────────────────────────────────────

const s = StyleSheet.create({
  root: { flex: 1, backgroundColor: '#0f0f11' },
  header: {
    flexDirection: 'row',
    alignItems: 'baseline',
    justifyContent: 'space-between',
    paddingHorizontal: 16,
    paddingVertical: 14,
    borderBottomWidth: StyleSheet.hairlineWidth,
    borderBottomColor: '#2c2c2e',
  },
  headerTitle: { fontSize: 20, fontWeight: '700', color: '#fff' },
  headerCount: { fontSize: 13, color: '#636366' },
  searchRow: {
    paddingHorizontal: 12,
    paddingVertical: 8,
    borderBottomWidth: StyleSheet.hairlineWidth,
    borderBottomColor: '#1c1c1e',
  },
  searchInput: {
    backgroundColor: '#1c1c1e',
    borderRadius: 10,
    paddingHorizontal: 12,
    paddingVertical: 8,
    color: '#fff',
    fontSize: 15,
  },
  list: { paddingBottom: 20 },
  row: {
    flexDirection: 'row',
    alignItems: 'flex-start',
    paddingHorizontal: 16,
    paddingVertical: 12,
    gap: 12,
    borderBottomWidth: StyleSheet.hairlineWidth,
    borderBottomColor: '#1c1c1e',
  },
  rowIcon: {
    width: 40,
    height: 40,
    borderRadius: 10,
    backgroundColor: '#1c1c1e',
    alignItems: 'center',
    justifyContent: 'center',
    flexShrink: 0,
  },
  rowBody: { flex: 1 },
  rowTitle: {
    fontSize: 14,
    fontWeight: '500',
    color: '#e5e5ea',
    lineHeight: 20,
    marginBottom: 6,
  },
  rowMeta: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 8,
    flexWrap: 'wrap',
  },
  typeBadge: {
    backgroundColor: '#2c2c2e',
    borderRadius: 4,
    paddingHorizontal: 6,
    paddingVertical: 2,
  },
  typeText: { fontSize: 11, color: '#8e8e93', fontWeight: '500' },
  chunkCount: { fontSize: 11, color: '#48484a' },
  dateText: { fontSize: 11, color: '#48484a', marginLeft: 'auto' },
  empty: { paddingTop: 100, alignItems: 'center', gap: 10 },
  emptyTitle: { fontSize: 17, fontWeight: '600', color: '#e5e5ea' },
  emptyHint: {
    fontSize: 13,
    color: '#636366',
    textAlign: 'center',
    lineHeight: 20,
    paddingHorizontal: 32,
  },
});
