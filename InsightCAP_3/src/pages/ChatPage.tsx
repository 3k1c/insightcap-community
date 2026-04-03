import React, { useEffect, useRef, useState, useCallback } from 'react';
import { useChatStore } from '../stores/chatStore';
import { useKnowledgeStore } from '../stores/knowledgeStore';
import { MessageList } from '../components/chat/MessageList';
import { InputArea } from '../components/chat/InputArea';
import { ContextHintBanner } from '../components/memory/ContextHintBanner';
import { EditorPane } from '../components/chat/EditorPane';
import { useUiStore } from '../stores/uiStore';
import { FolderPlus, MessageSquarePlus, ChevronDown, ChevronRight, MoreVertical, PanelRight } from 'lucide-react';

const PROJECT_COLORS = ['#6366F1', '#EC4899', '#F59E0B', '#10B981', '#3B82F6', '#EF4444', '#8B5CF6'];

export const ChatPage: React.FC = () => {
    const {
        conversations,
        projects,
        activeConversationId,
        activeProjectId,
        messages,
        isGenerating,
        contextStats,
        expandedProjectIds,
        loadConversations,
        loadProjects,
        createNewConversation,
        createProject,
        updateProject,
        deleteProject,
        toggleProjectExpanded,
        loadMessages,
        sendMessage,
        renameConversation,
        deleteConversation,
        updateConversation,
        moveConversationToProject,
        reorderProjects,
    } = useChatStore();

    const { loadSources } = useKnowledgeStore();
    const { isEditorOpen, toggleEditor, isSidebarOpen } = useUiStore();

    // Resizable split: chatPct is the % width of the chat column (editor gets the rest)
    const [chatPct, setChatPct] = useState(45);
    const containerRef = useRef<HTMLDivElement>(null);
    const isDraggingRef = useRef(false);

    const onDividerMouseDown = useCallback((e: React.MouseEvent) => {
        e.preventDefault();
        isDraggingRef.current = true;
        const onMove = (ev: MouseEvent) => {
            if (!isDraggingRef.current || !containerRef.current) return;
            const rect = containerRef.current.getBoundingClientRect();
            const pct = ((ev.clientX - rect.left) / rect.width) * 100;
            setChatPct(Math.min(Math.max(pct, 25), 75));
        };
        const onUp = () => {
            isDraggingRef.current = false;
            window.removeEventListener('mousemove', onMove);
            window.removeEventListener('mouseup', onUp);
        };
        window.addEventListener('mousemove', onMove);
        window.addEventListener('mouseup', onUp);
    }, []);

    // ── Local UI state ──
    const [showProjectMenu, setShowProjectMenu] = useState<string | null>(null);
    const [showConvMenu, setShowConvMenu] = useState<string | null>(null);
    const [showColorPicker, setShowColorPicker] = useState<string | null>(null);
    const [renamingProjectId, setRenamingProjectId] = useState<string | null>(null);
    const [renamingConvId, setRenamingConvId] = useState<string | null>(null);
    const [renameValue, setRenameValue] = useState('');
    // ── DnD：拖曳對話到 Project ──
    const [draggingConvId, setDraggingConvId] = useState<string | null>(null);
    const [dragOverProjectId, setDragOverProjectId] = useState<string | null>(null);
    const projectRowRefs = useRef<Record<string, HTMLDivElement | null>>({});
    const convGhostRef = useRef<HTMLDivElement | null>(null);

    // ── DnD：拖曳 Project 排序 ──
    const [draggingProjectId, setDraggingProjectId] = useState<string | null>(null);
    const [dragOverProjectIndex, setDragOverProjectIndex] = useState<number | null>(null);
    const projectGhostRef = useRef<HTMLDivElement | null>(null);

    // ── DnD：全域 guard，確保同時只有一個 DnD ──
    const isDndActiveRef = useRef(false);

    const startConvDrag = useCallback((e: React.PointerEvent, convId: string, convTitle: string) => {
        if (isDndActiveRef.current) return;
        const startX = e.clientX;
        const startY = e.clientY;
        let dragging = false;
        let ghost: HTMLDivElement | null = null;

        const activate = (cx: number, cy: number) => {
            isDndActiveRef.current = true;
            dragging = true;
            setDraggingConvId(convId);
            ghost = document.createElement('div');
            ghost.style.cssText = 'position:fixed;pointer-events:none;z-index:9999;max-width:200px;padding:6px 12px;border-radius:8px;background:transparent;border:none;font-size:0.8125rem;color:var(--text-secondary);white-space:nowrap;overflow:hidden;text-overflow:ellipsis;opacity:0.5;transform:translate(-50%,-50%)';
            ghost.textContent = convTitle;
            document.body.appendChild(ghost);
            convGhostRef.current = ghost;
            ghost.style.left = `${cx}px`;
            ghost.style.top = `${cy}px`;
        };

        const updateTarget = (cx: number, cy: number) => {
            if (ghost) { ghost.style.left = `${cx}px`; ghost.style.top = `${cy}px`; }
            let found: string | null = null;
            for (const [pid, el] of Object.entries(projectRowRefs.current)) {
                if (!el) continue;
                const r = el.getBoundingClientRect();
                if (cx >= r.left && cx <= r.right && cy >= r.top && cy <= r.bottom) { found = pid; break; }
            }
            setDragOverProjectId(found);
        };

        const move = (me: PointerEvent) => {
            if (!dragging) {
                if (Math.abs(me.clientX - startX) > 4 || Math.abs(me.clientY - startY) > 4) {
                    activate(me.clientX, me.clientY);
                }
                return;
            }
            updateTarget(me.clientX, me.clientY);
        };

        const up = async (ue: PointerEvent) => {
            document.removeEventListener('pointermove', move);
            document.removeEventListener('pointerup', up);
            isDndActiveRef.current = false;
            if (ghost) { ghost.remove(); convGhostRef.current = null; }

            if (!dragging) return;

            let targetProjectId: string | null = null;
            for (const [pid, el] of Object.entries(projectRowRefs.current)) {
                if (!el) continue;
                const r = el.getBoundingClientRect();
                if (ue.clientX >= r.left && ue.clientX <= r.right && ue.clientY >= r.top && ue.clientY <= r.bottom) {
                    targetProjectId = pid;
                    break;
                }
            }

            setDragOverProjectId(null);
            setDraggingConvId(null);

            if (targetProjectId) {
                const conv = useChatStore.getState().conversations.find(c => c.id === convId);
                if (conv && conv.projectId !== targetProjectId) {
                    await moveConversationToProject(convId, targetProjectId);
                    if (!useChatStore.getState().expandedProjectIds.has(targetProjectId)) {
                        toggleProjectExpanded(targetProjectId);
                    }
                }
            }
        };

        document.addEventListener('pointermove', move);
        document.addEventListener('pointerup', up);
    }, [moveConversationToProject, toggleProjectExpanded]);

    const startProjectDrag = useCallback((e: React.PointerEvent, projectId: string, projectName: string) => {
        if (isDndActiveRef.current) return;
        const startX = e.clientX;
        const startY = e.clientY;
        let dragging = false;
        let ghost: HTMLDivElement | null = null;

        const activate = (cx: number, cy: number) => {
            isDndActiveRef.current = true;
            dragging = true;
            setDraggingProjectId(projectId);
            ghost = document.createElement('div');
            ghost.style.cssText = 'position:fixed;pointer-events:none;z-index:9999;max-width:200px;padding:6px 12px;border-radius:8px;background:transparent;border:none;font-size:0.8125rem;color:var(--text-secondary);white-space:nowrap;overflow:hidden;text-overflow:ellipsis;opacity:0.5;transform:translate(-50%,-50%)';
            ghost.textContent = projectName;
            document.body.appendChild(ghost);
            projectGhostRef.current = ghost;
            ghost.style.left = `${cx}px`;
            ghost.style.top = `${cy}px`;
        };

        const updateIndex = (cy: number) => {
            const currentProjects = useChatStore.getState().projects;
            let foundIdx: number | null = null;
            for (let i = 0; i < currentProjects.length; i++) {
                const el = projectRowRefs.current[currentProjects[i].id];
                if (!el) continue;
                const r = el.getBoundingClientRect();
                if (cy <= r.top + r.height / 2) { foundIdx = i; break; }
                if (i === currentProjects.length - 1) foundIdx = i + 1;
            }
            setDragOverProjectIndex(foundIdx);
        };

        const move = (me: PointerEvent) => {
            if (!dragging) {
                if (Math.abs(me.clientX - startX) > 4 || Math.abs(me.clientY - startY) > 4) {
                    activate(me.clientX, me.clientY);
                }
                return;
            }
            if (ghost) { ghost.style.left = `${me.clientX}px`; ghost.style.top = `${me.clientY}px`; }
            updateIndex(me.clientY);
        };

        const up = async (ue: PointerEvent) => {
            document.removeEventListener('pointermove', move);
            document.removeEventListener('pointerup', up);
            isDndActiveRef.current = false;
            if (ghost) { ghost.remove(); projectGhostRef.current = null; }

            if (!dragging) return;

            const currentProjects = useChatStore.getState().projects;
            let insertIdx: number | null = null;
            for (let i = 0; i < currentProjects.length; i++) {
                const el = projectRowRefs.current[currentProjects[i].id];
                if (!el) continue;
                const r = el.getBoundingClientRect();
                if (ue.clientY <= r.top + r.height / 2) { insertIdx = i; break; }
                if (i === currentProjects.length - 1) insertIdx = i + 1;
            }

            setDragOverProjectIndex(null);
            setDraggingProjectId(null);

            if (insertIdx !== null) {
                const oldIdx = currentProjects.findIndex(p => p.id === projectId);
                if (oldIdx !== -1 && insertIdx !== oldIdx && insertIdx !== oldIdx + 1) {
                    const newOrder = [...currentProjects.map(p => p.id)];
                    newOrder.splice(oldIdx, 1);
                    const adjustedIdx = insertIdx > oldIdx ? insertIdx - 1 : insertIdx;
                    newOrder.splice(adjustedIdx, 0, projectId);
                    await reorderProjects(newOrder);
                }
            }
        };

        document.addEventListener('pointermove', move);
        document.addEventListener('pointerup', up);
    }, [reorderProjects]);


    useEffect(() => {
        loadConversations();
        loadProjects();
        loadSources();
    }, [loadConversations, loadProjects, loadSources]);

    const handleSendMessage = async (content: string, opts?: {
        ragEnabled: boolean;
        webEnabled: boolean;
        mentionedSourceIds: string[];
        mentionedSources: { id: string; title: string }[];
        mentionedTagNames: string[];
        attachedFiles: { name: string; filePath: string; fileType: string; previewUrl?: string; tempChunkIds?: string[] }[];
        tempChunkIds?: string[];
    }) => {
        if (!activeConversationId) {
            await createNewConversation(activeProjectId || undefined);
        }
        await sendMessage(content, opts);
    };

    const handleAddProject = async () => {
        try {
            // 產生不重複的預設名稱
            const existingNames = useChatStore.getState().projects.map(p => p.name);
            let baseName = '新項目';
            let candidateName = baseName;
            let suffix = 1;
            while (existingNames.includes(candidateName)) {
                candidateName = `${baseName} ${suffix++}`;
            }
            await createProject(candidateName);
            const latest = useChatStore.getState().projects[0];
            if (latest) {
                setRenamingProjectId(latest.id);
                setRenameValue(latest.name);
            }
        } catch (error) {
            console.error('[ChatPage] Error creating project:', error);
        }
    };

    const handleRenameCommit = async (projectId: string) => {
        const trimmed = renameValue.trim();
        if (trimmed) {
            const duplicate = useChatStore.getState().projects.some(
                p => p.name === trimmed && p.id !== projectId
            );
            if (duplicate) {
                // 名稱重複，恢復原名不儲存
                setRenamingProjectId(null);
                setRenameValue('');
                return;
            }
            await useChatStore.getState().updateProject(projectId, trimmed);
        }
        setRenamingProjectId(null);
        setRenameValue('');
    };

    const handleConvRenameCommit = async (convId: string) => {
        const trimmed = renameValue.trim();
        if (trimmed) {
            await renameConversation(convId, trimmed);
        }
        setRenamingConvId(null);
        setRenameValue('');
    };

    const handleNewConversation = async (projectId?: string) => {
        const newId = await createNewConversation(projectId);
        if (newId) {
            if (projectId && !useChatStore.getState().expandedProjectIds.has(projectId)) {
                toggleProjectExpanded(projectId);
            }
            setRenamingConvId(newId);
            setRenameValue('新對話');
        }
    };

    // 對話排序：置頂優先，同層再依 updatedAt DESC
    const byPinnedThenDate = (a: typeof conversations[0], b: typeof conversations[0]) => {
        if (a.isPinned !== b.isPinned) return a.isPinned ? -1 : 1;
        return b.updatedAt.localeCompare(a.updatedAt);
    };

    // 無 project 的對話（對話列表，置頂優先）
    const unprojectConversations = conversations
        .filter(c => !c.projectId)
        .sort(byPinnedThenDate);

    // 按 project 分組的對話（各組內部也按日期排序）
    // 項目列表排序由後端 sort_order 決定，前端不重排 projects[]
    const conversationsByProject = new Map<string, typeof conversations>();
    conversations.forEach(c => {
        if (c.projectId) {
            if (!conversationsByProject.has(c.projectId)) {
                conversationsByProject.set(c.projectId, []);
            }
            conversationsByProject.get(c.projectId)!.push(c);
        }
    });
    conversationsByProject.forEach((convs, key) => {
        conversationsByProject.set(key, [...convs].sort(byPinnedThenDate));
    });

    const noConversation = !activeConversationId && conversations.length === 0;

    return (
        <div className="flex h-full w-full bg-surface-base text-text-primary overflow-hidden">

            {/* ── 側邊欄：項目與對話 ── */}
            <div className={`${isSidebarOpen ? 'w-64' : 'w-0'} border-r border-stroke-divider bg-surface-layer flex flex-col shrink-0 overflow-hidden transition-all duration-200`}>

                {/* 頂部：標題 + 新項目 + 新對話 */}
                <div className="px-3 pt-4 pb-2 flex items-center justify-between shrink-0">
                    <span className="text-fs-xs font-semibold text-text-tertiary uppercase tracking-wider">工作區</span>
                    <div className="flex items-center gap-0.5">
                        <button
                            onClick={handleAddProject}
                            className="p-1.5 hover:bg-surface-subtle rounded text-text-secondary transition-colors"
                            title="新增項目"
                        >
                            <FolderPlus className="w-4 h-4" />
                        </button>
                        <button
                            onClick={() => handleNewConversation()}
                            className="p-1.5 hover:bg-surface-subtle rounded text-text-secondary transition-colors"
                            title="新增對話"
                        >
                            <MessageSquarePlus className="w-4 h-4" />
                        </button>
                    </div>
                </div>

                {/* 捲動區域 */}
                <div className="flex-1 overflow-y-auto px-2 pb-2 space-y-0.5" onClick={() => { setShowProjectMenu(null); setShowConvMenu(null); }}>

                    {/* ── 項目列表（一級） ── */}
                    {projects.map((project, projectIndex) => {
                        const convs = conversationsByProject.get(project.id) || [];
                        const isExpanded = expandedProjectIds.has(project.id);
                        const isActiveProject = activeProjectId === project.id;
                        const isDragTarget = dragOverProjectId === project.id && draggingConvId !== null;
                        const showInsertBefore = draggingProjectId !== null && dragOverProjectIndex === projectIndex && draggingProjectId !== project.id;
                        const showInsertAfter = draggingProjectId !== null && dragOverProjectIndex === projectIndex + 1 && draggingProjectId !== projects[projectIndex]?.id;

                        return (
                            <div key={project.id} ref={el => { projectRowRefs.current[project.id] = el; }}>
                                {/* 拖曳 Project 排序：插入線（前） */}
                                {showInsertBefore && (
                                    <div className="h-0.5 bg-accent-default rounded mx-2 my-0.5" />
                                )}
                                {/* Project 行 */}
                                <div
                                    className={`flex items-center gap-2.5 px-2 py-2.5 rounded-lg cursor-pointer transition-colors group ${isDragTarget
                                        ? 'bg-accent-light2 ring-1 ring-accent-default'
                                        : isActiveProject
                                            ? 'bg-accent-light2'
                                            : 'hover:bg-surface-subtle'
                                        } ${draggingProjectId === project.id ? 'opacity-40' : ''}`}
                                    onPointerDown={(e) => {
                                        if (renamingProjectId === project.id) return;
                                        startProjectDrag(e, project.id, project.name);
                                    }}
                                    onDoubleClick={(e) => {
                                        e.stopPropagation();
                                        if (renamingProjectId === project.id) return;
                                        setRenamingProjectId(project.id);
                                        setRenameValue(project.name);
                                    }}
                                    onClick={(e) => {
                                        // 雙擊時瀏覽器會先觸發兩次 onClick，用 detail 區分
                                        if (e.detail >= 2) return;
                                        if (convs.length > 0) toggleProjectExpanded(project.id);
                                        if (!isActiveProject) {
                                            useChatStore.setState({ activeProjectId: project.id, activeConversationId: null, messages: [] });
                                        }
                                    }}
                                >
                                    {/* 顏色圓點 */}
                                    <div className="w-2.5 h-2.5 rounded-full shrink-0"
                                        style={{ backgroundColor: project.color || '#6366F1' }} />
                                    {/* 名稱 / inline 改名 */}
                                    {renamingProjectId === project.id ? (
                                        <input
                                            autoFocus
                                            className="flex-1 text-fs-sm font-medium bg-transparent border-none rounded px-1 py-0 outline-none focus:outline-none focus:ring-0 text-text-primary min-w-0"
                                            value={renameValue}
                                            onChange={(e) => setRenameValue(e.target.value)}
                                            onClick={(e) => e.stopPropagation()}
                                            onDoubleClick={(e) => e.stopPropagation()}
                                            onBlur={() => handleRenameCommit(project.id)}
                                            onKeyDown={(e) => {
                                                if (e.key === 'Enter') handleRenameCommit(project.id);
                                                if (e.key === 'Escape') { setRenamingProjectId(null); setRenameValue(''); }
                                            }}
                                        />
                                    ) : (
                                        <span className={`flex-1 text-fs-sm font-medium truncate ${isActiveProject ? 'text-accent-default' : 'text-text-primary'}`}>
                                            {project.name}
                                        </span>
                                    )}
                                    {/* 右側：⋮（hover）/ 箭頭指示（非 hover，純展示） */}
                                    {renamingProjectId !== project.id && (
                                        <div className="relative shrink-0 w-3 h-3 flex items-center justify-center">
                                            {/* 箭頭：純指示，無互動 */}
                                            <div className="group-hover:opacity-0 transition-opacity absolute text-text-tertiary pointer-events-none">
                                                {convs.length > 0 && (isExpanded
                                                    ? <ChevronDown className="w-3 h-3" />
                                                    : <ChevronRight className="w-3 h-3" />)}
                                            </div>
                                            {/* ⋮ hover 才顯示，選單錨點在此 */}
                                            <div
                                                onClick={(e) => {
                                                    e.stopPropagation();
                                                    setShowProjectMenu(showProjectMenu === project.id ? null : project.id);
                                                }}
                                                className="opacity-0 group-hover:opacity-100 transition-opacity absolute cursor-pointer"
                                            >
                                                <MoreVertical className="w-3 h-3 text-text-secondary" />
                                            </div>
                                            {/* Project 快捷選單，錨定在 ⋮ 容器右下角 */}
                                            {showProjectMenu === project.id && (
                                                <div className="absolute right-0 top-4 bg-surface-base border border-stroke-divider rounded-lg shadow-2xl z-50 w-28 py-1 px-1 animate-in fade-in zoom-in duration-150"
                                                    onClick={(e) => e.stopPropagation()}
                                                >
                                                    <button onClick={(e) => { e.stopPropagation(); handleNewConversation(project.id); setShowProjectMenu(null); }}
                                                        className="flex w-full px-3 py-1.5 text-fs-sm text-text-secondary hover:bg-surface-subtle text-left transition-colors">
                                                        新增對話
                                                    </button>
                                                    <button onClick={(e) => { e.stopPropagation(); updateProject(project.id, undefined, undefined, !project.isPinned); setShowProjectMenu(null); }}
                                                        className="flex w-full px-3 py-1.5 text-fs-sm text-text-secondary hover:bg-surface-subtle text-left transition-colors">
                                                        {project.isPinned ? '取消置頂' : '置頂'}
                                                    </button>
                                                    <button onClick={(e) => { e.stopPropagation(); setShowColorPicker(showColorPicker === project.id ? null : project.id); }}
                                                        className="flex w-full px-3 py-1.5 text-fs-sm text-text-secondary hover:bg-surface-subtle text-left transition-colors">
                                                        顏色
                                                    </button>
                                                    {showColorPicker === project.id && (
                                                        <div className="flex flex-wrap gap-1.5 px-3 py-2">
                                                            {PROJECT_COLORS.map(c => (
                                                                <div key={c}
                                                                    onClick={(e) => { e.stopPropagation(); updateProject(project.id, undefined, c); setShowColorPicker(null); setShowProjectMenu(null); }}
                                                                    className="w-4 h-4 rounded-full cursor-pointer ring-offset-1 hover:ring-2 hover:ring-stroke-divider transition-all"
                                                                    style={{ backgroundColor: c }}
                                                                />
                                                            ))}
                                                        </div>
                                                    )}
                                                    <button onClick={(e) => { e.stopPropagation(); deleteProject(project.id); setShowProjectMenu(null); }}
                                                        className="flex w-full px-3 py-1.5 text-fs-sm text-red-400 hover:bg-surface-subtle text-left transition-colors">
                                                        刪除
                                                    </button>
                                                </div>
                                            )}
                                        </div>
                                    )}
                                </div>

                                {/* 拖曳 Project 排序：插入線（後） */}
                                {showInsertAfter && (
                                    <div className="h-0.5 bg-accent-default rounded mx-2 my-0.5" />
                                )}

                                {/* 對話列表（二級，縮排） */}
                                {isExpanded && (
                                    <div className="ml-5 mt-0.5 mb-1 space-y-0.5">
                                        {convs.length === 0 ? (
                                            <div className="px-3 py-1.5 text-fs-sm text-text-tertiary italic">尚無對話</div>
                                        ) : (
                                            convs.map(conv => (
                                                <div key={conv.id} className={`group flex items-center px-3 py-1.5 text-fs-sm rounded-md cursor-pointer transition-colors ${activeConversationId === conv.id
                                                    ? 'bg-accent-light2 text-accent-default font-medium'
                                                    : 'hover:bg-surface-subtle text-text-secondary'
                                                    }`}
                                                    onClick={() => loadMessages(conv.id)}
                                                    onPointerDown={(e) => {
                                                        e.stopPropagation();
                                                        if (renamingConvId === conv.id) return;
                                                        startConvDrag(e, conv.id, conv.title);
                                                    }}
                                                >
                                                    {renamingConvId === conv.id ? (
                                                        <input
                                                            autoFocus
                                                            className="flex-1 bg-transparent border-none rounded px-1 py-0 outline-none focus:outline-none focus:ring-0 text-text-primary text-fs-sm min-w-0"
                                                            value={renameValue}
                                                            onChange={(e) => setRenameValue(e.target.value)}
                                                            onClick={(e) => e.stopPropagation()}
                                                            onBlur={() => handleConvRenameCommit(conv.id)}
                                                            onKeyDown={(e) => {
                                                                if (e.key === 'Enter') handleConvRenameCommit(conv.id);
                                                                if (e.key === 'Escape') { setRenamingConvId(null); setRenameValue(''); }
                                                            }}
                                                        />
                                                    ) : (
                                                        <span
                                                            className="flex-1 truncate"
                                                            title={conv.title}
                                                            onDoubleClick={(e) => { e.stopPropagation(); setRenamingConvId(conv.id); setRenameValue(conv.title); }}
                                                        >
                                                            {conv.title}
                                                        </span>
                                                    )}
                                                    {renamingConvId !== conv.id && (
                                                        <div className="relative shrink-0 ml-1">
                                                            <button
                                                                onClick={(e) => { e.stopPropagation(); setShowConvMenu(showConvMenu === conv.id ? null : conv.id); }}
                                                                className="opacity-0 group-hover:opacity-100 p-0.5 text-text-tertiary hover:text-text-secondary rounded transition-all"
                                                            >
                                                                <MoreVertical className="w-3 h-3" />
                                                            </button>
                                                            {showConvMenu === conv.id && (
                                                                <div className="absolute right-0 top-5 bg-surface-base border border-stroke-divider rounded-lg shadow-2xl z-50 min-w-[100px] py-1 px-1 animate-in fade-in zoom-in duration-150">
                                                                    <button onClick={(e) => { e.stopPropagation(); updateConversation(conv.id, !conv.isPinned); setShowConvMenu(null); }}
                                                                        className="flex w-full px-3 py-1.5 text-fs-sm text-text-secondary hover:bg-surface-subtle text-left items-center gap-2 transition-colors">
                                                                        {conv.isPinned ? '取消置頂' : '置頂'}
                                                                    </button>
                                                                    <button onClick={(e) => { e.stopPropagation(); updateConversation(conv.id, undefined, !conv.isLocked); setShowConvMenu(null); }}
                                                                        className="flex w-full px-3 py-1.5 text-fs-sm text-text-secondary hover:bg-surface-subtle text-left items-center gap-2 transition-colors">
                                                                        {conv.isLocked ? '解除鎖定' : '鎖定'}
                                                                    </button>
                                                                    <button onClick={(e) => { e.stopPropagation(); if (!conv.isLocked) deleteConversation(conv.id); setShowConvMenu(null); }}
                                                                        className={`flex w-full px-3 py-1.5 text-fs-sm text-left items-center gap-2 transition-colors ${conv.isLocked ? 'text-text-tertiary cursor-not-allowed' : 'text-red-400 hover:bg-surface-subtle'}`}>
                                                                        刪除
                                                                    </button>
                                                                </div>
                                                            )}
                                                        </div>
                                                    )}
                                                </div>
                                            ))
                                        )}
                                    </div>
                                )}
                            </div>
                        );
                    })}

                    {/* ── 未分類對話（無項目，直接平鋪） ── */}
                    {unprojectConversations.length > 0 && (
                        <div className={projects.length > 0 ? "mt-2 pt-2 border-t border-stroke-divider" : ""}>
                            {unprojectConversations.map(conv => (
                                <div key={conv.id}
                                    className={`group flex items-center px-3 py-1.5 text-fs-sm rounded-md cursor-pointer transition-colors ${activeConversationId === conv.id
                                        ? 'bg-accent-light2 text-accent-default font-medium'
                                        : 'hover:bg-surface-subtle text-text-secondary'
                                        }`}
                                    onClick={() => { useChatStore.setState({ activeProjectId: null }); loadMessages(conv.id); }}
                                    onPointerDown={(e) => {
                                        e.stopPropagation();
                                        if (renamingConvId === conv.id) return;
                                        startConvDrag(e, conv.id, conv.title);
                                    }}
                                >
                                    {renamingConvId === conv.id ? (
                                        <input
                                            autoFocus
                                            className="flex-1 bg-transparent border-none rounded px-1 py-0 outline-none focus:outline-none focus:ring-0 text-text-primary text-fs-sm min-w-0"
                                            value={renameValue}
                                            onChange={(e) => setRenameValue(e.target.value)}
                                            onClick={(e) => e.stopPropagation()}
                                            onBlur={() => handleConvRenameCommit(conv.id)}
                                            onKeyDown={(e) => {
                                                if (e.key === 'Enter') handleConvRenameCommit(conv.id);
                                                if (e.key === 'Escape') { setRenamingConvId(null); setRenameValue(''); }
                                            }}
                                        />
                                    ) : (
                                        <span
                                            className="flex-1 truncate"
                                            title={conv.title}
                                            onDoubleClick={(e) => { e.stopPropagation(); setRenamingConvId(conv.id); setRenameValue(conv.title); }}
                                        >
                                            {conv.title}
                                        </span>
                                    )}
                                    {renamingConvId !== conv.id && (
                                        <div className="relative shrink-0 ml-1">
                                            <button
                                                onClick={(e) => { e.stopPropagation(); setShowConvMenu(showConvMenu === conv.id ? null : conv.id); }}
                                                className="opacity-0 group-hover:opacity-100 p-0.5 text-text-tertiary hover:text-text-secondary rounded transition-all"
                                            >
                                                <MoreVertical className="w-3 h-3" />
                                            </button>
                                            {showConvMenu === conv.id && (
                                                <div className="absolute right-0 top-5 bg-surface-base border border-stroke-divider rounded-lg shadow-2xl z-50 min-w-[100px] py-1 px-1 animate-in fade-in zoom-in duration-150">
                                                    <button onClick={(e) => { e.stopPropagation(); updateConversation(conv.id, !conv.isPinned); setShowConvMenu(null); }}
                                                        className="flex w-full px-3 py-1.5 text-fs-sm text-text-secondary hover:bg-surface-subtle text-left items-center gap-2 transition-colors">
                                                        {conv.isPinned ? '取消置頂' : '置頂'}
                                                    </button>
                                                    <button onClick={(e) => { e.stopPropagation(); updateConversation(conv.id, undefined, !conv.isLocked); setShowConvMenu(null); }}
                                                        className="flex w-full px-3 py-1.5 text-fs-sm text-text-secondary hover:bg-surface-subtle text-left items-center gap-2 transition-colors">
                                                        {conv.isLocked ? '解除鎖定' : '鎖定'}
                                                    </button>
                                                    <button onClick={(e) => { e.stopPropagation(); if (!conv.isLocked) deleteConversation(conv.id); setShowConvMenu(null); }}
                                                        className={`flex w-full px-3 py-1.5 text-fs-sm text-left items-center gap-2 transition-colors ${conv.isLocked ? 'text-text-tertiary cursor-not-allowed' : 'text-red-400 hover:bg-surface-subtle'}`}>
                                                        刪除
                                                    </button>
                                                </div>
                                            )}
                                        </div>
                                    )}
                                </div>
                            ))}
                        </div>
                    )}

                    {conversations.length === 0 && projects.length === 0 && (
                        <div className="text-fs-sm text-text-tertiary p-4 text-center">尚無對話記錄</div>
                    )}
                </div>
            </div>

            {/* ── 主體區域 ── */}
            <div ref={containerRef} className="flex flex-1 h-full overflow-hidden">

                {/* ── 對話主區 ── */}
                <div
                    className="flex flex-col h-full overflow-hidden shrink-0"
                    style={{ width: isEditorOpen ? `${chatPct}%` : '100%' }}
                >
                    {/* 頂部標題列 */}
                    <div className="h-12 border-b border-stroke-divider bg-surface-layer flex items-center justify-between px-4 shrink-0">
                        <div className="flex items-center gap-2 min-w-0 pr-4">
                            {activeProjectId && (
                                <>
                                    <div
                                        className="w-2 h-2 rounded-full shrink-0"
                                        style={{ backgroundColor: projects.find(p => p.id === activeProjectId)?.color || '#6366F1' }}
                                    />
                                    <span className="text-fs-sm text-text-tertiary shrink-0 font-medium">
                                        {projects.find(p => p.id === activeProjectId)?.name}
                                    </span>
                                    <span className="text-text-tertiary shrink-0">/</span>
                                </>
                            )}
                            <span className="font-semibold text-fs-sm truncate text-text-secondary">
                                {conversations.find(c => c.id === activeConversationId)?.title || 'InsightCAP 助理'}
                            </span>
                        </div>
                        <div className="flex items-center gap-1">
                            <button
                                onClick={() => toggleEditor()}
                                className={`p-1.5 rounded transition-colors ${isEditorOpen ? 'bg-accent-light2 text-accent-default' : 'text-text-secondary hover:bg-surface-subtle hover:text-text-primary'}`}
                                title={isEditorOpen ? "關閉編輯器" : "開啟編輯器"}
                            >
                                <PanelRight className="w-4 h-4" />
                            </button>
                        </div>
                    </div>

                    {noConversation ? (
                        <div className="flex-1 flex flex-col items-center justify-center">
                            <h1 className="text-fs-2xl font-bold mb-4">InsightCAP 即時對話</h1>
                            <p className="text-text-tertiary mb-8">開始詢問關於您的經驗與知識的問題</p>
                            <button
                                onClick={() => handleNewConversation()}
                                className="px-6 py-2 bg-accent-default text-white rounded-lg hover:bg-accent-light1 transition-colors shadow-sm"
                            >
                                新增對話
                            </button>
                        </div>
                    ) : (
                        <>
                            <ContextHintBanner stats={contextStats} isInjecting={isGenerating} />
                            <MessageList messages={messages} isGenerating={isGenerating} />
                            <InputArea onSendMessage={handleSendMessage} isGenerating={isGenerating} />
                        </>
                    )}
                </div>

                {/* ── 拖曳分隔線 ── */}
                {isEditorOpen && (
                    <div
                        onMouseDown={onDividerMouseDown}
                        className="w-3 h-full cursor-col-resize shrink-0 select-none flex items-center justify-center group"
                    >
                        <div className="w-1 h-8 rounded-full bg-stroke-divider group-hover:bg-accent-default transition-colors" />
                    </div>
                )}

                {/* ── 右側編輯器 ── */}
                {isEditorOpen && (
                    <div className="flex-1 h-full">
                        <EditorPane />
                    </div>
                )}
            </div>
        </div>
    );
};
