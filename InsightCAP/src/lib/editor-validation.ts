export type EditorIssueSeverity = 'warning' | 'info';

export type EditorIssueCode =
    | 'empty_document'
    | 'untitled_document'
    | 'image_missing_alt'
    | 'large_inline_image'
    | 'empty_table'
    | 'empty_table_cell'
    | 'wide_table'
    | 'merged_table_cells'
    | 'empty_table_header'
    | 'invalid_link';

export interface EditorValidationIssue {
    code: EditorIssueCode;
    severity: EditorIssueSeverity;
    count?: number;
}

export interface EditorValidationInput {
    title: string;
    html: string;
    text: string;
}

const INLINE_IMAGE_WARN_BYTES = 750_000;
const WIDE_TABLE_COLUMN_LIMIT = 6;

function isLikelyPlaceholderTitle(title: string): boolean {
    const trimmed = title.trim().toLowerCase();
    return !trimmed || /^new document\b/.test(trimmed);
}

function isValidHref(href: string): boolean {
    const trimmed = href.trim();
    if (!trimmed) return false;
    if (trimmed.startsWith('#') || trimmed.startsWith('/') || trimmed.startsWith('./') || trimmed.startsWith('../')) {
        return true;
    }
    try {
        const url = new URL(trimmed);
        return ['http:', 'https:', 'mailto:'].includes(url.protocol);
    } catch {
        return false;
    }
}

export function validateEditorDocument(input: EditorValidationInput): EditorValidationIssue[] {
    const issues: EditorValidationIssue[] = [];
    const parser = new DOMParser();
    const doc = parser.parseFromString(input.html || '', 'text/html');
    const images = Array.from(doc.querySelectorAll('img'));

    if (!input.text.trim() && images.length === 0) {
        issues.push({ code: 'empty_document', severity: 'warning' });
    }

    if (isLikelyPlaceholderTitle(input.title)) {
        issues.push({ code: 'untitled_document', severity: 'info' });
    }

    const missingAltCount = images.filter((img) => !(img.getAttribute('alt') || img.getAttribute('title'))?.trim()).length;
    if (missingAltCount > 0) {
        issues.push({ code: 'image_missing_alt', severity: 'info', count: missingAltCount });
    }

    const largeInlineImageCount = images.filter((img) => {
        const src = img.getAttribute('src') || '';
        return src.startsWith('data:image/') && src.length > INLINE_IMAGE_WARN_BYTES;
    }).length;
    if (largeInlineImageCount > 0) {
        issues.push({ code: 'large_inline_image', severity: 'warning', count: largeInlineImageCount });
    }

    const tables = Array.from(doc.querySelectorAll('table'));
    const emptyTableCount = tables.filter((table) => !table.textContent?.trim()).length;
    if (emptyTableCount > 0) {
        issues.push({ code: 'empty_table', severity: 'warning', count: emptyTableCount });
    }

    const wideTableCount = tables.filter((table) => {
        const rows = Array.from(table.querySelectorAll('tr'));
        const maxColumns = rows.reduce((max, row) => {
            const count = Array.from(row.querySelectorAll('td,th')).reduce((sum, cell) => {
                const colspan = Number(cell.getAttribute('colspan') || '1');
                return sum + (Number.isFinite(colspan) && colspan > 0 ? colspan : 1);
            }, 0);
            return Math.max(max, count);
        }, 0);
        return maxColumns > WIDE_TABLE_COLUMN_LIMIT;
    }).length;
    if (wideTableCount > 0) {
        issues.push({ code: 'wide_table', severity: 'info', count: wideTableCount });
    }

    const mergedCellCount = Array.from(doc.querySelectorAll('td[colspan],td[rowspan],th[colspan],th[rowspan]')).filter((cell) => {
        const colspan = Number(cell.getAttribute('colspan') || '1');
        const rowspan = Number(cell.getAttribute('rowspan') || '1');
        return colspan > 1 || rowspan > 1;
    }).length;
    if (mergedCellCount > 0) {
        issues.push({ code: 'merged_table_cells', severity: 'info', count: mergedCellCount });
    }

    const emptyHeaderTableCount = tables.filter((table) => {
        const firstRow = table.querySelector('tr');
        if (!firstRow) return false;
        const headers = Array.from(firstRow.querySelectorAll('th'));
        return headers.length > 0 && headers.every((header) => !header.textContent?.trim());
    }).length;
    if (emptyHeaderTableCount > 0) {
        issues.push({ code: 'empty_table_header', severity: 'info', count: emptyHeaderTableCount });
    }

    const emptyCellCount = Array.from(doc.querySelectorAll('td,th')).filter((cell) => !cell.textContent?.trim()).length;
    if (emptyCellCount > 0) {
        issues.push({ code: 'empty_table_cell', severity: 'info', count: emptyCellCount });
    }

    const invalidLinkCount = Array.from(doc.querySelectorAll('a[href]')).filter((link) => !isValidHref(link.getAttribute('href') || '')).length;
    if (invalidLinkCount > 0) {
        issues.push({ code: 'invalid_link', severity: 'warning', count: invalidLinkCount });
    }

    return issues.slice(0, 20);
}
