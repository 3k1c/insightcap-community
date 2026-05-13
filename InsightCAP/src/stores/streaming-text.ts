export function nextStreamingText(current: string, target: string): string {
    if (current.length >= target.length) return target;

    const remaining = target.length - current.length;

    // Flush the tail directly so the last few characters do not hang.
    if (remaining <= 3) return target;

    // Called every 24ms. Large backlogs catch up at 2 chars/frame;
    // normal responses advance at 1 char/frame for a steadier reading pace.
    const step = remaining > 300 ? 2 : 1;
    return target.slice(0, current.length + step);
}
