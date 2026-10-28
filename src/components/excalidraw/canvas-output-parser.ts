/** Original-text spans shared by canvas parsing and message presentation. */
export interface OutputSpan {
  readonly kind: 'text' | 'code' | 'think' | 'canvas'
  readonly start: number
  readonly end: number
  readonly inCanvas: boolean
}

export function scanOutputSpans(text: string): OutputSpan[] {
  const spans: OutputSpan[] = []
  let canvas = false
  let inString = false
  let escaped = false
  let depth = 0
  let i = 0

  const append = (kind: OutputSpan['kind'], end: number) => {
    const previous = spans[spans.length - 1]
    if (kind !== 'think' && previous?.kind === kind && previous.inCanvas === canvas) {
      spans[spans.length - 1] = { ...previous, end }
    } else {
      spans.push({ kind, start: i, end, inCanvas: canvas })
    }
    i = end
  }

  while (i < text.length) {
    // Protocol-looking strings in JSON are element data, not delimiters.
    if (inString) {
      const char = text[i]
      if (escaped) escaped = false
      else if (char === '\\') escaped = true
      else if (char === '"') inString = false
      append(canvas ? 'canvas' : 'text', i + 1)
      continue
    }

    const tail = text.slice(i)
    const fence = (i === 0 || text[i - 1] === '\n')
      ? /^ {0,3}(`{3,}|~{3,})[^\n]*(?:\n|$)/.exec(tail)
      : null
    if (fence) {
      const marker = fence[1]
      const close = new RegExp(`^ {0,3}${marker[0]}{${marker.length},}[ \\t]*\\r?$`, 'm')
      const rest = tail.slice(fence[0].length)
      const match = close.exec(rest)
      append('code', match ? i + fence[0].length + match.index + match[0].length : text.length)
      continue
    }
    // Inline code is also documentation, including streaming unfinished examples.
    const ticks = /^`+/.exec(tail)
    if (ticks) {
      const end = text.indexOf(ticks[0], i + ticks[0].length)
      append('code', end < 0 ? text.length : end + ticks[0].length)
      continue
    }
    if (tail.startsWith('<think>')) {
      const end = text.indexOf('</think>', i + 7)
      append('think', end < 0 ? text.length : end + 8)
      continue
    }
    if (!canvas && tail.startsWith('<canvas>')) {
      canvas = true
      depth = 0
      spans.push({ kind: 'canvas', start: i, end: i + 8, inCanvas: true })
      i += 8
      continue
    }
    if (canvas && tail.startsWith('</canvas>')) {
      append('canvas', i + 9)
      canvas = false
      depth = 0
      continue
    }
    if (!canvas && tail.startsWith('<c') && '<canvas>'.startsWith(tail)) {
      append('canvas', text.length)
      continue
    }
    if (text[i] === '{') depth++
    if (text[i] === '}') depth = Math.max(0, depth - 1)
    if (depth > 0 && text[i] === '"') inString = true
    append(canvas ? 'canvas' : 'text', i + 1)
  }
  return spans
}
