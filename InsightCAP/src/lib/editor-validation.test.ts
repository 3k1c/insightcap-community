import { describe, expect, it } from 'vitest';
import { validateEditorDocument } from './editor-validation';

describe('validateEditorDocument', () => {
    it('reports empty placeholder documents', () => {
        const issues = validateEditorDocument({
            title: 'New Document 1',
            html: '<p></p>',
            text: '',
        });

        expect(issues.map((issue) => issue.code)).toEqual([
            'empty_document',
            'untitled_document',
        ]);
    });

    it('reports common export risks', () => {
        const issues = validateEditorDocument({
            title: 'Report',
            html: '<p>Hello</p><img src="data:image/png;base64,abc"><table><tr><th></th></tr><tr><td colspan="2"></td></tr></table><a href="not a url">bad</a>',
            text: 'Hello',
        });

        expect(issues.map((issue) => issue.code)).toContain('image_missing_alt');
        expect(issues.map((issue) => issue.code)).toContain('merged_table_cells');
        expect(issues.map((issue) => issue.code)).toContain('empty_table_header');
        expect(issues.map((issue) => issue.code)).toContain('empty_table_cell');
        expect(issues.map((issue) => issue.code)).toContain('invalid_link');
    });

    it('reports wide tables', () => {
        const issues = validateEditorDocument({
            title: 'Report',
            html: '<table><tr><td>1</td><td>2</td><td>3</td><td>4</td><td>5</td><td>6</td><td>7</td></tr></table>',
            text: '1 2 3 4 5 6 7',
        });

        expect(issues.map((issue) => issue.code)).toContain('wide_table');
    });
});
