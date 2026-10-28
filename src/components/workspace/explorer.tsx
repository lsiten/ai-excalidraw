import { useState } from 'react'
import {
  Folder,
  FolderOpen,
  FilePenLine,
  MoreHorizontal,
  Search,
  ChevronRight,
  ChevronDown,
  PanelLeftClose,
} from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { ExplorerToolbar } from './explorer-toolbar'
import type { WorkspaceSnapshot, FileEntry } from '@/lib/workspace/types'
export type FileAction = 'rename' | 'duplicate' | 'delete' | 'reveal'
interface Props {
  readonly state: WorkspaceSnapshot
  readonly openProject: () => void
  readonly selectProject: (id: string) => void
  readonly openFile: (path: string) => void
  readonly newCanvas: () => void
  readonly newDirectory: () => void
  readonly importFile: () => void
  readonly refresh: () => void
  readonly action: (action: FileAction, entry: FileEntry) => void
  readonly onPresent: (folder: string) => void
  readonly collapse: () => void
}
export function Explorer(props: Props) {
  const { state } = props
  const project = state.projects.find((p) => p.id === state.projectId)
  const [query, setQuery] = useState('')
  const [collapsed, setCollapsed] = useState<Set<string>>(new Set())
  const [menu, setMenu] = useState<string | null>(null)
  const disabled = !state.initialized || state.busy || state.aiBusy
  const active = state.documents.find((doc) => doc.id === state.activeId)
  const entries = state.entries.filter((entry) =>
    query
      ? entry.path.toLocaleLowerCase().includes(query.toLocaleLowerCase())
      : ![...collapsed].some((folder) => entry.path.startsWith(`${folder}/`)),
  )
  return (
    <aside
      className="workspace-explorer w-full min-w-0 shrink-0 border-r border-border bg-card flex flex-col"
      aria-label="项目文件"
    >
      <header className="flex items-center justify-between px-4 h-14 border-b border-border">
        <span className="font-semibold text-sm">AI Excalidraw</span>
        <Button
          variant="ghost"
          size="icon"
          title="收起文件栏"
          aria-label="收起文件栏"
          onClick={props.collapse}
        >
          <PanelLeftClose size={16} />
        </Button>
      </header>
      <div className="p-3 space-y-2">
        <Button
          variant="outline"
          className="w-full justify-start gap-2"
          onClick={props.openProject}
          disabled={disabled}
        >
          <FolderOpen size={16} />
          打开本地项目
        </Button>
        {project && (
          <>
            <label htmlFor="workspace-project" className="text-xs text-muted-foreground block pt-2">
              当前项目
            </label>
            <select
              id="workspace-project"
              aria-label="当前项目"
              className="w-full rounded-md border border-input bg-background px-2 py-2 text-sm"
              value={state.projectId ?? ''}
              onChange={(event) => props.selectProject(event.target.value)}
              disabled={disabled}
            >
              {state.projects.map((p) => (
                <option key={p.id} value={p.id}>
                  {p.name}
                </option>
              ))}
            </select>
            <p className="text-[11px] text-muted-foreground truncate" title={project.directory}>
              {project.directory}
            </p>
          </>
        )}
      </div>
      {project && (
        <>
          <ExplorerToolbar
            disabled={disabled}
            newCanvas={props.newCanvas}
            newDirectory={props.newDirectory}
            importFile={props.importFile}
            refresh={props.refresh}
          />
          <div className="relative px-3 pb-3">
            <Search className="absolute left-5 top-2.5 text-muted-foreground" size={14} />
            <Input
              className="pl-7 h-9 text-xs"
              placeholder="查找画布或目录"
              aria-label="查找画布或目录"
              value={query}
              onChange={(event) => setQuery(event.target.value)}
            />
          </div>
        </>
      )}
      <nav className="flex-1 overflow-auto px-2 pb-4" aria-label="画布文件树">
        {project && !entries.length && (
          <p className="text-xs leading-6 text-muted-foreground px-3 py-8">
            {query ? '没有匹配的文件。' : '项目还没有画布。点击上方 + 创建第一张。'}
          </p>
        )}
        {entries.map((entry) => {
          const directory = entry.kind === 'directory'
          const selected = active?.projectId === state.projectId && active.path === entry.path
          return (
            <div
              key={entry.path}
              className="relative"
              onContextMenu={(event) => {
                event.preventDefault()
                if (!disabled) setMenu(entry.path)
              }}
            >
              <div
                className={`group flex items-center rounded-md mb-0.5 ${selected ? 'bg-primary/10 text-primary' : 'hover:bg-muted'}`}
              >
                <button
                  className="min-w-0 flex-1 flex items-center gap-1.5 py-2 text-left text-xs rounded-md focus-visible:outline-2 focus-visible:outline-ring"
                  style={{ paddingLeft: 8 + (query ? 0 : entry.path.split('/').length - 1) * 14 }}
                  title={entry.path}
                  disabled={disabled}
                  aria-current={selected ? 'page' : undefined}
                  aria-expanded={directory ? !collapsed.has(entry.path) : undefined}
                  onClick={() => {
                    setMenu(null)
                    if (directory)
                      setCollapsed((previous) => {
                        const next = new Set(previous)
                        if (next.has(entry.path)) next.delete(entry.path)
                        else next.add(entry.path)
                        return next
                      })
                    else props.openFile(entry.path)
                  }}
                >
                  {directory ? (
                    collapsed.has(entry.path) ? (
                      <ChevronRight size={12} />
                    ) : (
                      <ChevronDown size={12} />
                    )
                  ) : (
                    <span className="w-3 shrink-0" />
                  )}
                  {directory ? (
                    <Folder size={15} className="shrink-0 text-muted-foreground" />
                  ) : (
                    <FilePenLine size={15} className="shrink-0" />
                  )}
                  <span className="truncate">
                    {query ? entry.path : entry.name.replace(/\.excalidraw$/, '')}
                  </span>
                </button>
                <button
                  className="p-1.5 rounded hover:bg-muted shrink-0"
                  aria-label={`${entry.name}的操作`}
                  title="文件操作"
                  disabled={disabled}
                  onClick={() => setMenu(menu === entry.path ? null : entry.path)}
                >
                  <MoreHorizontal size={14} />
                </button>
              </div>
              {menu === entry.path && (
                <div
                  className="absolute right-0 top-8 z-40 bg-card border border-border rounded-lg shadow-lg p-1 min-w-32"
                  onKeyDown={(event) => {
                    if (event.key === 'Escape') setMenu(null)
                  }}
                >
                  {directory && (
                    <button
                      className="block w-full rounded px-3 py-2 text-left text-xs hover:bg-muted"
                      onClick={() => {
                        setMenu(null)
                        props.onPresent(entry.path)
                      }}
                    >
                      播放此文件夹
                    </button>
                  )}
                  {(
                    [
                      'rename',
                      ...(!directory ? (['duplicate'] as const) : []),
                      'reveal',
                      'delete',
                    ] as const
                  ).map((action) => (
                    <button
                      key={action}
                      className={`block w-full text-left px-3 py-2 text-xs rounded hover:bg-muted ${action === 'delete' ? 'text-destructive' : ''}`}
                      onClick={() => {
                        setMenu(null)
                        props.action(action, entry)
                      }}
                    >
                      {
                        {
                          rename: '重命名 / 移动',
                          duplicate: '复制画布',
                          reveal: '在 Finder 中显示',
                          delete: '移到废纸篓',
                        }[action]
                      }
                    </button>
                  ))}
                </div>
              )}
            </div>
          )
        })}
      </nav>
      <footer className="px-4 py-3 border-t border-border text-[11px] text-muted-foreground">
        .excalidraw 文件直接保存在本地
      </footer>
    </aside>
  )
}
