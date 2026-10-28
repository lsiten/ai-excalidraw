import { isTauri } from '@tauri-apps/api/core'
import { WorkspaceShell } from '@/components/workspace'
import { ExcalidrawEditor } from '@/components/excalidraw'

function App() {
  return (
    <div className="h-screen w-screen overflow-hidden">
      {isTauri() ? <WorkspaceShell /> : <ExcalidrawEditor className="h-full" />}
    </div>
  )
}

export default App
