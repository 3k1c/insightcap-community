export function nextStreamingText(current: string, target: string): string {
    if (current.length >= target.length) return target;

    const remaining = target.length - current.length;

    // 尾端直通：最後 3 字直接顯示，避免懸空
    if (remaining <= 3) return target;

    // 固定節奏輸出（每 24ms）
    // buffer > 300 → 2字/幀 ≈ 83字/秒（積壓追趕）
    // 其餘        → 1字/幀 ≈ 42字/秒（舒適閱讀速度）
    const step = remaining > 300 ? 2 : 1;
    return target.slice(0, current.length + step);
}

