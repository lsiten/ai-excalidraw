import { describe, expect, it } from 'vitest'
import { parseCanvasOutput } from './element-parser'

const box = '{"id":"box","type":"rectangle","x":1,"y":2}'
const update = '{"id":"box","strokeColor":"red"}'

describe('parseCanvasOutput', () => {
  it.each([
    `\`\`\`json\n${box}\n\`\`\``,
    `\`\`\`markdown\n<canvas>${box}</canvas>\n\`\`\``,
    `~~~~markdown\n<canvas>${box}</canvas>\n~~~\n<canvas>${box}</canvas>\n~~~~`,
    `<think><canvas>${box}</canvas></think>`,
    `<think><canvas>${box}`,
    `\`<canvas>${box}</canvas>\``,
    `\`\`\`md\n<canvas>${box}`,
  ])('ignores reasoning and documentation: %s', (text) => {
    const result = parseCanvasOutput(text)
    expect(result.elements).toEqual([])
  })

  it.each([box, update])('accepts legacy bare creation and selection updates by default: %s', (json) => {
    const result = parseCanvasOutput(`说明 ${json} 完成`)
    expect(result.elements).toEqual([JSON.parse(json)])
    expect(result.remainingBuffer).toBe(' 完成')
  })

  it('streams bare creation then a same-ID partial update without replay', () => {
    const first = parseCanvasOutput(box)
    const text = `${box}\n${update}`
    const result = parseCanvasOutput(text, box.length - first.remainingBuffer.length)
    expect(result.elements).toEqual([{ id: 'box', strokeColor: 'red' }])
    expect(result.remainingBuffer).toBe('')
  })

  it('parses multiple regions and updates', () => {
    const text = `前言<canvas>${box}</canvas>说明<canvas>${update}</canvas>完成`
    const result = parseCanvasOutput(text)
    expect(result.elements).toEqual([JSON.parse(box), JSON.parse(update)])
    expect(result.remainingBuffer).toBe('</canvas>完成')
  })

  it('isolates an unfinished object from the next adjacent region', () => {
    const text = '<canvas>{"id":</canvas><canvas>{"id":"next"}</canvas>'
    const result = parseCanvasOutput(text)
    expect(result.elements).toEqual([{ id: 'next' }])
  })

  it('emits a complete element before the closing tag arrives', () => {
    const text = `前言<canvas>${box}\n{"id":"next"`
    const result = parseCanvasOutput(text)
    expect(result.elements).toEqual([JSON.parse(box)])
    expect(result.remainingBuffer).toBe('\n{"id":"next"')
  })

  it('preserves nested data, escapes and literal protocol tags in strings', () => {
    const element = { id: 't', type: 'text', text: '"<canvas></canvas><think>`{x}', nested: { id: 'child' } }
    const result = parseCanvasOutput(`<canvas>${JSON.stringify(element)}</canvas>`)
    expect(result.elements).toEqual([element])
  })

  it('excludes code fences and thinking inside an active canvas region', () => {
    const text = `<canvas>\n\`\`\`json\n${box}\n\`\`\`\n<think>${box}</think>${update}</canvas>`
    const result = parseCanvasOutput(text)
    expect(result.elements).toEqual([JSON.parse(update)])
  })

  it('uses original offsets across every possible two-chunk boundary', () => {
    const text = `😀说明\n\`\`\`md\n<canvas>${box}</canvas>\n\`\`\`\n<think>${box}</think><canvas>${box}\n${update}</canvas>尾<canvas>{"id":"last"}</canvas>`
    for (let split = 0; split <= text.length; split++) {
      const first = parseCanvasOutput(text.slice(0, split))
      const offset = split - first.remainingBuffer.length
      const second = parseCanvasOutput(text, offset)
      expect([...first.elements, ...second.elements], `split=${split}`).toEqual([
        JSON.parse(box), JSON.parse(update), { id: 'last' },
      ])
      expect(text.endsWith(second.remainingBuffer)).toBe(true)
    }
  })

  it('emits each element once during character-by-character streaming', () => {
    const text = `前言<canvas>${box}</canvas>中<canvas>${update}</canvas>`
    const elements = []
    let processed = 0
    for (let length = 1; length <= text.length; length++) {
      const result = parseCanvasOutput(text.slice(0, length), processed)
      elements.push(...result.elements)
      processed = length - result.remainingBuffer.length
    }
    expect(elements).toEqual([JSON.parse(box), JSON.parse(update)])
    expect(processed).toBe(text.lastIndexOf('</canvas>'))
  })
})

it('preserves code-looking labels inside legacy bare element JSON', () => {
  // Given
  const element = { id: 'label', type: 'text', x: 1, y: 2, width: 250, height: 25, text: '调用 `fetch`，示例 <canvas> 与 <think> 均为标签文字' }
  // When
  const result = parseCanvasOutput(JSON.stringify(element))
  // Then
  expect(result.elements).toEqual([element])
})
