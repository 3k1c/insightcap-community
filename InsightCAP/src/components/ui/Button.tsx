import { cva, type VariantProps } from 'class-variance-authority';
import { cn } from '../../lib/utils';

const buttonVariants = cva(
    [
        'inline-flex items-center justify-center gap-2 rounded-md text-fs-sm font-medium',
        'transition-colors duration-100',
        'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-stroke-focus',
        'disabled:opacity-50 disabled:pointer-events-none',
        'select-none cursor-pointer',
    ].join(' '),
    {
        variants: {
            variant: {
                primary: [
                    'bg-accent-default text-on-accent',
                    'hover:bg-accent-light1',
                ].join(' '),
                secondary: [
                    'bg-surface-card text-text-primary',
                    'border border-stroke-divider',
                    'hover:bg-surface-subtle',
                ].join(' '),
                ghost: [
                    'text-text-secondary',
                    'hover:bg-surface-subtle hover:text-text-primary',
                ].join(' '),
                destructive: [
                    'bg-color-danger text-white',
                    'hover:bg-color-danger-hover',
                ].join(' '),
                link: [
                    'text-accent-default underline-offset-4',
                    'hover:underline',
                ].join(' '),
            },
            size: {
                sm: 'h-8 px-3 text-fs-xs',
                md: 'h-9 px-4',
                lg: 'h-10 px-6 text-fs-base',
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
