import { isTauri, invoke } from '@tauri-apps/api/core'

export async function persistDesktopCanvas(key: string, value: unknown): Promise<void> {
  if (isTauri() && key.startsWith('excalidraw-canvas-data')) {
    await invoke('save_canvas_snapshot', { key, value })
  }
}

export async function removeDesktopCanvas(key: string): Promise<void> {
  if (isTauri() && key.startsWith('excalidraw-canvas-data')) {
    await invoke('remove_canvas_snapshot', { key })
  }
}

export async function restoreDesktopCanvases(): Promise<void> {
  if (!isTauri()) return
  const snapshots = await invoke<Record<string, unknown>>('load_canvas_snapshots')
  for (const [key, value] of Object.entries(snapshots)) {
    localStorage.setItem(key, JSON.stringify(value))
  }
}
