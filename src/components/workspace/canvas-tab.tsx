import { useCallback, useMemo, useState } from 'react'
import { ExcalidrawEditor } from '@/components/excalidraw'
import type { ExcalidrawWrapperRef } from '@/components/excalidraw/wrapper'
import type { CanvasFile } from '@/lib/workspace/types'
import type { ChatSession } from '@/components/excalidraw/use-chat-history'
import { workspace } from '@/lib/workspace/store'
export function CanvasTab({
  file,
  active,
  locked,
}: {
  readonly file: CanvasFile
  readonly active: boolean
  readonly locked: boolean
}) {
  const register = useCallback(
    (editor: ExcalidrawWrapperRef | null) => {
      if (editor) workspace.documents.editors.set(file.id, editor)
      else workspace.documents.editors.delete(file.id)
    },
    [file.id],
  )
  const onChange = useCallback(
    (content: string) => workspace.documents.scene(file.id, content),
    [file.id],
  )
  const save = useCallback(() => workspace.documents.flush(file.id), [file.id])
  const fileMode = useMemo(
    () => ({ content: file.content, initialData: file.scene, onChange, save }),
    [file.content, file.scene, onChange, save],
  )
  const onChats = useCallback(
    (sessions: ChatSession[]) => workspace.documents.chats(file.id, sessions),
    [file.id],
  )
  // Initial history is captured at mount; subsequent writes never feed back into initialization.
  const [initialChats] = useState(file.chats)
  const history = useMemo(
    () => ({ initial: initialChats, onChange: onChats }),
    [initialChats, onChats],
  )
  const onBusyChange = useCallback((busy: boolean) => workspace.set({ aiBusy: busy }), [])
  return (
    <div
      className="workspace-canvas h-full"
      style={{ display: active ? 'block' : 'none' }}
      inert={!active || locked}
      aria-hidden={!active}
    >
      <ExcalidrawEditor
        className="h-full"
        active={active}
        fileMode={fileMode}
        fileHistory={history}
        register={register}
        onBusyChange={onBusyChange}
        projectContext={{
          projectId: file.projectId,
          canvasId: file.id,
          path: file.path,
          directory: workspace.getSnapshot().projects.find((project) => project.id === file.projectId)?.directory ?? '',
        }}
      />
    </div>
  )
}
