import React, { useState, useRef } from 'react';
import { NodeViewWrapper, NodeViewProps } from '@tiptap/react';

const ImageNodeView: React.FC<NodeViewProps> = ({ node, updateAttributes, selected, editor }) => {
    const { src, width, textAlign } = node.attrs;
    const [isResizing, setIsResizing] = useState(false);
    const [dragWidth, setDragWidth] = useState<number | null>(null);
    const [dragging, setDragging] = useState(false); // 拖拉狀態
    const containerRef = useRef<HTMLDivElement>(null);

    const handleResizeStart = (e: React.MouseEvent, direction: 'left' | 'right') => {
        e.preventDefault();
        e.stopPropagation();
        setIsResizing(true);
        setDragging(true);

        const startX = e.pageX;
        const startWidth = containerRef.current?.offsetWidth || 0;
        let currentNewWidth = startWidth;

        const onMouseMove = (moveEvent: MouseEvent) => {
            if (!containerRef.current) return;
            const currentX = moveEvent.pageX;
            const diffX = currentX - startX;

            let deltaWidth = 0;
            // 根據不同對齊方式計算實際寬度增量，確保滑鼠與手把完美跟隨
            if (textAlign === 'center') {
                deltaWidth = direction === 'right' ? diffX * 2 : -diffX * 2;
            } else if (textAlign === 'left') {
                deltaWidth = direction === 'right' ? diffX : -diffX;
            } else if (textAlign === 'right') {
                deltaWidth = direction === 'left' ? -diffX : diffX;
            } else {
                deltaWidth = direction === 'right' ? diffX * 2 : -diffX * 2;
            }

            const maxWidth = editor.view.dom.clientWidth || 1000;
            currentNewWidth = Math.max(100, Math.min(maxWidth, startWidth + deltaWidth));
            setDragWidth(currentNewWidth);
        };

        const onMouseUp = () => {
            document.removeEventListener('mousemove', onMouseMove);
            document.removeEventListener('mouseup', onMouseUp);
            // 只有在放開滑鼠時才寫入 ProseMirror/Tiptap 的資料，徹底消除拖拽卡頓
            updateAttributes({ width: `${currentNewWidth}px` });
            setDragWidth(null);
            setIsResizing(false);
            setDragging(false);
        };

        document.addEventListener('mousemove', onMouseMove);
        document.addEventListener('mouseup', onMouseUp);
    };

    const alignmentClass =
        textAlign === 'left' ? 'ml-0 mr-auto' :
            textAlign === 'right' ? 'ml-auto mr-0' :
                'mx-auto';

    const displayWidth = dragWidth ? `${dragWidth}px` : width;

    return (
        <NodeViewWrapper className={`tiptap-image-wrapper relative group ${alignmentClass}`} style={{ width: displayWidth, lineHeight: 0 }}>
            {/* Notion-style Resize Handles (Outside of Image Container, relative to Wrapper) */}
            {selected && (
                <>
                    <div
                        className="absolute -left-3 top-1/2 -translate-y-1/2 w-1.5 h-12 bg-text-secondary/50 border border-stroke-divider hover:bg-accent-default hover:scale-110 rounded-full shadow-sm cursor-col-resize transition-all z-30"
                        onMouseDown={(e) => handleResizeStart(e, 'left')}
                        title="拖曳以縮放"
                    />
                    <div
                        className="absolute -right-3 top-1/2 -translate-y-1/2 w-1.5 h-12 bg-text-secondary/50 border border-stroke-divider hover:bg-accent-default hover:scale-110 rounded-full shadow-sm cursor-col-resize transition-all z-30"
                        onMouseDown={(e) => handleResizeStart(e, 'right')}
                        title="拖曳以縮放"
                    />
                </>
            )}

                        <div
                                ref={containerRef}
                                className={
                                    `relative w-full ` +
                                    (selected && !dragging ? 'scale-[1.01] transition-all duration-300 ring-2 ring-accent-default shadow-lg' : '') +
                                    (selected && dragging ? ' transition-none ring-2 ring-accent-default shadow-lg' : '') +
                                    (!selected ? 'hover:shadow-md' : '')
                                }
                        >
                <img
                    src={src}
                    alt="Uploaded"
                    className="w-full h-auto block select-none pointer-events-none bg-surface-base"
                />

                {/* Resizing overlay */}
                {isResizing && (
                    <div className="absolute inset-0 bg-accent-default/5 flex items-center justify-center pointer-events-none z-20">
                        <span className="bg-surface-base px-2 py-1 rounded text-fs-xs font-bold shadow-sm border border-stroke-divider">
                            {displayWidth}
                        </span>
                    </div>
                )}
            </div>
        </NodeViewWrapper>
    );
};

export default ImageNodeView;
