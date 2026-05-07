export type ChatHistoryMessage = {
    role: string;
    content: string;
};

export type ChatHistoryOptions = {
    maxMessages?: number;
    maxChars?: number;
    minRecentMessages?: number;
};

const DEFAULT_MAX_MESSAGES = 12;
const DEFAULT_MAX_CHARS = 8000;
const DEFAULT_MIN_RECENT_MESSAGES = 4;

export function buildChatHistory(
    messages: ChatHistoryMessage[],
    options: ChatHistoryOptions = {},
): [string, string][] {
    const maxMessages = options.maxMessages ?? DEFAULT_MAX_MESSAGES;
    const maxChars = options.maxChars ?? DEFAULT_MAX_CHARS;
    const minRecentMessages = options.minRecentMessages ?? DEFAULT_MIN_RECENT_MESSAGES;

    const eligible = messages
        .filter(m => (m.role === 'user' || m.role === 'assistant') && m.content.trim().length > 0)
        .slice(-maxMessages);

    const selected: ChatHistoryMessage[] = [];
    let usedChars = 0;

    for (let i = eligible.length - 1; i >= 0; i -= 1) {
        const msg = eligible[i];
        const nextChars = usedChars + msg.content.length;
        const mustKeepRecent = selected.length < minRecentMessages;

        if (!mustKeepRecent && nextChars > maxChars) {
            continue;
        }

        if (mustKeepRecent || nextChars <= maxChars) {
            selected.unshift(msg);
            usedChars += msg.content.length;
        }
    }

    return selected.map(m => [m.role, m.content]);
}
