import React from 'react';
import { cn } from '../../lib/utils';

type MemoryType = 'data' | 'pattern' | 'log';
type BadgeVariant = 'default' | 'outline' | MemoryType;

interface BadgeProps extends React.HTMLAttributes<HTMLSpanElement> {
    children: React.ReactNode;
    variant?: BadgeVariant;
    className?: string;
}

const memorySymbols: Record<MemoryType, string> = {
    data: '●',
    pattern: '◆',
    log: '▲',
};

export function Badge({ children, variant = 'default', className, ...props }: BadgeProps) {
    const base = 'inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-xs font-medium';

    const variantClasses: Record<BadgeVariant, string> = {
        default: 'bg-[var(--ic-bg-sunken)] text-[var(--ic-text-secondary)]',
        outline: 'border border-[var(--ic-border-default)] text-[var(--ic-text-secondary)]',
        data: 'ic-badge-data',
        pattern: 'ic-badge-pattern',
        log: 'ic-badge-log',
    };

    const isMemoryType = ['data', 'pattern', 'log'].includes(variant);

    return (
        <span {...props} className={cn(base, variantClasses[variant], className)}>
            {isMemoryType && (
                <span>{memorySymbols[variant as MemoryType]}</span>
            )}
            {children}
        </span>
    );
}
