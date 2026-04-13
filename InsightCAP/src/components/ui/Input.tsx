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
                    className="text-fs-sm font-medium text-text-primary"
                >
                    {label}
                </label>
            )}
            <div className="relative flex items-center">
                {leftIcon && (
                    <span className="absolute left-3 text-text-tertiary">
                        {leftIcon}
                    </span>
                )}
                <input
                    id={inputId}
                    className={cn(
                        'w-full rounded-md border bg-surface-card',
                        'text-fs-sm text-text-primary placeholder:text-text-tertiary',
                        'border-stroke-divider',
                        'px-3 py-2 h-9',
                        'transition-colors duration-100',
                        'focus:outline-none focus:ring-1 focus:ring-stroke-focus focus:border-stroke-focus',
                        'disabled:opacity-50 disabled:cursor-not-allowed',
                        error && 'border-color-danger focus:ring-color-danger',
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
                <p className="text-fs-xs text-text-tertiary">{hint}</p>
            )}
            {error && (
                <p className="text-fs-xs text-color-danger">{error}</p>
            )}
        </div>
    );
}
