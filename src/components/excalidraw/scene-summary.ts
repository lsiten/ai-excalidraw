import type { ExcalidrawElement } from '@excalidraw/excalidraw/element/types'
import type { ElementSummary } from './wrapper'
export function summarizeElement(
  el: ExcalidrawElement,
  allElements?: readonly ExcalidrawElement[],
): ElementSummary {
  let text = 'text' in el ? (el.text as string | undefined) : undefined

  if (!text && el.boundElements && Array.isArray(el.boundElements) && allElements) {
    const boundTextElement = el.boundElements.find(
      (bound: { type: string; id: string }) => bound.type === 'text',
    )
    if (boundTextElement) {
      const textElement = allElements.find((e) => e.id === boundTextElement.id)
      if (textElement && 'text' in textElement && textElement.text) {
        text = textElement.text
      }
    }
  }

  return {
    id: el.id,
    type: el.type,
    text,
    x: el.x,
    y: el.y,
    width: el.width,
    height: el.height,
    strokeColor: el.strokeColor,
    backgroundColor: el.backgroundColor,
    containerId: 'containerId' in el ? (el.containerId ?? undefined) : undefined,
  }
}
