import { cva, type VariantProps } from 'class-variance-authority';
import { cn } from '../../lib/utils';

const buttonVariants = cva(
    [
        'inline-flex items-center justify-center gap-2 rounded-md text-sm font-medium',
        'transition-colors duration-100',
        'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ic-border-focus)]',
        'disabled:opacity-50 disabled:pointer-events-none',
        'select-none cursor-pointer',
    ].join(' '),
    {
        variants: {
            variant: {
                primary: [
                    'bg-[var(--ic-accent)] text-[var(--ic-text-inverse)]',
                    'hover:bg-[var(--ic-accent-hover)]',
                ].join(' '),
                secondary: [
                    'bg-[var(--ic-bg-elevated)] text-[var(--ic-text-primary)]',
                    'border border-[var(--ic-border-default)]',
                    'hover:bg-[var(--ic-bg-sunken)]',
                ].join(' '),
                ghost: [
                    'text-[var(--ic-text-secondary)]',
                    'hover:bg-[var(--ic-bg-sunken)] hover:text-[var(--ic-text-primary)]',
                ].join(' '),
                destructive: [
                    'bg-[var(--ic-danger)] text-white',
                    'hover:bg-[var(--ic-danger-hover)]',
                ].join(' '),
                link: [
                    'text-[var(--ic-accent)] underline-offset-4',
                    'hover:underline',
                ].join(' '),
            },
            size: {
                sm: 'h-8 px-3 text-xs',
                md: 'h-9 px-4',
                lg: 'h-10 px-6 text-base',
                icon: 'h-9 w-9 p-0',
            },
        },
        defaultVariants: {
            variant: 'primary',
            size: 'md',
        },
    }
);

export interface ButtonProps
    extends React.ButtonHTMLAttributes<HTMLButtonElement>,
    VariantProps<typeof buttonVariants> {
    loading?: boolean;
}

import React from 'react';
import { Loader2 } from 'lucide-react';

export function Button({
    className,
    variant,
    size,
    loading,
    disabled,
    children,
    ...props
}: ButtonProps) {
    return (
        <button
            className={cn(buttonVariants({ variant, size }), className)}
            disabled={disabled || loading}
            {...props}
        >
            {loading && <Loader2 className="h-4 w-4 animate-spin" />}
            {children}
        </button>
    );
}
