import { tauriCmd } from './tauri';

export type EditorExportFormat = 'txt' | 'md' | 'html' | 'docx' | 'pdf';

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
    const blobOrBuffer = await asBlob(buildStandaloneHtml(html));
    const bytes = blobOrBuffer instanceof Blob
        ? new Uint8Array(await blobOrBuffer.arrayBuffer())
        : new Uint8Array(blobOrBuffer as unknown as ArrayBufferLike);

    let binary = '';
    const chunk = 0x8000;
    for (let i = 0; i < bytes.length; i += chunk) {
        binary += String.fromCharCode(...bytes.subarray(i, i + chunk));
    }
    await tauriCmd.writeBinaryFile(filePath, btoa(binary));
}

export async function writePdfFromElement(filePath: string, element: HTMLElement): Promise<void> {
    const { default: jsPDF } = await import('jspdf');
    const { default: html2canvas } = await import('html2canvas');
    const canvas = await html2canvas(element, { scale: 2, useCORS: true, backgroundColor: '#ffffff' });
    const imgData = canvas.toDataURL('image/png');
    const pdf = new jsPDF({ orientation: 'p', unit: 'mm', format: 'a4' });
    const pageW = pdf.internal.pageSize.getWidth();
    const pageH = pdf.internal.pageSize.getHeight();
    const imgH = (canvas.height * pageW) / canvas.width;
    let yOffset = 0;
    while (yOffset < imgH) {
        if (yOffset > 0) pdf.addPage();
        pdf.addImage(imgData, 'PNG', 0, -yOffset, pageW, imgH);
        yOffset += pageH;
    }
    pdf.save(filePath);
}
