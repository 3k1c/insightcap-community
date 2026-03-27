import React from 'react';
import { Badge } from '../ui/Badge';
import { FileText, Link as LinkIcon, Image as ImageIcon } from 'lucide-react';

export type CitationType = 'data' | 'pattern' | 'log';
export type SourceType = 'article' | 'website' | 'image' | 'pdf';

interface CitationBadgeProps {
    type: CitationType;
    sourceType: SourceType;
    title: string;
    onClick?: () => void;
    className?: string;
}

const TYPE_CONFIG = {
    data: {
        colorClass: 'ic-memory-data',
        label: 'Data',
    },
    pattern: {
        colorClass: 'ic-memory-pattern',
        label: 'Pattern',
    },
    log: {
        colorClass: 'ic-memory-log',
        label: 'Log',
    },
};

const ICON_MAP = {
    article: FileText,
    website: LinkIcon,
    image: ImageIcon,
    pdf: FileText, // Could use a specific PDF icon
};

export const CitationBadge: React.FC<CitationBadgeProps> = ({
    type,
    sourceType,
    title,
    onClick,
    className = '',
}) => {
    const config = TYPE_CONFIG[type];
    const Icon = ICON_MAP[sourceType] || FileText;

    return (
        <Badge
            variant="outline"
            className={`
        cursor-pointer
        hover:shadow-sm
        transition-all duration-200
        border-${config.colorClass}
        text-${config.colorClass}
        bg-${config.colorClass}/10
        hover:bg-${config.colorClass}/20
        gap-1.5 px-2 py-0.5
        ${className}
      `}
            onClick={onClick}
            title={title}
        >
            <Icon className="w-3.5 h-3.5" />
            <span className="max-w-[120px] truncate text-xs font-medium">
                {title}
            </span>
            <span className="ml-1 text-[10px] opacity-70 uppercase tracking-wider">
                {config.label}
            </span>
        </Badge>
    );
};
