import { describe, expect, it } from 'vitest';
import { appendReminderCreatedAck } from './reminder-ack';

describe('appendReminderCreatedAck', () => {
    it('appends a deterministic reminder confirmation when reminders were created', () => {
        expect(appendReminderCreatedAck('Done.', 1, 'Reminder created.'))
            .toBe('Done.\nReminder created.');
    });

    it('does not append when no reminder was created', () => {
        expect(appendReminderCreatedAck('Done.', 0, 'Reminder created.'))
            .toBe('Done.');
    });

    it('does not duplicate an existing confirmation', () => {
        expect(appendReminderCreatedAck('Done.\nReminder created.', 1, 'Reminder created.'))
            .toBe('Done.\nReminder created.');
    });
});
