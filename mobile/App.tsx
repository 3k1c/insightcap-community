/**
 * InsightCAP Mobile — Root App
 *
 * Drawer Navigator：
 *   側邊欄：ConversationListScreen（對話歷史）
 *   主畫面：ChatScreen（預設）
 *   其他：KnowledgeScreen / CaptureScreen / SettingsScreen
 */

import 'react-native-gesture-handler';
import React, { useEffect } from 'react';
import { View, StyleSheet, StatusBar } from 'react-native';
import { NavigationContainer } from '@react-navigation/native';
import { createDrawerNavigator } from '@react-navigation/drawer';
import { Menu, Settings, BookOpen, PenSquare } from 'lucide-react-native';

import ChatScreen from './src/screens/ChatScreen';
import ConversationListScreen from './src/screens/ConversationListScreen';
import KnowledgeScreen from './src/screens/KnowledgeScreen';
import CaptureScreen from './src/screens/CaptureScreen';
import SettingsScreen from './src/screens/SettingsScreen';
import { loadSettings } from './src/services/settings-store';

// ─── Navigation Types ───────────────────────────────────────────────────────

export type DrawerParamList = {
  Chat: { conversationId?: string; title?: string } | undefined;
  Knowledge: undefined;
  Capture: undefined;
  Settings: undefined;
};

// ─── Navigator ──────────────────────────────────────────────────────────────

const Drawer = createDrawerNavigator<DrawerParamList>();

export default function App() {
  useEffect(() => {
    loadSettings();
  }, []);

  return (
    <NavigationContainer>
      <StatusBar barStyle="light-content" backgroundColor="#0f0f11" />
      <Drawer.Navigator
        initialRouteName="Chat"
        drawerContent={() => <ConversationListScreen />}
        screenOptions={({ navigation }) => ({
          headerStyle: styles.header,
          headerTintColor: '#fff',
          headerTitleStyle: styles.headerTitle,
          drawerStyle: styles.drawer,
          headerLeft: () => (
            <View style={styles.headerLeft}>
              <Menu
                color="#fff"
                size={24}
                onPress={() => navigation.openDrawer()}
              />
              <BookOpen
                color="#fff"
                size={20}
                onPress={() => navigation.navigate('Knowledge')}
              />
            </View>
          ),
          headerRight: () => (
            <View style={styles.headerRight}>
              <PenSquare
                color="#fff"
                size={20}
                onPress={() => navigation.navigate('Capture')}
              />
              <Settings
                color="#fff"
                size={20}
                onPress={() => navigation.navigate('Settings')}
              />
            </View>
          ),
        })}
      >
        <Drawer.Screen
          name="Chat"
          component={ChatScreen}
          options={({ route }) => ({
            title: route.params?.title ?? '新對話',
          })}
        />
        <Drawer.Screen
          name="Knowledge"
          component={KnowledgeScreen}
          options={{ title: '知識庫' }}
        />
        <Drawer.Screen
          name="Capture"
          component={CaptureScreen}
          options={{ title: '擷取' }}
        />
        <Drawer.Screen
          name="Settings"
          component={SettingsScreen}
          options={{ title: '設定' }}
        />
      </Drawer.Navigator>
    </NavigationContainer>
  );
}

// ─── Styles ─────────────────────────────────────────────────────────────────

const styles = StyleSheet.create({
  header: {
    backgroundColor: '#0f0f11',
    borderBottomWidth: StyleSheet.hairlineWidth,
    borderBottomColor: '#2c2c2e',
    elevation: 0,
    shadowOpacity: 0,
  },
  headerTitle: {
    color: '#fff',
    fontSize: 16,
    fontWeight: '600',
  },
  drawer: {
    backgroundColor: '#1c1c1e',
    width: 280,
  },
  headerLeft: {
    flexDirection: 'row',
    alignItems: 'center',
    marginLeft: 16,
    gap: 16,
  },
  headerRight: {
    flexDirection: 'row',
    alignItems: 'center',
    marginRight: 16,
    gap: 16,
  },
});
