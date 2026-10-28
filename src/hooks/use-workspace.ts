import { useEffect, useSyncExternalStore } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { workspace } from '@/lib/workspace/store'
import { presentations } from '@/lib/presentation/controller'
import { workspaceCommand } from '@/lib/workspace/commands'
import { message } from '@/lib/workspace/types'
import { z } from 'zod'
const requestSchema = z.object({ id: z.string().uuid(), command: z.unknown() })
let initialization: Promise<void> | undefined
export function useWorkspace() {
  const state = useSyncExternalStore(workspace.subscribe, workspace.getSnapshot)
  useEffect(() => {
    initialization ??= workspace.initialize()
    let disposed = false
    const subscriptions = [
      listen<unknown>('canvas-command', async (event) => {
        const request = requestSchema.safeParse(event.payload)
        if (disposed || !request.success) return
        try {
          const result = await workspaceCommand(request.data.command, request.data.id)
          await invoke('complete_canvas_command', { id: request.data.id, result })
        } catch (error) {
          await invoke('complete_canvas_command', { id: request.data.id, error: message(error) })
        }
      }),
      listen('flush-before-exit', () => {
        void presentations
          .stop()
          .then(() => initialization)
          .then(() => workspace.run(() => workspace.flushAll()))
          .then(() => invoke('workspace_exit'))
          .catch((error) => workspace.set({ error: `退出前保存失败：${message(error)}` }))
      }),
      listen<string[]>('open-files', (event) => {
        void openPaths(event.payload).catch((error) => workspace.set({ error: message(error) }))
      }),
    ]
    void Promise.all(subscriptions)
      .then(async () => {
        if (!disposed) await invoke('canvas_bridge_ready')
        await initialization
        if (!disposed) {
          await openPaths(z.array(z.string()).parse(await invoke('take_launch_files')))
        }
      })
      .catch((error) => workspace.set({ error: message(error) }))
    const check = (refreshTree = false) => {
      if (
        workspace.getSnapshot().initialized &&
        !workspace.getSnapshot().busy &&
        !workspace.getSnapshot().aiBusy &&
        !workspace.getSnapshot().presenting
      )
        void workspace
          .run(() => workspace.checkExternal(refreshTree))
          .catch((error) => workspace.set({ error: message(error) }))
    }
    const focus = () => check(true)
    window.addEventListener('focus', focus)
    const timer = setInterval(() => check(), 8000)
    return () => {
      disposed = true
      clearInterval(timer)
      window.removeEventListener('focus', focus)
      subscriptions.forEach((p) => void p.then((unlisten) => unlisten()))
    }
  }, [])
  return state
}
async function openPaths(paths: readonly string[]) {
  if (!paths.length) return
  await workspace.run(async () => {
    for (const path of paths) {
      if (!path.endsWith('.excalidraw')) continue
      await workspace.openFilePath(path)
    }
  })
}
