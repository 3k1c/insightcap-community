import React from 'react';
import { cn } from '../../lib/utils';

interface CardProps {
    children: React.ReactNode;
    className?: string;
    onClick?: () => void;
    hoverable?: boolean;
}

export function Card({ children, className, onClick, hoverable }: CardProps) {
    return (
        <div
            className={cn(
                'rounded-lg border border-stroke-divider',
                'bg-surface-card',
                'transition-colors duration-100',
                hoverable && 'cursor-pointer hover:border-stroke-control hover:bg-surface-layer',
                className
            )}
            onClick={onClick}
        >
            {children}
        </div>
    );
}

interface CardHeaderProps {
    children: React.ReactNode;
    className?: string;
}

export function CardHeader({ children, className }: CardHeaderProps) {
    return (
        <div className={cn('px-4 py-3 border-b border-stroke-divider', className)}>
            {children}
        </div>
    );
}

interface CardContentProps {
    children: React.ReactNode;
    className?: string;
}

export function CardContent({ children, className }: CardContentProps) {
    return (
        <div className={cn('px-4 py-3', className)}>
            {children}
        </div>
    );
}
