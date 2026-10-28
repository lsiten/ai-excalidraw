import { afterEach, describe, expect, it, vi } from 'vitest'
import { copyIntegrationGuide } from './guide-clipboard'

afterEach(() => { vi.unstubAllGlobals() })

function textArea() {
  return { value: '完整指南\nMCP + Skill', focus: vi.fn(), select: vi.fn(), setSelectionRange: vi.fn() }
}

describe('guide clipboard fallback', () => {
  it('copies the complete guide through the modern API without selecting or invoking fallback', async () => {
    const textarea = textArea()
    const writeText = vi.fn().mockResolvedValue(undefined)
    const execCommand = vi.fn()
    vi.stubGlobal('navigator', { clipboard: { writeText } })
    vi.stubGlobal('document', { execCommand })
    expect(await copyIntegrationGuide(textarea)).toBe('copied')
    expect(writeText).toHaveBeenCalledExactlyOnceWith(textarea.value)
    expect(textarea.select).not.toHaveBeenCalled()
    expect(execCommand).not.toHaveBeenCalled()
  })

  it('selects the full text before legacy copy after permission denial', async () => {
    const textarea = textArea()
    const execCommand = vi.fn(() => {
      expect(textarea.focus).toHaveBeenCalledOnce()
      expect(textarea.select).toHaveBeenCalledOnce()
      expect(textarea.setSelectionRange).toHaveBeenCalledWith(0, textarea.value.length)
      return true
    })
    vi.stubGlobal('navigator', { clipboard: { writeText: vi.fn().mockRejectedValue(new Error('permission denied')) } })
    vi.stubGlobal('document', { execCommand })
    expect(await copyIntegrationGuide(textarea)).toBe('copied')
    expect(execCommand).toHaveBeenCalledExactlyOnceWith('copy')
  })

  it('returns manual-copy state and keeps the entire text selected if clipboard APIs are unavailable', async () => {
    const textarea = textArea()
    vi.stubGlobal('navigator', {})
    vi.stubGlobal('document', {})
    expect(await copyIntegrationGuide(textarea)).toBe('manual')
    expect(textarea.setSelectionRange).toHaveBeenCalledWith(0, textarea.value.length)
  })

  it('does not report success when legacy copy returns false', async () => {
    vi.stubGlobal('navigator', {})
    vi.stubGlobal('document', { execCommand: vi.fn(() => false) })
    expect(await copyIntegrationGuide(textArea())).toBe('manual')
  })
})
