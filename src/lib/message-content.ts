import { scanOutputSpans } from '../components/excalidraw/canvas-output-parser'

/**
 * 消息内容清洗：剥离思考标签与元素 JSON，供 UI 展示与多轮上下文共用
 */

function removeLegacyJsonObjects(text: string): string {
  let result = ''
  let i = 0

  while (i < text.length) {
    if (text[i] === '{') {
      let depth = 0
      let inString = false
      let escape = false
      let j = i

      for (; j < text.length; j++) {
        const char = text[j]

        if (escape) {
          escape = false
          continue
        }
        if (char === '\\' && inString) {
          escape = true
          continue
        }
        if (char === '"') {
          inString = !inString
          continue
        }
        if (inString) continue

        if (char === '{') depth++
        else if (char === '}') {
          depth--
          if (depth === 0) {
            const jsonStr = text.slice(i, j + 1)
            if (/"type"\s*:\s*"(rectangle|ellipse|diamond|text|arrow|line)"/.test(jsonStr)) {
              i = j + 1
              break
            } else {
              result += text[i]
              i++
              break
            }
          }
        }
      }

      if (depth !== 0) {
        result += text[i]
        i++
      }
    } else {
      result += text[i]
      i++
    }
  }

  return result
}

export function removeJsonObjects(text: string): string {
  return scanOutputSpans(text).map((span) => {
    if (span.inCanvas || span.kind === 'canvas') return ''
    const value = text.slice(span.start, span.end)
    return span.kind === 'text' ? removeLegacyJsonObjects(value) : value
  }).join('').replace(/\n{3,}/g, '\n\n').trim()
}

export function parseThinkingContent(content: string): { thinking: string; main: string } {
  let thinking = ''
  let main = ''
  for (const span of scanOutputSpans(content)) {
    const value = content.slice(span.start, span.end)
    if (span.kind === 'think') {
      thinking += value.replace(/^<think>/, '').replace(/<\/think>$/, '')
    } else {
      main += value
    }
  }
  return { thinking: thinking.trim(), main }
}

/** 清洗助手消息，用于发送给模型的历史上下文 */
export function sanitizeAssistantForHistory(content: string): string {
  const { main } = parseThinkingContent(content)
  const cleaned = removeJsonObjects(main)
  return cleaned || '[已在画布生成图形]'
}

const MAX_HISTORY_MESSAGES = 20
const MAX_HISTORY_CHARS = 12000

export interface HistoryTurn {
  role: 'user' | 'assistant'
  content: string
}

/** 将会话消息转为模型历史，控制条数与字符上限 */
export function buildHistoryForModel(
  messages: HistoryTurn[],
  excludeLastUser = true
): HistoryTurn[] {
  let source = messages
  if (excludeLastUser) {
    const last = source[source.length - 1]
    if (last?.role === 'user') {
      source = source.slice(0, -1)
    }
  }

  const sanitized: HistoryTurn[] = source.map((msg) => ({
    role: msg.role,
    content:
      msg.role === 'assistant'
        ? sanitizeAssistantForHistory(msg.content)
        : msg.content,
  })).filter((msg) => msg.content.trim().length > 0)

  const recent = sanitized.slice(-MAX_HISTORY_MESSAGES)
  let total = 0
  const result: HistoryTurn[] = []

  for (let i = recent.length - 1; i >= 0; i--) {
    const len = recent[i].content.length
    if (total + len > MAX_HISTORY_CHARS && result.length > 0) break
    total += len
    result.unshift(recent[i])
  }

  return result
}
