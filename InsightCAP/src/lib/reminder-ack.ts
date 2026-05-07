export function appendReminderCreatedAck(
    answer: string,
    createdCount: number,
    ackMessage: string,
): string {
    if (createdCount <= 0) return answer;

    const message = ackMessage.trim();
    if (!message || answer.includes(message)) return answer;

    return `${answer.trimEnd()}\n${message}`;
}
