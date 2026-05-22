import { tauriCmd } from './tauri';

export type EditorExportFormat = 'txt' | 'md' | 'html' | 'docx' | 'pdf';

export type PdfTextAlign = 'left' | 'center' | 'right';

export type PdfTableCell = {
    text: string;
    bold: boolean;
    background?: string;
    align: PdfTextAlign;
};

export type PdfExportBlock =
    | { type: 'heading'; level: number; text: string; bold: true; align: PdfTextAlign }
    | { type: 'paragraph'; text: string; bold: boolean; align: PdfTextAlign; variant: 'text1' | 'text2' | 'text3' }
    | { type: 'list_item'; text: string; ordered: boolean; index: number; bold: boolean; align: PdfTextAlign }
    | { type: 'table'; rows: PdfTableCell[][] }
    | { type: 'image'; src: string; width?: string; align: PdfTextAlign };

const DOCX_CONTENT_WIDTH_PX = 860;

export function buildStandaloneHtml(innerHtml: string): string {
    const minimalCSS = [
        'body{font-family:-apple-system,BlinkMacSystemFont,"Segoe UI",Roboto,"Helvetica Neue",Arial;color:#111827;padding:32px 40px;max-width:860px;margin:0 auto;background:#ffffff}',
        'h1{font-size:1.5rem;line-height:2rem;font-weight:700;margin-top:1.5rem;margin-bottom:0.75rem;color:#0f172a}',
        'h2{font-size:1.25rem;line-height:1.75rem;font-weight:700;margin-top:1.25rem;margin-bottom:0.5rem;color:#0f172a}',
        'h3{font-size:1.125rem;line-height:1.625rem;font-weight:700;margin-top:1rem;margin-bottom:0.4rem;color:#0f172a}',
        'h4{font-size:1rem;line-height:1.5rem;font-weight:700;margin-top:0.875rem;margin-bottom:0.35rem;color:#0f172a}',
        'h5{font-size:0.9375rem;line-height:1.4rem;font-weight:700;margin-top:0.75rem;margin-bottom:0.3rem;color:#0f172a}',
        'h6{font-size:0.875rem;line-height:1.35rem;font-weight:700;margin-top:0.625rem;margin-bottom:0.25rem;color:#0f172a}',
        'p{margin-bottom:0.75rem;line-height:1.65;color:#1f2937}',
        'strong{font-weight:700}',
        'em{font-style:italic}',
        'u{text-decoration:underline}',
        'ul,ol{padding-left:1.75rem;margin:0.25rem 0;color:#1f2937}',
        'ul{list-style-type:disc}ul ul{list-style-type:circle}ul ul ul{list-style-type:square}',
        'ol{list-style-type:decimal}ol ol{list-style-type:lower-alpha}ol ol ol{list-style-type:lower-roman}',
        'li{margin-bottom:0.15rem}',
        'img{max-width:100%;height:auto;display:block;margin:0.5rem 0}',
        'table{border-collapse:collapse;table-layout:fixed;width:100%;margin:1.5rem 0;border:1px solid #d1d5db}',
        'th,td{border:1px solid #d1d5db;padding:6px 8px;vertical-align:top;box-sizing:border-box;color:#111827}',
        'th{font-weight:700;text-align:left;background-color:#f3f4f6}',
        'tr:nth-child(even) td{background:#fafafa}',
        'pre{background:#f3f4f6;border-radius:6px;padding:0.75rem 1rem;font-family:"Courier New",monospace;font-size:0.875rem;overflow-x:auto;margin:0.75rem 0;color:#111827}',
        'code{font-family:"Courier New",monospace;font-size:0.9em;background:#f3f4f6;padding:0.125rem 0.25rem;border-radius:3px;color:#111827}',
        'pre code{background:transparent;padding:0}',
        'mark{background-color:#fef3c7;color:#92400e;padding:0.1em 0.2em;border-radius:0.2em}',
        'a{color:#2563eb;text-decoration:underline}',
        'blockquote{border-left:3px solid #d1d5db;margin:0.5rem 0;padding:0.25rem 0 0.25rem 1rem;color:#6b7280;font-style:italic}',
    ].join('\n');
    return `<!doctype html><html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><style>${minimalCSS}</style></head><body>${innerHtml}</body></html>`;
}

export async function htmlToMarkdown(html: string): Promise<string> {
    const TurndownService = (await import('turndown')).default;
    const td = new TurndownService({ headingStyle: 'atx', bulletListMarker: '-' });
    return td.turndown(html);
}

export async function writeDocxFromHtml(filePath: string, html: string): Promise<void> {
    const { asBlob } = await import('html-docx-js-typescript');
    const normalizedHtml = applyDocxImageDimensions(await inlineDocxLocalImages(normalizeHtmlForDocx(html)));
    const blobOrBuffer = await asBlob(buildStandaloneHtml(normalizedHtml));
    const bytes = blobOrBuffer instanceof Blob
        ? new Uint8Array(await blobOrBuffer.arrayBuffer())
        : new Uint8Array(blobOrBuffer as unknown as ArrayBufferLike);

    await tauriCmd.writeBinaryFile(filePath, bytesToBase64(bytes));
}

function normalizeHtmlForDocx(html: string): string {
    const doc = new DOMParser().parseFromString(html, 'text/html');

    doc.body.querySelectorAll('[data-type="image-node-pro"]').forEach((node) => {
        if (!(node instanceof HTMLElement)) return;

        const src = node.getAttribute('src') ?? '';
        if (!src) {
            node.remove();
            return;
        }

        const align = getTextAlign(node);
        const wrapper = doc.createElement('p');
        wrapper.setAttribute('data-editor-image-wrapper', 'true');
        wrapper.setAttribute('data-editor-align', align);
        wrapper.style.textAlign = align;
        wrapper.style.margin = '0.5rem 0';

        const image = doc.createElement('img');
        image.setAttribute('src', src);

        const width = node.getAttribute('width');
        if (width) {
            const docxWidth = toDocxImageWidthPx(width);
            image.setAttribute('width', String(docxWidth));
            image.setAttribute('data-editor-width', width);
            image.style.width = `${docxWidth}px`;
            image.style.maxWidth = `${docxWidth}px`;
        } else {
            image.style.maxWidth = '100%';
        }

        const alt = node.getAttribute('alt');
        if (alt) image.setAttribute('alt', alt);

        const title = node.getAttribute('title');
        if (title) image.setAttribute('title', title);

        image.style.height = 'auto';
        image.style.display = 'inline-block';

        wrapper.appendChild(image);
        node.replaceWith(wrapper);
    });

    return doc.body.innerHTML;
}

function toDocxImageWidthPx(width: string): number {
    const trimmed = width.trim();
    if (trimmed.endsWith('%')) {
        const percent = Number.parseFloat(trimmed);
        if (Number.isFinite(percent) && percent > 0) {
            return Math.round(DOCX_CONTENT_WIDTH_PX * Math.min(percent, 100) / 100);
        }
    }

    const pixels = Number.parseFloat(trimmed.replace(/px$/i, ''));
    if (Number.isFinite(pixels) && pixels > 0) {
        return Math.round(Math.min(pixels, DOCX_CONTENT_WIDTH_PX));
    }

    return DOCX_CONTENT_WIDTH_PX;
}

async function inlineDocxLocalImages(html: string): Promise<string> {
    const doc = new DOMParser().parseFromString(html, 'text/html');
    const images = Array.from(doc.body.querySelectorAll('img'));

    for (const image of images) {
        const src = image.getAttribute('src') ?? '';
        if (!isLocalImageSource(src)) continue;

        const path = src.startsWith('file://') ? src.slice('file://'.length) : src;
        image.setAttribute('src', await tauriCmd.readImageBase64(path));
    }

    return doc.body.innerHTML;
}

function applyDocxImageDimensions(html: string): string {
    const doc = new DOMParser().parseFromString(html, 'text/html');

    doc.body.querySelectorAll('img').forEach((image) => {
        const width = Number.parseInt(image.getAttribute('width') ?? '', 10);
        if (!Number.isFinite(width) || width <= 0) return;

        const size = getDataUrlImageSize(image.getAttribute('src') ?? '');
        if (!size) return;

        const height = Math.max(1, Math.round(width * size.height / size.width));
        image.setAttribute('height', String(height));
        image.style.width = `${width}px`;
        image.style.maxWidth = `${width}px`;
        image.style.height = `${height}px`;
    });

    return doc.body.innerHTML;
}

function getDataUrlImageSize(src: string): { width: number; height: number } | null {
    const match = /^data:image\/(png|jpe?g);base64,([A-Za-z0-9+/=]+)$/i.exec(src);
    if (!match) return null;

    try {
        const binary = atob(match[2]);
        if (binary.length < 10) return null;
        const bytes = Uint8Array.from(binary, (char) => char.charCodeAt(0));
        const size = match[1].toLowerCase() === 'png'
            ? getPngImageSize(bytes)
            : getJpegImageSize(bytes);
        if (!size) return null;
        const { width, height } = size;
        if (width <= 0 || height <= 0) return null;
        return { width, height };
    } catch {
        return null;
    }
}

function getPngImageSize(bytes: Uint8Array): { width: number; height: number } | null {
    const isPng = bytes[0] === 0x89
        && bytes[1] === 0x50
        && bytes[2] === 0x4e
        && bytes[3] === 0x47;
    if (!isPng || bytes.length < 24) return null;
    return {
        width: readUint32Be(bytes, 16),
        height: readUint32Be(bytes, 20),
    };
}

function getJpegImageSize(bytes: Uint8Array): { width: number; height: number } | null {
    if (bytes[0] !== 0xff || bytes[1] !== 0xd8) return null;

    let offset = 2;
    while (offset + 8 < bytes.length) {
        if (bytes[offset] !== 0xff) return null;
        const marker = bytes[offset + 1];
        const length = (bytes[offset + 2] << 8) + bytes[offset + 3];
        if (length < 2 || offset + 2 + length > bytes.length) return null;

        const isStartOfFrame = marker >= 0xc0
            && marker <= 0xcf
            && ![0xc4, 0xc8, 0xcc].includes(marker);
        if (isStartOfFrame) {
            return {
                height: (bytes[offset + 5] << 8) + bytes[offset + 6],
                width: (bytes[offset + 7] << 8) + bytes[offset + 8],
            };
        }

        offset += 2 + length;
    }

    return null;
}

function readUint32Be(bytes: Uint8Array, offset: number): number {
    return ((bytes[offset] << 24) >>> 0)
        + (bytes[offset + 1] << 16)
        + (bytes[offset + 2] << 8)
        + bytes[offset + 3];
}

function isLocalImageSource(src: string): boolean {
    return Boolean(src)
        && !src.startsWith('data:')
        && !src.startsWith('http://')
        && !src.startsWith('https://')
        && !src.startsWith('blob:')
        && !src.startsWith('asset:');
}

function bytesToBase64(bytes: Uint8Array): string {
    let binary = '';
    const chunk = 0x8000;
    for (let i = 0; i < bytes.length; i += chunk) {
        binary += String.fromCharCode(...bytes.subarray(i, i + chunk));
    }
    return btoa(binary);
}

export async function writePdfFromHtml(filePath: string, html: string): Promise<void> {
    await tauriCmd.exportPdfDocument(filePath, htmlToPdfBlocks(html));
}

function normalizeText(text: string): string {
    return text.replace(/\s+/g, ' ').trim();
}

function htmlToPdfBlocks(html: string): PdfExportBlock[] {
    const doc = new DOMParser().parseFromString(html, 'text/html');
    const blocks: PdfExportBlock[] = [];

    const appendParagraph = (value: string, bold: boolean) => {
        const text = normalizeText(value);
        if (text) blocks.push({ type: 'paragraph', text, bold, align: 'left', variant: 'text1' });
    };

    const walk = (node: ChildNode) => {
        if (node.nodeType === Node.TEXT_NODE) {
            appendParagraph(node.textContent ?? '', false);
            return;
        }
        if (!(node instanceof HTMLElement)) return;

        const tag = node.tagName.toLowerCase();
        if (tag === 'img' || node.getAttribute('data-type') === 'image-node-pro') {
            const src = node.getAttribute('src') ?? '';
            if (src) {
                blocks.push({
                    type: 'image',
                    src,
                    width: node.getAttribute('width') ?? undefined,
                    align: getTextAlign(node),
                });
            }
            return;
        }
        if (/^h[1-6]$/.test(tag)) {
            const text = normalizeText(node.textContent ?? '');
            if (text) blocks.push({ type: 'heading', level: Number(tag.slice(1)), text, bold: true, align: getTextAlign(node) });
            return;
        }
        if (tag === 'p' || tag === 'blockquote' || tag === 'pre') {
            const text = normalizeText(node.textContent ?? '');
            if (text) {
                blocks.push({
                    type: 'paragraph',
                    text,
                    bold: hasBoldContent(node),
                    align: getTextAlign(node),
                    variant: getParagraphVariant(node),
                });
            }
            return;
        }
        if (tag === 'ul' || tag === 'ol') {
            Array.from(node.children).forEach((item, index) => {
                if (item.tagName.toLowerCase() !== 'li') return;
                const text = normalizeText(item.textContent ?? '');
                if (text) {
                    blocks.push({
                        type: 'list_item',
                        text,
                        ordered: tag === 'ol',
                        index: index + 1,
                        bold: hasBoldContent(item as HTMLElement),
                        align: getTextAlign(item as HTMLElement),
                    });
                }
            });
            return;
        }
        if (tag === 'table') {
            const rows = Array.from(node.querySelectorAll('tr')).map((row) => {
                const cells = Array.from(row.children)
                    .filter((cell) => ['td', 'th'].includes(cell.tagName.toLowerCase()))
                    .map((cell) => {
                        const element = cell as HTMLElement;
                        const isHeader = element.tagName.toLowerCase() === 'th';
                        return {
                            text: normalizeText(element.textContent ?? ''),
                            bold: isHeader || hasBoldContent(element),
                            background: isHeader ? '#f3f4f6' : undefined,
                            align: getTextAlign(element),
                        };
                    });
                return cells;
            }).filter((row) => row.length > 0);
            if (rows.length > 0) blocks.push({ type: 'table', rows });
            return;
        }

        Array.from(node.childNodes).forEach(walk);
    };

    Array.from(doc.body.childNodes).forEach(walk);
    return blocks;
}

function hasBoldContent(element: HTMLElement): boolean {
    return Boolean(element.closest('strong,b') || element.querySelector('strong,b'));
}

function getTextAlign(element: HTMLElement): PdfTextAlign {
    const value =
        element.style.textAlign ||
        element.getAttribute('data-text-align') ||
        element.getAttribute('textalign') ||
        element.getAttribute('align') ||
        '';
    if (value === 'center' || value === 'right') return value;
    return 'left';
}

function getParagraphVariant(element: HTMLElement): 'text1' | 'text2' | 'text3' {
    const value = element.getAttribute('data-variant');
    if (value === 'text2' || value === 'text3') return value;
    return 'text1';
}

export const __editorExportTest = {
    htmlToPdfBlocks,
    normalizeHtmlForDocx,
    inlineDocxLocalImages,
    applyDocxImageDimensions,
};
