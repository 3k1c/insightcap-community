import { describe, expect, it } from 'vitest';
import { appendReminderCreatedAck } from './reminder-ack';

describe('appendReminderCreatedAck', () => {
    it('appends a deterministic reminder confirmation when reminders were created', () => {
        expect(appendReminderCreatedAck('好的，我會處理。', 1, '已建立提醒。'))
            .toBe('好的，我會處理。\n已建立提醒。');
    });

    it('does not append when no reminder was created', () => {
        expect(appendReminderCreatedAck('知道了。', 0, '已建立提醒。'))
            .toBe('知道了。');
    });

    it('does not duplicate an existing confirmation', () => {
        expect(appendReminderCreatedAck('好的。\n已建立提醒。', 1, '已建立提醒。'))
            .toBe('好的。\n已建立提醒。');
    });
});
