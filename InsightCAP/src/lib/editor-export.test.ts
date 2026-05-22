import { beforeEach, describe, expect, it, vi } from 'vitest';

const mocks = vi.hoisted(() => ({
    writeBinaryFile: vi.fn(),
    exportPdfDocument: vi.fn(),
    html2canvas: vi.fn(),
    addImage: vi.fn(),
    savePdf: vi.fn(),
    outputPdf: vi.fn(),
}));

vi.mock('./tauri', () => ({
    tauriCmd: {
        writeBinaryFile: mocks.writeBinaryFile,
        exportPdfDocument: mocks.exportPdfDocument,
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

import { writePdfFromHtml, __editorExportTest } from './editor-export';

describe('writePdfFromHtml', () => {
    beforeEach(() => {
        mocks.writeBinaryFile.mockClear();
        mocks.exportPdfDocument.mockClear();
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
});
