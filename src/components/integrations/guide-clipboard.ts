type GuideTextArea = Pick<HTMLTextAreaElement, 'value' | 'focus' | 'select' | 'setSelectionRange'>

export async function copyIntegrationGuide(textarea: GuideTextArea): Promise<'copied' | 'manual'> {
  try {
    await navigator.clipboard.writeText(textarea.value)
    return 'copied'
  } catch {
    // WebView clipboard permissions can deny the modern API; keep a selectable fallback.
    textarea.focus()
    textarea.select()
    textarea.setSelectionRange(0, textarea.value.length)
    try {
      return document.execCommand('copy') ? 'copied' : 'manual'
    } catch {
      return 'manual'
    }
  }
}
