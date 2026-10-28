import { describe, expect, it } from 'vitest'
import { canvasCommandSchema } from './canvas-commands'

describe('external canvas command boundary', () => {
  it('rejects a malformed creation before the canvas can change', () => {
    // Given a negative size, when a create command is parsed, then it is rejected.
    const result = canvasCommandSchema.safeParse({ tool: 'add_elements', arguments: { elements: [{ id: 'a', type: 'rectangle', x: 0, y: 0, width: -1, height: 80 }] } })
    expect(result.success).toBe(false)
  })

  it('accepts a partial update without requiring unrelated geometry', () => {
    // Given an existing ID and a color patch, when parsed, then the patch is preserved.
    const command = { tool: 'update_elements', arguments: { elements: [{ id: 'box', strokeColor: '#ff0000' }] } }
    expect(canvasCommandSchema.parse(command)).toEqual(command)
  })

  it('rejects unsupported executable fields', () => {
    // Given an unsupported field, when a command crosses the boundary, then it is rejected.
    const result = canvasCommandSchema.safeParse({ tool: 'update_elements', arguments: { elements: [{ id: 'a', link: 'javascript:alert(1)' }] } })
    expect(result.success).toBe(false)
  })

  it('rejects unknown tools', () => {
    // Given an unknown operation, when parsed, then it cannot reach the canvas.
    expect(canvasCommandSchema.safeParse({ tool: 'run_shell', arguments: {} }).success).toBe(false)
  })
})
