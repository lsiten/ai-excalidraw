import { loadFromBlob } from '@excalidraw/excalidraw'
import { invoke } from '@tauri-apps/api/core'
import { z } from 'zod'
import { fileSchema, chatSchema, canvasId, message, WorkspaceError, type CanvasFile } from './types'
import type { ChatSession } from '@/components/excalidraw/use-chat-history'
import type { ExcalidrawWrapperRef } from '@/components/excalidraw/wrapper'
export const native = (tool: string, args: object = {}): Promise<unknown> =>
  invoke('workspace_command', { request: { tool, arguments: args } })

export class Documents {
  // Mutable maps track live editor handles and serialize persistence per canvas.
  readonly editors = new Map<string, ExcalidrawWrapperRef>()
  private readonly paused = new Set<string>()
  readonly timers = new Map<string, ReturnType<typeof setTimeout>>()
  private readonly writes = new Map<string, Promise<void>>()
  readonly access: {
    get: (id: string) => CanvasFile
    update: (id: string, patch: Partial<CanvasFile>) => void
  }
  constructor(access: Documents['access']) {
    this.access = access
  }
  async load(projectId: string, path: string): Promise<CanvasFile> {
    const file = fileSchema.parse(await native('read_canvas_file', { projectId, path }))
    const chats = chatSchema.parse(await native('read_chats', { projectId, path }))
    const scene = await loadFromBlob(new Blob([file.content]), null, null)
    return {
      tabId: crypto.randomUUID(),
      scene,
      id: canvasId(projectId, path),
      projectId,
      path,
      content: file.content,
      savedContent: file.content,
      revision: file.revision,
      chats,
      savedChats: JSON.stringify(chats),
      status: 'saved',
    }
  }
  scene(id: string, content: string): void {
    const document = this.access.get(id)
    if (content === document.content) return
    this.access.update(id, {
      content,
      status: document.status === 'conflict' ? 'conflict' : 'dirty',
    })
    this.schedule(id)
  }
  chats(id: string, chats: ChatSession[]): void {
    this.access.update(id, { chats })
    this.schedule(id)
  }
  pause(ids: readonly string[]): void {
    for (const id of ids) {
      this.paused.add(id)
      this.cancel(id)
    }
  }
  resume(ids: readonly string[], schedule = false): void {
    for (const id of ids) {
      this.paused.delete(id)
      if (schedule) this.schedule(id)
    }
  }
  cancel(id: string): void {
    clearTimeout(this.timers.get(id))
    this.timers.delete(id)
  }
  private schedule(id: string): void {
    if (this.paused.has(id)) return
    clearTimeout(this.timers.get(id))
    this.timers.set(
      id,
      setTimeout(() => {
        this.timers.delete(id)
        void this.flush(id).catch(() => {
          /* State exposes the persistent save error. */
        })
      }, 650),
    )
  }
  async flush(id: string): Promise<void> {
    clearTimeout(this.timers.get(id))
    this.timers.delete(id)
    const previous = this.writes.get(id) ?? Promise.resolve()
    const write = previous.then(
      () => this.persist(id),
      () => this.persist(id),
    )
    this.writes.set(id, write)
    try {
      await write
    } finally {
      if (this.writes.get(id) === write) this.writes.delete(id)
    }
    const latest = this.access.get(id)
    if (
      latest.content !== latest.savedContent ||
      JSON.stringify(latest.chats) !== latest.savedChats
    )
      await this.flush(id)
  }
  private async persist(id: string): Promise<void> {
    const file = this.access.get(id)
    if (file.status === 'conflict')
      throw new WorkspaceError(file.error ?? '文件冲突，请重新加载或另存副本')
    try {
      if (file.content !== file.savedContent) {
        this.access.update(id, { status: 'saving', error: undefined })
        const saved = z.object({ revision: z.string() }).parse(
          await native('save_canvas_file', {
            projectId: file.projectId,
            path: file.path,
            content: file.content,
            revision: file.revision,
          }),
        )
        this.access.update(id, { savedContent: file.content, revision: saved.revision })
      }
      const chats = JSON.stringify(file.chats)
      if (chats !== file.savedChats) {
        await native('write_chats', {
          projectId: file.projectId,
          path: file.path,
          chats: file.chats,
        })
        this.access.update(id, { savedChats: chats })
      }
      this.access.update(id, { status: 'saved', error: undefined })
    } catch (error) {
      const text = message(error)
      this.access.update(id, {
        status: text.includes('CONFLICT') ? 'conflict' : 'error',
        error: text,
      })
      throw new WorkspaceError(text)
    }
  }
  async ready(id: string): Promise<ExcalidrawWrapperRef> {
    for (let attempt = 0; attempt < 100; attempt++) {
      const editor = this.editors.get(id)
      if (editor?.isReady()) return editor
      await new Promise<void>((resolve) => setTimeout(resolve, 30))
    }
    throw new WorkspaceError('画布尚未就绪')
  }
}
