import React from 'react';
import { X } from 'lucide-react';
import { cn } from '../../lib/utils';
import { Button } from './Button';

interface DialogProps {
    open: boolean;
    onClose: () => void;
    title?: string;
    children: React.ReactNode;
    className?: string;
    size?: 'sm' | 'md' | 'lg';
}

const sizeClasses = {
    sm: 'max-w-sm',
    md: 'max-w-md',
    lg: 'max-w-lg',
};

export function Dialog({ open, onClose, title, children, className, size = 'md' }: DialogProps) {
    if (!open) return null;

    return (
        <div className="fixed inset-0 z-[var(--z-modal)] flex items-center justify-center p-4">
            {/* Overlay */}
            <div
                className="absolute inset-0 bg-black/50"
                onClick={onClose}
            />
            {/* Dialog box */}
            <div
                className={cn(
                    'relative w-full rounded-xl',
                    'bg-surface-base border border-stroke-divider',
                    'shadow-flyout',
                    'flex flex-col',
                    sizeClasses[size],
                    className
                )}
                onClick={e => e.stopPropagation()}
            >
                {/* Header */}
                {title && (
                    <div className="flex items-center justify-between px-5 py-4 border-b border-stroke-divider">
                        <h2 className="text-fs-base font-semibold text-text-primary">
                            {title}
                        </h2>
                        <Button variant="ghost" size="icon" onClick={onClose}>
                            <X className="h-4 w-4" />
                        </Button>
                    </div>
                )}
                {/* Content */}
                <div className="px-5 py-4 flex-1 overflow-y-auto">
                    {children}
                </div>
            </div>
        </div>
    );
}
