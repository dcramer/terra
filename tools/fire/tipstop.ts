// The flame's top row in a crop's mask, for tips.ts: the first row with 3 flame pixels that belong
// to a blob (8-connected) of at least `share` of the crop's flame pixels, so a stray wisp or spark
// above the flame does not stretch the measured top third down onto the body (FOUND in fire round
// g60). share 0 keeps every pixel (the rule before g61). -1 when no row qualifies.
export const BLOB = 0.03;

export function topRow(mask: boolean[][], share = BLOB): number {
  const h = mask.length;
  const w = h ? mask[0].length : 0;
  const label = new Int32Array(w * h).fill(-1);
  const sizes: number[] = [];
  let total = 0;
  for (let y = 0; y < h; y++)
    for (let x = 0; x < w; x++) {
      if (!mask[y][x] || label[y * w + x] >= 0) continue;
      const id = sizes.length;
      let n = 0;
      const stack = [y * w + x];
      label[y * w + x] = id;
      while (stack.length) {
        const i = stack.pop() as number;
        n++;
        const cx = i % w;
        const cy = (i - cx) / w;
        for (let dy = -1; dy <= 1; dy++)
          for (let dx = -1; dx <= 1; dx++) {
            const nx = cx + dx;
            const ny = cy + dy;
            if (nx < 0 || ny < 0 || nx >= w || ny >= h || !mask[ny][nx]) continue;
            const j = ny * w + nx;
            if (label[j] >= 0) continue;
            label[j] = id;
            stack.push(j);
          }
      }
      sizes.push(n);
      total += n;
    }
  const min = share * total;
  for (let y = 0; y < h; y++) {
    let n = 0;
    for (let x = 0; x < w; x++) {
      const id = label[y * w + x];
      if (id >= 0 && sizes[id] >= min) n++;
    }
    if (n >= 3) return y;
  }
  return -1;
}
