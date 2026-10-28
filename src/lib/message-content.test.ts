import { describe, expect, it } from 'vitest'
import {
  removeJsonObjects,
  parseThinkingContent,
  buildHistoryForModel,
  sanitizeAssistantForHistory,
} from './message-content'

describe('removeJsonObjects', () => {
  it('strips excalidraw element JSON', () => {
    const text = `说明一下
{"id":"r1","type":"rectangle","x":1,"y":2,"width":10,"height":10}
完成`
    expect(removeJsonObjects(text)).toBe('说明一下\n\n完成')
  })

  it('keeps braces inside strings', () => {
    const text = `{"id":"t1","type":"text","x":0,"y":0,"width":10,"height":10,"text":"a{b}c"}`
    expect(removeJsonObjects(text)).toBe('')
  })

  it('keeps incomplete JSON for later streaming', () => {
    const text = '前言 {"id":"r1","type":"rectangle"'
    expect(removeJsonObjects(text)).toContain('前言')
    expect(removeJsonObjects(text)).toContain('"id":"r1"')
  })

  it('preserves non-element JSON objects', () => {
    const text = '结果 {"ok":true,"count":1}'
    expect(removeJsonObjects(text)).toContain('{"ok":true,"count":1}')
  })
})

describe('parseThinkingContent', () => {
  it('extracts think blocks', () => {
    const { thinking, main } = parseThinkingContent(
      '<think>先画框</think>然后输出'
    )
    expect(thinking).toBe('先画框')
    expect(main).toBe('然后输出')
  })
})

describe('history helpers', () => {
  it('sanitizes assistant messages', () => {
    const content =
      '<think>思考</think>说明{"id":"a","type":"rectangle","x":1,"y":1,"width":1,"height":1}'
    expect(sanitizeAssistantForHistory(content)).toBe('说明')
  })

  it('builds bounded history and excludes trailing user', () => {
    const messages = [
      { role: 'user' as const, content: '画一个框' },
      { role: 'assistant' as const, content: '好的' },
      { role: 'user' as const, content: '改成红色' },
    ]
    const history = buildHistoryForModel(messages, true)
    expect(history).toEqual([
      { role: 'user', content: '画一个框' },
      { role: 'assistant', content: '好的' },
    ])
  })
})

describe('canvas boundary presentation', () => {
  const element = '{"id":"box","type":"rectangle","x":1,"y":2}'
  const example = `\`\`\`markdown\n<canvas>${element}</canvas>\n<think>example</think>\n\`\`\``

  it.each(['<canvas>', '<canvas>{"id":', '<can', `<canvas>${element}</canvas>`])(
    'hides actual or unfinished canvas output: %s', (region) => {
      const result = removeJsonObjects(`说明${region}`)
      expect(result).toBe('说明')
    },
  )

  it('preserves fenced Skill JSON and canvas examples in UI and history', () => {
    const content = `${example}\n\`\`\`json\n${element}\n\`\`\``
    expect(removeJsonObjects(parseThinkingContent(content).main)).toBe(content)
    expect(sanitizeAssistantForHistory(content)).toBe(content)
  })

  it('preserves fenced examples while removing real regions and legacy elements', () => {
    const content = `${example}\n<canvas>${element}</canvas>完成${element}`
    const result = sanitizeAssistantForHistory(content)
    expect(result).toBe(`${example}\n完成`)
  })

  it('hides incomplete reasoning and extracts adjacent reasoning blocks', () => {
    const result = parseThinkingContent('正文<think>one</think><think>two</think><think>three')
    expect(result).toEqual({ main: '正文', thinking: 'onetwothree' })
  })

  it('preserves tags inside element strings until the whole region is hidden', () => {
    const content = '正文<canvas>{"id":"t","text":"</canvas><think>data</think>"}</canvas>完成'
    const result = sanitizeAssistantForHistory(content)
    expect(result).toBe('正文完成')
  })

  it('hides fences nested in actual canvas regions', () => {
    const result = removeJsonObjects(`<canvas>\n${example}\n</canvas>完成`)
    expect(result).toBe('完成')
  })
})
