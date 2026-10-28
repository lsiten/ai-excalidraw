import type { ExcalidrawWrapperRef } from './wrapper'
import type { ParsedElement } from './element-parser'

export function applyAIElements(
  canvas: Pick<ExcalidrawWrapperRef, 'addElements' | 'deleteElements'>,
  elements: ParsedElement[],
): string[] {
  const visibleIds = new Set<string>()
  const pending = new Map<string, ParsedElement>()
  const flush = () => {
    if (pending.size === 0) return
    canvas.addElements([...pending.values()])
    pending.forEach((element) => visibleIds.add(element.id))
    pending.clear()
  }
  for (const element of elements) {
    if (element.isDeleted === true) {
      flush()
      const result = canvas.deleteElements([element.id])
      result.deleted.forEach((id) => visibleIds.delete(id))
    } else pending.set(element.id, { ...pending.get(element.id), ...element })
  }
  flush()
  return [...visibleIds]
}
