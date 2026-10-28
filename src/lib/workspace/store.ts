import { z } from 'zod'
import { FileOperations, type FileMutationArguments } from './file-operations'
import { Documents, native } from './documents'
import {
  projectSchema,
  entrySchema,
  canvasId,
  WorkspaceError,
  message,
  type WorkspaceSnapshot,
  type CanvasFile,
} from './types'

export class Workspace {
  private readonly files = new FileOperations(this)
  private snapshot: WorkspaceSnapshot = {
    projects: [],
    projectId: null,
    entries: [],
    documents: [],
    activeId: null,
    aiBusy: false,
    busy: false,
    error: null,
    initialized: false,
    loading: '正在读取项目列表',
  }
  private readonly listeners = new Set<() => void>()
  readonly documents = new Documents({
    get: (id) => this.file(id),
    update: (id, patch) =>
      this.set({
        documents: this.snapshot.documents.map((doc) =>
          doc.id === id ? { ...doc, ...patch } : doc,
        ),
      }),
  })
  getSnapshot = () => this.snapshot
  subscribe = (listener: () => void) => {
    this.listeners.add(listener)
    return () => this.listeners.delete(listener)
  }
  set(patch: Partial<WorkspaceSnapshot>): void {
    this.snapshot = { ...this.snapshot, ...patch }
    this.listeners.forEach((listener) => listener())
  }
  file(id: string): CanvasFile {
    const file = this.snapshot.documents.find((doc) => doc.id === id)
    if (!file) throw new WorkspaceError('画布未打开')
    return file
  }
  async initialize(): Promise<void> {
    try {
      await this.refreshProjects()
      this.set({ loading: '正在读取上次打开的标签' })
      const session = z
        .object({
          projectId: z.string().nullable(),
          tabs: z.array(z.object({ projectId: z.string(), path: z.string() })),
          activeId: z.string().nullable(),
        })
        .nullable()
        .parse(await native('load_workspace_session'))
      if (session) {
        // Missing files are reported individually; remaining tabs can still be recovered.
        const recovered: CanvasFile[] = []
        for (const tab of session.tabs) {
          try {
            this.set({ loading: `正在读取画布：${tab.path}` })
            recovered.push(await this.documents.load(tab.projectId, tab.path))
          } catch (error) {
            this.set({ error: `无法恢复 ${tab.path}：${message(error)}` })
          }
        }
        this.set({ loading: '正在准备编辑器', documents: recovered })
        this.set({
          projectId: session.projectId,
          activeId: this.snapshot.documents.some((doc) => doc.id === session.activeId)
            ? session.activeId
            : this.snapshot.activeId,
        })
      }
      if (!this.snapshot.projectId) this.set({ projectId: this.snapshot.projects[0]?.id ?? null })
      this.set({ loading: '正在读取项目目录' })
      await this.refresh()
    } catch (error) {
      this.set({ error: message(error) })
    }
    this.set({ initialized: true, loading: null })
  }
  async refreshProjects(): Promise<void> {
    this.set({
      projects: z.object({ projects: z.array(projectSchema) }).parse(await native('list_projects'))
        .projects,
    })
  }
  async refresh(): Promise<void> {
    const projectId = this.snapshot.projectId
    if (projectId)
      this.set({
        entries: z
          .object({ entries: z.array(entrySchema) })
          .parse(await native('list_files', { projectId })).entries,
      })
  }
  async persistSession(): Promise<void> {
    await native('save_workspace_session', {
      session: {
        projectId: this.snapshot.projectId,
        activeId: this.snapshot.activeId,
        tabs: this.snapshot.documents.map(({ projectId, path }) => ({ projectId, path })),
      },
    })
  }
  private async saveDocument(id: string): Promise<void> {
    const editor = this.documents.editors.get(id)
    if (editor?.isReady() && !(await editor.flushCanvas()))
      throw new WorkspaceError('画布还未完成保存')
    await this.documents.flush(id)
  }
  async flushAll(): Promise<void> {
    do {
      for (const doc of this.snapshot.documents) await this.saveDocument(doc.id)
      await this.persistSession()
    } while (
      this.snapshot.documents.some(
        (doc) => doc.content !== doc.savedContent || JSON.stringify(doc.chats) !== doc.savedChats,
      )
    )
  }
  async openProject(directory: string): Promise<unknown> {
    await this.flushAll()
    const project = projectSchema.parse(await native('open_project', { directory }))
    await this.refreshProjects()
    this.set({
      projectId: project.id,
      activeId: this.snapshot.documents.find((doc) => doc.projectId === project.id)?.id ?? null,
    })
    await this.refresh()
    await this.persistSession()
    return project
  }
  async openFilePath(absolutePath: string): Promise<{ canvasId: string }> {
    if (!absolutePath.startsWith('/') || !absolutePath.endsWith('.excalidraw'))
      throw new WorkspaceError('请选择本地 .excalidraw 文件')
    const project = [...this.snapshot.projects]
      .sort((a, b) => b.directory.length - a.directory.length)
      .find((item) => absolutePath.startsWith(`${item.directory}/`))
    if (project)
      return this.openCanvas(project.id, absolutePath.slice(project.directory.length + 1))
    const index = absolutePath.lastIndexOf('/')
    const opened = projectSchema.parse(await this.openProject(absolutePath.slice(0, index)))
    return this.openCanvas(opened.id, absolutePath.slice(index + 1))
  }
  async selectProject(projectId: string): Promise<void> {
    await this.flushAll()
    this.set({
      projectId,
      activeId: this.snapshot.documents.find((doc) => doc.projectId === projectId)?.id ?? null,
    })
    await this.refresh()
    await this.persistSession()
  }
  async openCanvas(projectId: string, path: string, persist = true): Promise<{ canvasId: string }> {
    const id = canvasId(projectId, path)
    if (this.snapshot.activeId && this.snapshot.activeId !== id)
      await this.documents.flush(this.snapshot.activeId)
    if (!this.snapshot.documents.some((doc) => doc.id === id))
      this.set({
        documents: [...this.snapshot.documents, await this.documents.load(projectId, path)],
      })
    this.set({ activeId: id, projectId })
    await this.refresh()
    if (persist) await this.persistSession()
    return { canvasId: id }
  }
  async activate(id: string): Promise<void> {
    const doc = this.file(id)
    await this.openCanvas(doc.projectId, doc.path)
  }
  async close(id: string, discard = false): Promise<void> {
    if (discard) this.documents.cancel(id)
    else await this.saveDocument(id)
    const docs = this.snapshot.documents.filter((doc) => doc.id !== id)
    this.set({
      documents: docs,
      activeId: this.snapshot.activeId === id ? (docs.at(-1)?.id ?? null) : this.snapshot.activeId,
    })
    await this.persistSession()
  }
  async reload(id: string): Promise<void> {
    const file = this.file(id)
    clearTimeout(this.documents.timers.get(id))
    const next = await this.documents.load(file.projectId, file.path)
    this.set({ documents: this.snapshot.documents.filter((doc) => doc.id !== id) })
    await new Promise<void>((resolve) => setTimeout(resolve, 30))
    this.set({ documents: [...this.snapshot.documents, next], activeId: id })
  }
  async checkExternal(refreshTree = false): Promise<void> {
    for (const file of this.snapshot.documents) {
      if (file.status === 'saving' || file.status === 'conflict') continue
      try {
        const current = z
          .object({ revision: z.string(), content: z.string() })
          .parse(await native('read_canvas_file', { projectId: file.projectId, path: file.path }))
        if (
          current.revision !== this.file(file.id).revision &&
          file.revision === this.file(file.id).revision &&
          this.file(file.id).status !== 'saving'
        )
          this.documents.access.update(file.id, {
            status: 'conflict',
            error: '文件已被外部修改。请重新加载或另存副本。',
          })
      } catch (error) {
        this.documents.access.update(file.id, {
          status: 'conflict',
          error: `文件读取失败：${message(error)}。可另存副本保留当前内容。`,
        })
      }
    }
    if (refreshTree) await this.refresh()
  }
  async mutate(tool: string, args: FileMutationArguments): Promise<unknown> {
    return this.files.mutate(tool, args)
  }
  async saveCopy(id: string, path: string): Promise<unknown> {
    return this.files.saveCopy(id, path)
  }
  async run<T>(action: () => Promise<T>, agentCanvasId?: string): Promise<T> {
    if (!this.snapshot.initialized) throw new WorkspaceError('正在恢复项目，请稍后重试')
    if (this.snapshot.presenting) throw new WorkspaceError('请先退出演示模式，再操作项目或画布')
    const ownAgent = !!agentCanvasId && agentCanvasId === this.snapshot.localAgentCanvasId
    if (this.snapshot.busy || (this.snapshot.aiBusy && !ownAgent))
      throw new WorkspaceError('应用正在处理操作，请稍后重试')
    this.set({ busy: true, error: null })
    try {
      return await action()
    } catch (error) {
      this.set({ error: message(error) })
      throw error
    } finally {
      this.set({ busy: false })
    }
  }
}
export const workspace = new Workspace()
