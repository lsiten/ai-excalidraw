import { describe, expect, it } from 'vitest'
import { buildSelectionRelationPlan } from './selection-relations'
import type { ElementSummary } from '@/components/excalidraw/wrapper'

const reference = (value: Partial<ElementSummary> & Pick<ElementSummary, 'id'>): ElementSummary => ({
  type: 'rectangle',
  x: 0,
  y: 0,
  width: 100,
  height: 100,
  ...value,
})

describe('buildSelectionRelationPlan', () => {
  it('places generated elements beside the references and creates one arrow per reference root', () => {
    const plan = buildSelectionRelationPlan(
      [
        reference({ id: 'reference-a' }),
        reference({ id: 'label-a', x: 10, y: 10, width: 80, height: 24, containerId: 'reference-a' }),
        reference({ id: 'reference-b', y: 200 }),
      ],
      [reference({ id: 'generated', width: 200 })],
      new Set(['reference-a', 'label-a', 'reference-b', 'generated']),
      (() => {
        let count = 0
        return () => `relation-${++count}`
      })(),
    )

    expect(plan.positionUpdates).toEqual([{ id: 'generated', x: 260, y: 100 }])
    expect(plan.arrows).toHaveLength(2)
    expect(plan.arrows.map((arrow) => arrow.id)).toEqual(['relation-1', 'relation-2'])
    expect(plan.arrows[0]).toMatchObject({ type: 'arrow', x: 100, y: 50, endArrowhead: 'arrow' })
    expect(plan.arrows[1]).toMatchObject({ type: 'arrow', x: 100, y: 250, endArrowhead: 'arrow' })
  })

  it('does not create relations without both a reference and a new result', () => {
    expect(buildSelectionRelationPlan([], [reference({ id: 'generated' })], new Set())).toEqual({
      positionUpdates: [],
      arrows: [],
    })
    expect(buildSelectionRelationPlan([reference({ id: 'reference' })], [], new Set())).toEqual({
      positionUpdates: [],
      arrows: [],
    })
  })
})
