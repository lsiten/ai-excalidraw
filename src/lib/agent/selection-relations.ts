import type { ElementSummary } from '@/components/excalidraw/wrapper'
import type { ParsedElement } from '@/components/excalidraw/element-parser'

type Bounds = {
  readonly minX: number
  readonly minY: number
  readonly maxX: number
  readonly maxY: number
}

export type SelectionRelationPlan = {
  readonly positionUpdates: readonly ParsedElement[]
  readonly arrows: readonly ParsedElement[]
}

function boundsOf(elements: readonly ElementSummary[]): Bounds | null {
  if (elements.length === 0) return null
  return elements.reduce<Bounds>((bounds, element, index) => {
    const left = Math.min(element.x, element.x + element.width)
    const top = Math.min(element.y, element.y + element.height)
    const right = Math.max(element.x, element.x + element.width)
    const bottom = Math.max(element.y, element.y + element.height)
    if (index === 0) return { minX: left, minY: top, maxX: right, maxY: bottom }
    return {
      minX: Math.min(bounds.minX, left),
      minY: Math.min(bounds.minY, top),
      maxX: Math.max(bounds.maxX, right),
      maxY: Math.max(bounds.maxY, bottom),
    }
  }, { minX: 0, minY: 0, maxX: 0, maxY: 0 })
}

function referenceRoots(elements: readonly ElementSummary[]): readonly ElementSummary[] {
  const roots = elements.filter((element) => !element.containerId)
  return roots.length > 0 ? roots : elements
}

export function buildSelectionRelationPlan(
  references: readonly ElementSummary[],
  generated: readonly ElementSummary[],
  existingIds: ReadonlySet<string>,
  createId: () => string = crypto.randomUUID,
): SelectionRelationPlan {
  const referenceBounds = boundsOf(referenceRoots(references))
  const generatedBounds = boundsOf(generated)
  if (!referenceBounds || !generatedBounds || generated.length === 0) {
    return { positionUpdates: [], arrows: [] }
  }

  const gap = 160
  const deltaX = referenceBounds.maxX + gap - generatedBounds.minX
  const referenceCenterY = (referenceBounds.minY + referenceBounds.maxY) / 2
  const generatedHeight = Math.max(generatedBounds.maxY - generatedBounds.minY, 1)
  const deltaY = referenceCenterY - (generatedBounds.minY + generatedHeight / 2)
  const positionUpdates = generated.map((element) => ({
    id: element.id,
    x: element.x + deltaX,
    y: element.y + deltaY,
  }))

  const movedBounds: Bounds = {
    minX: generatedBounds.minX + deltaX,
    minY: generatedBounds.minY + deltaY,
    maxX: generatedBounds.maxX + deltaX,
    maxY: generatedBounds.maxY + deltaY,
  }
  const roots = referenceRoots(references)
  const usedIds = new Set(existingIds)
  const arrows: ParsedElement[] = []
  for (const [index, reference] of roots.entries()) {
    let id = createId()
    while (usedIds.has(id)) id = createId()
    usedIds.add(id)

    const referenceRight = Math.max(reference.x, reference.x + reference.width)
    const referenceTop = Math.min(reference.y, reference.y + reference.height)
    const referenceBottom = Math.max(reference.y, reference.y + reference.height)
    const startX = referenceRight
    const startY = (referenceTop + referenceBottom) / 2
    const targetY = movedBounds.minY + ((index + 1) / (roots.length + 1)) * Math.max(movedBounds.maxY - movedBounds.minY, 1)
    const endX = movedBounds.minX
    const endY = targetY
    arrows.push({
      id,
      type: 'arrow',
      x: startX,
      y: startY,
      width: endX - startX,
      height: endY - startY,
      points: [[0, 0], [endX - startX, endY - startY]],
      strokeColor: '#64748b',
      strokeWidth: 2,
      roughness: 0,
      opacity: 70,
      startArrowhead: null,
      endArrowhead: 'arrow',
    })
  }

  return { positionUpdates, arrows }
}
