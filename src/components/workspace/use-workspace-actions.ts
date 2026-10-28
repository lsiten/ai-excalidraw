import { useState } from 'react'
import { z } from 'zod'
import { workspace } from '@/lib/workspace/store'
import { native } from '@/lib/workspace/documents'
import {
  message,
  type CanvasFile,
  type FileEntry,
  type WorkspaceSnapshot,
} from '@/lib/workspace/types'
import type { FileAction } from './explorer'
import type { FileDialogRequest } from './file-dialog'

export function useWorkspaceActions(state: WorkspaceSnapshot, active: CanvasFile | undefined) {
  const [dialog, setDialog] = useState<FileDialogRequest | null>(null)
  const [deleting, setDeleting] = useState<FileEntry | null>(null)
  const run = (action: () => Promise<unknown>) => {
    void workspace.run(action).catch((error) => workspace.set({ error: message(error) }))
  }
  const pickProject = () =>
    run(async () => {
      const selected = z
        .object({ path: z.string() })
        .parse(await native('pick_path', { kind: 'directory' }))
      if (selected.path) await workspace.openProject(selected.path)
    })
  const pickFile = () =>
    run(async () => {
      const selected = z
        .object({ path: z.string() })
        .parse(await native('pick_path', { kind: 'file' }))
      if (!selected.path) return
      await workspace.openFilePath(selected.path)
    })
  const newCanvas = () => {
    const projectId = state.projectId
    if (!projectId) return
    setDialog({
      title: '新建画布',
      description: '画布将保存为项目目录中的文件。',
      initial: '未命名.excalidraw',
      submit: (path) =>
        workspace.run(async () => {
          await workspace.mutate('create_canvas', { projectId, path })
          await workspace.openCanvas(projectId, path)
        }),
    })
  }
  const newDirectory = () => {
    const projectId = state.projectId
    if (!projectId) return
    setDialog({
      title: '新建目录',
      description: '在当前项目中创建一个分类目录。',
      initial: '新目录',
      submit: (path) =>
        workspace.run(() => workspace.mutate('create_directory', { projectId, path })),
    })
  }
  const importFile = () =>
    run(async () => {
      const projectId = state.projectId
      if (!projectId) return
      const selected = z
        .object({ path: z.string() })
        .parse(await native('pick_path', { kind: 'file' }))
      if (!selected.path) return
      setDialog({
        title: '导入画布',
        description: '将所选文件复制到当前项目，原文件保留。',
        initial: selected.path.split('/').at(-1) ?? '导入.excalidraw',
        submit: (path) =>
          workspace.run(async () => {
            await workspace.mutate('import_canvas', { projectId, source: selected.path, path })
            await workspace.openCanvas(projectId, path)
          }),
      })
    })
  const copy = (conflict = false) => {
    if (!active) return
    setDialog({
      title: conflict ? '保存冲突副本' : '另存为画布',
      description: '在原文件旁创建独立副本，保留当前画布和对话。',
      initial: active.path.replace(/\.excalidraw$/, '-副本.excalidraw'),
      submit: (path) => workspace.run(() => workspace.saveCopy(active.id, path)),
    })
  }
  const action = (action: FileAction, entry: FileEntry) => {
    const projectId = state.projectId
    if (!projectId) return
    switch (action) {
      case 'rename':
        setDialog({
          title: '重命名或移动',
          description: '修改相对路径可移动到已有目录，对话记录会一并关联。',
          initial: entry.path,
          submit: (to) =>
            workspace.run(() =>
              workspace.mutate('rename_file', { projectId, from: entry.path, to }),
            ),
        })
        break
      case 'duplicate':
        setDialog({
          title: '复制画布',
          description: '创建一张内容相同的独立画布。',
          initial: entry.path.replace(/\.excalidraw$/, '-副本.excalidraw'),
          submit: (to) =>
            workspace.run(() =>
              workspace.mutate('duplicate_canvas', { projectId, from: entry.path, to }),
            ),
        })
        break
      case 'delete':
        setDeleting(entry)
        break
      case 'reveal':
        run(() => native('reveal_file', { projectId, path: entry.path }))
        break
    }
  }

  return {
    dialog,
    setDialog,
    deleting,
    setDeleting,
    run,
    pickProject,
    pickFile,
    newCanvas,
    newDirectory,
    importFile,
    copy,
    action,
  }
}
