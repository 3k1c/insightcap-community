import React from 'react';
import { cn } from '../../lib/utils';

export interface InputProps extends React.InputHTMLAttributes<HTMLInputElement> {
    label?: string;
    error?: string;
    hint?: string;
    leftIcon?: React.ReactNode;
    rightElement?: React.ReactNode;
}

export function Input({
    className,
    label,
    error,
    hint,
    leftIcon,
    rightElement,
    id,
    ...props
}: InputProps) {
    const inputId = id || label?.toLowerCase().replace(/\s/g, '-');

    return (
        <div className="flex flex-col gap-1.5">
            {label && (
                <label
                    htmlFor={inputId}
                    className="text-sm font-medium text-[var(--ic-text-primary)]"
                >
                    {label}
                </label>
            )}
            <div className="relative flex items-center">
                {leftIcon && (
                    <span className="absolute left-3 text-[var(--ic-text-muted)]">
                        {leftIcon}
                    </span>
                )}
                <input
                    id={inputId}
                    className={cn(
                        'w-full rounded-md border bg-[var(--ic-bg-elevated)]',
                        'text-sm text-[var(--ic-text-primary)] placeholder:text-[var(--ic-text-muted)]',
                        'border-[var(--ic-border-default)]',
                        'px-3 py-2 h-9',
                        'transition-colors duration-100',
                        'focus:outline-none focus:ring-1 focus:ring-[var(--ic-border-focus)] focus:border-[var(--ic-border-focus)]',
                        'disabled:opacity-50 disabled:cursor-not-allowed',
                        error && 'border-[var(--ic-danger)] focus:ring-[var(--ic-danger)]',
                        leftIcon && 'pl-9',
                        rightElement && 'pr-10',
                        className
                    )}
                    {...props}
                />
                {rightElement && (
                    <span className="absolute right-3">
                        {rightElement}
                    </span>
                )}
            </div>
            {hint && !error && (
                <p className="text-xs text-[var(--ic-text-muted)]">{hint}</p>
            )}
            {error && (
                <p className="text-xs text-[var(--ic-danger)]">{error}</p>
            )}
        </div>
    );
}
