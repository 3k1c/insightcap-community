import { beforeEach, describe, expect, it, vi } from 'vitest';

const mocks = vi.hoisted(() => ({
    writeBinaryFile: vi.fn(),
    exportPdfDocument: vi.fn(),
    readImageBase64: vi.fn(),
    html2canvas: vi.fn(),
    addImage: vi.fn(),
    savePdf: vi.fn(),
    outputPdf: vi.fn(),
}));

vi.mock('./tauri', () => ({
    tauriCmd: {
        writeBinaryFile: mocks.writeBinaryFile,
        exportPdfDocument: mocks.exportPdfDocument,
        readImageBase64: mocks.readImageBase64,
    },
}));

vi.mock('html2canvas', () => ({
    default: mocks.html2canvas,
}));

vi.mock('jspdf', () => {
    function JsPDF() {
        return {
            addImage: mocks.addImage,
            output: mocks.outputPdf,
            save: mocks.savePdf,
        };
    }

    return { default: JsPDF };
});

import { getEditorKnowledgeContent, writePdfFromHtml, __editorExportTest } from './editor-export';

describe('writePdfFromHtml', () => {
    beforeEach(() => {
        mocks.writeBinaryFile.mockClear();
        mocks.exportPdfDocument.mockClear();
        mocks.readImageBase64.mockClear();
        mocks.html2canvas.mockClear();
        mocks.addImage.mockClear();
        mocks.savePdf.mockClear();
        mocks.outputPdf.mockClear();
    });

    it('passes structured document blocks to the Rust PDF exporter', async () => {
        await writePdfFromHtml(
            'C:/tmp/export.pdf',
            '<h1>Report</h1><p>Hello <strong>PDF</strong></p><ul><li>First item</li></ul><table><tr><th>Name</th><th>Score</th></tr><tr><td>Ada</td><td>95</td></tr></table>',
        );

        expect(mocks.exportPdfDocument).toHaveBeenCalledWith('C:/tmp/export.pdf', [
            { type: 'heading', level: 1, text: 'Report', bold: true, align: 'left' },
            { type: 'paragraph', text: 'Hello PDF', bold: true, align: 'left', variant: 'text1' },
            { type: 'list_item', text: 'First item', ordered: false, index: 1, bold: false, align: 'left' },
            {
                type: 'table',
                rows: [
                    [
                        { text: 'Name', bold: true, background: '#f3f4f6', align: 'left' },
                        { text: 'Score', bold: true, background: '#f3f4f6', align: 'left' },
                    ],
                    [
                        { text: 'Ada', bold: false, background: undefined, align: 'left' },
                        { text: '95', bold: false, background: undefined, align: 'left' },
                    ],
                ],
            },
        ]);
        expect(mocks.html2canvas).not.toHaveBeenCalled();
        expect(mocks.addImage).not.toHaveBeenCalled();
        expect(mocks.savePdf).not.toHaveBeenCalled();
        expect(mocks.outputPdf).not.toHaveBeenCalled();
        expect(mocks.writeBinaryFile).not.toHaveBeenCalled();
    });

    it('preserves Traditional Chinese text in structured blocks', () => {
        const blocks = __editorExportTest.htmlToPdfBlocks('<h2>繁體中文標題</h2><p>這是一段測試文字。</p>');

        expect(blocks).toEqual([
            { type: 'heading', level: 2, text: '繁體中文標題', bold: true, align: 'left' },
            { type: 'paragraph', text: '這是一段測試文字。', bold: false, align: 'left', variant: 'text1' },
        ]);
    });

    it('carries paragraph size and alignment styles', () => {
        const blocks = __editorExportTest.htmlToPdfBlocks('<p data-variant="text2" style="text-align: center">Centered text</p>');

        expect(blocks).toEqual([
            { type: 'paragraph', text: 'Centered text', bold: false, align: 'center', variant: 'text2' },
        ]);
    });

    it('keeps editor images as PDF image blocks', () => {
        const blocks = __editorExportTest.htmlToPdfBlocks(
            '<div data-type="image-node-pro" src="data:image/png;base64,abc123" width="50%" textalign="right"></div>',
        );

        expect(blocks).toEqual([
            { type: 'image', src: 'data:image/png;base64,abc123', width: '50%', align: 'right' },
        ]);
    });

    it('normalizes editor image nodes before DOCX export', () => {
        const html = __editorExportTest.normalizeHtmlForDocx(
            '<div data-type="image-node-pro" src="data:image/png;base64,abc123" width="50%" textalign="center"></div>',
        );

        expect(html).toContain('<p data-editor-image-wrapper="true"');
        expect(html).toContain('text-align: center');
        expect(html).toContain('<img src="data:image/png;base64,abc123"');
        expect(html).toContain('width="430"');
        expect(html).toContain('data-editor-width="50%"');
        expect(html).toContain('width: 430px');
        expect(html).toContain('max-width: 430px');
    });

    it('normalizes right aligned editor images before DOCX export', () => {
        const html = __editorExportTest.normalizeHtmlForDocx(
            '<div data-type="image-node-pro" src="data:image/png;base64,abc123" width="25%" textalign="right"></div>',
        );

        expect(html).toContain('<p data-editor-image-wrapper="true"');
        expect(html).toContain('text-align: right');
        expect(html).toContain('width="215"');
        expect(html).toContain('data-editor-align="right"');
    });

    it('inlines local image paths before DOCX export', async () => {
        mocks.readImageBase64.mockResolvedValue('data:image/png;base64,abc123');

        const html = await __editorExportTest.inlineDocxLocalImages('<img src="C:/tmp/photo.png">');

        expect(mocks.readImageBase64).toHaveBeenCalledWith('C:/tmp/photo.png');
        expect(html).toContain('src="data:image/png;base64,abc123"');
    });

    it('adds proportional DOCX image height from data URL dimensions', () => {
        const png400x200 = 'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAZAAAADICAIAAADGFbfi';
        const html = __editorExportTest.applyDocxImageDimensions(
            `<p data-editor-image-wrapper="true"><img src="${png400x200}" width="430" data-editor-width="50%" style="width: 430px; max-width: 430px;"></p>`,
        );

        expect(html).toContain('width="430"');
        expect(html).toContain('height="215"');
        expect(html).toContain('height: 215px');
    });

    it('adds proportional DOCX image height from JPEG data URL dimensions', () => {
        const jpeg300x150Bytes = [
            0xff, 0xd8,
            0xff, 0xc0, 0x00, 0x11, 0x08,
            0x00, 0x96,
            0x01, 0x2c,
            0x03, 0x01, 0x11, 0x00, 0x02, 0x11, 0x00, 0x03, 0x11, 0x00,
            0xff, 0xd9,
        ];
        const jpeg300x150 = `data:image/jpeg;base64,${btoa(String.fromCharCode(...jpeg300x150Bytes))}`;
        const html = __editorExportTest.applyDocxImageDimensions(
            `<p data-editor-image-wrapper="true"><img src="${jpeg300x150}" width="300" style="width: 300px;"></p>`,
        );

        expect(html).toContain('width="300"');
        expect(html).toContain('height="150"');
    });

    it('stores editor exports as clean markdown instead of HTML tags', async () => {
        const content = await getEditorKnowledgeContent(
            'docx',
            '<h2><strong>觀塘</strong></h2><p>歡迎觀看 <strong>18區</strong></p>',
            '觀塘\n歡迎觀看 18區',
        );

        expect(content).toContain('## **觀塘**');
        expect(content).toContain('歡迎觀看 **18區**');
        expect(content).not.toContain('<h2>');
        expect(content).not.toContain('<strong>');
    });
});
