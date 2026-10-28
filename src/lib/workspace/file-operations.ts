import type { Workspace } from './store'
import { native } from './documents'
import { canvasId } from './types'
export interface FileMutationArguments {
  readonly projectId: string
  readonly path?: string
  readonly from?: string
  readonly to?: string
  readonly content?: string
  readonly source?: string
}
export class FileOperations {
  readonly workspace: Workspace
  constructor(workspace: Workspace) {
    this.workspace = workspace
  }
  async mutate(tool: string, args: FileMutationArguments): Promise<unknown> {
    const from = args.from
    const affected =
      tool === 'rename_file' && from
        ? this.workspace
            .getSnapshot()
            .documents.filter(
              (doc) =>
                doc.projectId === args.projectId &&
                (doc.path === from || doc.path.startsWith(`${from}/`)),
            )
            .map((doc) => doc.id)
        : []
    this.workspace.documents.pause(affected)
    let result: unknown
    try {
      await this.workspace.flushAll()
      result = await native(tool, args)
      if (tool === 'rename_file' && args.from && args.to) {
        const from = args.from,
          to = args.to
        let activeId = this.workspace.getSnapshot().activeId
        const documents = this.workspace.getSnapshot().documents.map((doc) => {
          if (
            doc.projectId !== args.projectId ||
            !(doc.path === from || doc.path.startsWith(`${from}/`))
          )
            return doc
          const path = to + doc.path.slice(from.length),
            id = canvasId(doc.projectId, path)
          if (activeId === doc.id) activeId = id
          // Rename preserves bytes/revision and the mounted editor, including late edits and undo history.
          return { ...doc, path, id }
        })
        this.workspace.set({ documents, activeId })
      }
    } catch (error) {
      this.workspace.documents.resume(affected, true)
      throw error
    }
    this.workspace.documents.resume(affected)
    if (tool === 'rename_file') await this.workspace.flushAll()
    if (tool === 'delete_file' && args.path) {
      const path = args.path
      const docs = this.workspace
        .getSnapshot()
        .documents.filter(
          (doc) =>
            doc.projectId !== args.projectId ||
            !(doc.path === path || doc.path.startsWith(`${path}/`)),
        )
      this.workspace.set({
        documents: docs,
        activeId: docs.some((doc) => doc.id === this.workspace.getSnapshot().activeId)
          ? this.workspace.getSnapshot().activeId
          : (docs.at(-1)?.id ?? null),
      })
    }
    await this.workspace.refresh()
    await this.workspace.persistSession()
    return result
  }
  async saveCopy(id: string, path: string): Promise<unknown> {
    const doc = this.workspace.file(id)
    await native('create_canvas', { projectId: doc.projectId, path, content: doc.content })
    await native('write_chats', { projectId: doc.projectId, path, chats: doc.chats })
    // Conflict stays explicit on the original tab, so no edits are silently discarded.
    this.workspace.set({ activeId: null })
    await this.workspace.openCanvas(doc.projectId, path)
    return { canvasId: canvasId(doc.projectId, path) }
  }
}
