import { z } from 'zod'
import { workspace } from './store'
import { native } from './documents'
import { canvasCommandSchema, executeCanvasCommand } from '@/lib/canvas-commands'
import { WorkspaceError } from './types'
import { presentationSchema } from '@/lib/presentation/types'
import { canvasGateway } from '@/lib/agent/canvas-gateway'
import { projectMcpTools, executeProjectMcp } from '@/lib/agent/mcp-tools'
const projectId = z.string().min(1),
  path = z.string().min(1).max(2000)
const schemas = {
  get_workspace: z.object({}).strict(),
  list_projects: z.object({}).strict(),
  open_project: z.object({ directory: path }).strict(),
  list_files: z.object({ projectId }).strict(),
  get_presentation: z.object({ projectId }).strict(),
  set_presentation: z.object({ projectId, presentation: presentationSchema.strict() }).strict(),
  create_canvas: z.object({ projectId, path }).strict(),
  open_canvas: z.object({ projectId, path }).strict(),
  rename_file: z.object({ projectId, from: path, to: path }).strict(),
  duplicate_canvas: z.object({ projectId, from: path, to: path }).strict(),
  create_directory: z.object({ projectId, path }).strict(),
  delete_file: z.object({ projectId, path }).strict(),
  reveal_file: z.object({ projectId, path: path.optional() }).strict(),
}
export async function workspaceCommand(input: unknown, operationId?: string): Promise<unknown> {
  const command = z
    .object({ tool: z.string(), arguments: z.record(z.unknown()).default({}) })
    .strict()
    .parse(input)
  if (command.tool === 'get_workspace') {
    schemas.get_workspace.parse(command.arguments)
    const state = workspace.getSnapshot()
    return {
      projects: state.projects,
      projectId: state.projectId,
      activeCanvasId: state.activeId,
      initialized: state.initialized,
      loading: state.loading,
      busy: state.busy || state.aiBusy,
      localAgentCanvasId: state.localAgentCanvasId ?? null,
      presenting: state.presenting ?? false,
      error: state.error,
      canvases: state.documents.map((doc) => ({
        canvasId: doc.id,
        projectId: doc.projectId,
        path: doc.path,
        status: doc.status,
      })),
    }
  }
  if (projectMcpTools.has(command.tool)) return executeProjectMcp(command.tool, command.arguments, operationId ?? crypto.randomUUID())
  const snapshot = workspace.getSnapshot()
  const target = z.object({ canvasId: z.string().optional() }).passthrough().parse(command.arguments)
  const agentProjectId = snapshot.documents.find((document) => document.id === snapshot.localAgentCanvasId)?.projectId
  const scopedRead = command.tool === 'list_files' && command.arguments.projectId === agentProjectId
  const scopedCanvas = target.canvasId === snapshot.localAgentCanvasId &&
    ['get_canvas', 'add_elements', 'update_elements', 'delete_elements', 'fit_canvas'].includes(command.tool)
  const agentCanvasId = snapshot.localAgentCanvasId && (scopedRead || scopedCanvas)
    ? snapshot.localAgentCanvasId : undefined
  return workspace.run(async () => {
    const state = workspace.getSnapshot()
    switch (command.tool) {
      case 'list_projects':
        schemas.list_projects.parse(command.arguments)
        return native('list_projects')
      case 'open_project': {
        const args = schemas.open_project.parse(command.arguments)
        return workspace.openProject(args.directory)
      }
      case 'list_files':
        return native('list_files', schemas.list_files.parse(command.arguments))
      case 'get_presentation':
        return native('read_presentation', schemas.get_presentation.parse(command.arguments))
      case 'set_presentation':
        return native('write_presentation', schemas.set_presentation.parse(command.arguments))
      case 'open_canvas': {
        const args = schemas.open_canvas.parse(command.arguments)
        return workspace.openCanvas(args.projectId, args.path)
      }
      case 'create_canvas': {
        const args = schemas.create_canvas.parse(command.arguments)
        await workspace.mutate('create_canvas', args)
        return workspace.openCanvas(args.projectId, args.path)
      }
      case 'rename_file':
        return workspace.mutate(command.tool, schemas.rename_file.parse(command.arguments))
      case 'duplicate_canvas':
        return workspace.mutate(command.tool, schemas.duplicate_canvas.parse(command.arguments))
      case 'create_directory':
        return workspace.mutate(command.tool, schemas.create_directory.parse(command.arguments))
      case 'delete_file':
        return workspace.mutate(command.tool, schemas.delete_file.parse(command.arguments))
      case 'reveal_file':
        return native(command.tool, schemas.reveal_file.parse(command.arguments))
      default: {
        const target = z
          .object({ canvasId: z.string().optional() })
          .passthrough()
          .parse(command.arguments)
        const id = target.canvasId ?? state.activeId
        if (!id) throw new WorkspaceError('请先打开项目和画布（open_project / open_canvas）')
        if (command.tool !== 'get_canvas' && !target.canvasId)
          throw new WorkspaceError(
            '写入和视图操作必须提供 canvasId；先 get_canvas 或 open_canvas 获取',
          )
        const { canvasId: ignored, ...args } = command.arguments
        void ignored
        const parsed = canvasCommandSchema.parse({ tool: command.tool, arguments: args })
        const file = workspace.file(id)
        if (file.status === 'conflict' || file.status === 'error') {
          if (parsed.tool !== 'get_canvas')
            throw new WorkspaceError(file.error ?? '先处理文件保存问题')
        }
        const editor = await workspace.documents.ready(id)
        const gateway = canvasGateway(id)
        if (gateway && parsed.tool !== 'get_canvas' && parsed.tool !== 'fit_canvas') {
          const result = await gateway.execute(parsed.tool, parsed.arguments, operationId)
          return { canvasId: id, ...z.record(z.unknown()).parse(result) }
        }
        if (parsed.tool === 'undo' || parsed.tool === 'redo') await workspace.activate(id)
        const result = executeCanvasCommand(editor, parsed)
        // A timer runs in background WebViews too; rAF can suspend indefinitely there.
        if (parsed.tool !== 'get_canvas' && parsed.tool !== 'fit_canvas') {
          await new Promise<void>((resolve) => setTimeout(resolve, 50))
          await editor.flushCanvas()
        }
        if (parsed.tool === 'get_canvas')
          return {
            canvasId: id,
            projectId: file.projectId,
            path: file.path,
            status: file.status,
            ...z.record(z.unknown()).parse(result),
          }
        return { canvasId: id, ...z.record(z.unknown()).parse(result) }
      }
    }
  }, agentCanvasId)
}
