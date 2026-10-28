import { z } from 'zod'
import type { ExcalidrawWrapperRef } from '@/components/excalidraw/wrapper'

const id = z.string().min(1).max(200)
const number = z.number().finite()
const patch = z.object({
  id,
  type: z.enum(['rectangle', 'ellipse', 'diamond', 'text', 'arrow', 'line']).optional(),
  x: number.optional(), y: number.optional(),
  width: number.nonnegative().optional(), height: number.nonnegative().optional(),
  text: z.string().max(20000).optional(), originalText: z.string().max(20000).optional(),
  strokeColor: z.string().max(100).optional(), backgroundColor: z.string().max(100).optional(),
  strokeWidth: number.min(0).max(20).optional(),
  opacity: number.min(0).max(100).optional(), roughness: number.min(0).max(5).optional(),
  angle: number.optional(), fontSize: number.positive().max(500).optional(),
  fontFamily: z.number().int().min(1).max(8).optional(),
  textAlign: z.enum(['left', 'center', 'right']).optional(),
  verticalAlign: z.enum(['top', 'middle', 'bottom']).optional(),
  fillStyle: z.enum(['solid', 'hachure', 'cross-hatch']).optional(),
  points: z.array(z.tuple([number, number])).min(2).max(1000).optional(),
  startArrowhead: z.enum(['arrow', 'bar', 'dot', 'triangle']).nullable().optional(),
  endArrowhead: z.enum(['arrow', 'bar', 'dot', 'triangle']).nullable().optional(),
  containerId: id.nullable().optional(),
  boundElements: z.array(z.object({ id, type: z.enum(['text', 'arrow']) }).strict()).nullable().optional(),
}).strict()
const newElement = patch.required({ type: true, x: true, y: true, width: true, height: true })
export const streamElementPatchSchema = patch.strip()
const empty = z.object({}).strict()

export const canvasCommandSchema = z.discriminatedUnion('tool', [
  z.object({ tool: z.literal('get_canvas'), arguments: empty.default({}) }),
  z.object({ tool: z.literal('undo'), arguments: empty.default({}) }),
  z.object({ tool: z.literal('redo'), arguments: empty.default({}) }),
  z.object({ tool: z.literal('add_elements'), arguments: z.object({ elements: z.array(newElement).min(1).max(500) }).strict() }),
  z.object({ tool: z.literal('update_elements'), arguments: z.object({ elements: z.array(patch).min(1).max(500) }).strict() }),
  z.object({ tool: z.literal('delete_elements'), arguments: z.object({ ids: z.array(id).min(1).max(500) }).strict() }),
  z.object({ tool: z.literal('fit_canvas'), arguments: z.object({ ids: z.array(id).max(500).optional() }).strict().default({}) }),
])

export type CanvasCommand = z.infer<typeof canvasCommandSchema>
export type CanvasCommandTarget = Pick<ExcalidrawWrapperRef,
  'addElements' | 'updateElements' | 'deleteElements' | 'getCurrentSessionId' | 'getSelectedElementsSummary' | 'getCanvasBounds' | 'scrollToElements' | 'historyAction'> & {
  readonly getElements: () => readonly { readonly id: string; readonly type: string }[]
}

export function executeCanvasCommand(canvas: CanvasCommandTarget, command: CanvasCommand): unknown {
  switch (command.tool) {
    case 'undo':
    case 'redo': {
      return { applied: canvas.historyAction(command.tool) }
    }
    case 'get_canvas':
      return {
        sessionId: canvas.getCurrentSessionId(),
        elements: canvas.getElements(),
        selectedElements: canvas.getSelectedElementsSummary(),
        bounds: canvas.getCanvasBounds(),
      }
    case 'add_elements': {
      const ids = command.arguments.elements.map((element) => element.id)
      const existing = new Set(canvas.getElements().map((element) => element.id))
      if (new Set(ids).size !== ids.length || ids.some((value) => existing.has(value))) {
        throw new CanvasCommandError('New element IDs must be unique. Use update_elements for existing IDs.')
      }
      canvas.addElements(command.arguments.elements)
      return { added: ids }
    }
    case 'update_elements': {
      const current = new Map(canvas.getElements().map((element) => [element.id, element]))
      const elements = command.arguments.elements.map((element) => {
        const previous = current.get(element.id)
        if (element.type && previous && element.type !== previous.type) {
          throw new CanvasCommandError('Changing an element type is unsupported. Delete it and create a new ID.')
        }
        return typeof element.text === 'string' ? { ...element, originalText: element.text } : element
      })
      return canvas.updateElements(elements)
    }
    case 'delete_elements':
      return canvas.deleteElements(command.arguments.ids)
    case 'fit_canvas':
      canvas.scrollToElements(command.arguments.ids)
      return { fitted: true }
  }
}

export class CanvasCommandError extends Error {
  override readonly name = 'CanvasCommandError'
}
