import { useEffect, type RefObject } from 'react'
import { isTauri, invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { z } from 'zod'
import {
  CanvasCommandError,
  canvasCommandSchema,
  executeCanvasCommand,
} from '@/lib/canvas-commands'
import type { ExcalidrawWrapperRef } from '@/components/excalidraw/wrapper'

const envelope = z.object({ id: z.string().uuid(), command: z.unknown() })

export function useCanvasBridge(
  ref: RefObject<ExcalidrawWrapperRef | null>,
  disabled = false,
): void {
  useEffect(() => {
    if (!isTauri() || disabled) return
    let disposed = false
    const subscription = listen<unknown>('canvas-command', async (event) => {
      if (disposed) return
      const request = envelope.safeParse(event.payload)
      if (!request.success) return
      try {
        const command = canvasCommandSchema.parse(request.data.command)
        const canvas = ref.current
        if (!canvas?.isReady()) {
          await invoke('complete_canvas_command', {
            id: request.data.id,
            error: 'Canvas is loading. Retry when the editor is visible.',
          })
          return
        }
        const result = executeCanvasCommand(canvas, command)
        // Wait for Excalidraw's React scene commit before acknowledging a mutation.
        await new Promise<void>((resolve) =>
          requestAnimationFrame(() => requestAnimationFrame(() => resolve())),
        )
        if (
          command.tool !== 'get_canvas' &&
          command.tool !== 'fit_canvas' &&
          !(await canvas.flushCanvas())
        ) {
          throw new CanvasCommandError(
            'Canvas changed, but local storage could not save it. Export a backup before closing the app.',
          )
        }
        await invoke('complete_canvas_command', { id: request.data.id, result })
      } catch (error) {
        await invoke('complete_canvas_command', {
          id: request.data.id,
          error: error instanceof Error ? error.message : String(error),
        })
      }
    })
    void subscription
      .then(async () => {
        if (!disposed) await invoke('canvas_bridge_ready')
      })
      .catch((error: unknown) => console.error('Cannot register the canvas bridge:', error))
    return () => {
      disposed = true
      void subscription.then((unlisten) => unlisten())
    }
  }, [ref, disabled])
}
