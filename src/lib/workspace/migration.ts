import { invoke } from '@tauri-apps/api/core'
import { z } from 'zod'
import { workspace } from './store'
import { native } from './documents'
export async function migrateLegacy(projectId: string) {
  const snapshots = z
    .record(z.object({ elements: z.array(z.unknown()) }).passthrough())
    .parse(await invoke('load_canvas_snapshots'))
  let first: string | undefined,
    count = 0
  for (const [key, snapshot] of Object.entries(snapshots)) {
    if (!snapshot.elements.length) continue
    const path = `旧版画布-${Date.now()}-${count + 1}.excalidraw`
    await native('create_canvas', {
      projectId,
      path,
      content: JSON.stringify({
        ...snapshot,
        type: 'excalidraw',
        version: 2,
        source: 'AI Excalidraw 旧版迁移',
        appState: {},
        files: {},
      }),
    })
    first ??= path
    count++
    void key
  }
  await workspace.refresh()
  if (first) await workspace.openCanvas(projectId, first)
  else workspace.set({ error: '未发现需要迁移的非空旧版画布。原始缓存保持保留。' })
}
