import { isTauri } from '@tauri-apps/api/core'
import { useRef, useCallback, useState, useEffect } from 'react'
import { ExcalidrawWrapper, type ExcalidrawWrapperRef, type ElementSummary } from './wrapper'
import { ChatPanel } from './chat-panel'
import { MobileInput } from './mobile-input'
import { useChatHistory } from './use-chat-history'
import { useAiDraw } from './use-ai-draw'
import { Button } from '@/components/ui/button'
import { ConfirmDialog } from '@/components/ui/confirm-dialog'
import { ToastProvider, useToast } from '@/components/ui/toast'
import { Trash2, PanelRightOpen, Loader2 } from 'lucide-react'
import { cn } from '@/lib/utils'
import { SettingsDialog } from '@/components/settings-dialog'
import { useLocale } from '@/hooks/use-locale'
import { excalidrawLangCode } from '@/lib/i18n'
import type { FileMode } from './wrapper'
import type { ChatSession } from './use-chat-history'
import { useCanvasBridge } from '@/hooks/use-canvas-bridge'
import type { AIProjectContext } from '@/lib/local-chat'

interface ExcalidrawEditorProps {
  className?: string
  fileMode?: FileMode
  fileHistory?: {
    readonly initial: ChatSession[]
    readonly onChange: (sessions: ChatSession[]) => void
  }
  register?: (editor: ExcalidrawWrapperRef | null) => void
  active?: boolean
  onBusyChange?: (busy: boolean) => void
  projectContext?: AIProjectContext
}

function useIsMobile() {
  const [isMobile, setIsMobile] = useState(false)

  useEffect(() => {
    const checkMobile = () => setIsMobile(window.innerWidth < 768)
    checkMobile()
    window.addEventListener('resize', checkMobile)
    return () => window.removeEventListener('resize', checkMobile)
  }, [])

  return isMobile
}

function ExcalidrawEditorInner({
  className,
  fileMode,
  fileHistory,
  register,
  onBusyChange,
  active,
  projectContext,
}: ExcalidrawEditorProps) {
  const excalidrawRef = useRef<ExcalidrawWrapperRef>(null)
  useCanvasBridge(excalidrawRef, !!fileMode)
  const chatThreshold = fileMode ? 1380 : 1180
  const [isChatOpen, setIsChatOpen] = useState(() => window.innerWidth >= chatThreshold)
  const [confirmClearOpen, setConfirmClearOpen] = useState(false)
  const [selectedElements, setSelectedElements] = useState<ElementSummary[]>([])
  const [theme, setTheme] = useState<'light' | 'dark'>('light')
  const isMobile = useIsMobile()
  const { t, locale } = useLocale()
  const { showToast } = useToast()

  const chatHistory = useChatHistory(fileHistory)
  const aiDraw = useAiDraw({
    excalidrawRef,
    chatHistory,
    selectedElements,
    projectContext,
  })

  useEffect(() => {
    const resize = () => {
      if (window.innerWidth < chatThreshold) setIsChatOpen(false)
    }
    window.addEventListener('resize', resize)
    return () => window.removeEventListener('resize', resize)
  }, [chatThreshold])
  useEffect(() => {
    register?.(excalidrawRef.current)
    return () => register?.(null)
  }, [register])
  useEffect(() => {
    onBusyChange?.(aiDraw.isLoading)
    return () => onBusyChange?.(false)
  }, [aiDraw.isLoading, onBusyChange])
  useEffect(() => {
    if (active !== false) document.documentElement.classList.toggle('dark', theme === 'dark')
  }, [theme, active])

  const handleSelectionChange = useCallback((elements: ElementSummary[]) => {
    setSelectedElements(elements)
  }, [])

  const handleClearCanvas = useCallback(() => {
    setConfirmClearOpen(true)
  }, [])

  const confirmClear = useCallback(() => {
    excalidrawRef.current?.clearCanvas()
    showToast(t.clearCanvas, 1500)
  }, [showToast, t.clearCanvas])

  const toolbar = (
    <>
      {!isTauri() && <SettingsDialog />}
      {!isMobile && (
        <Button
          variant="ghost"
          size="sm"
          onClick={handleClearCanvas}
          className="gap-1.5 text-xs text-destructive hover:text-destructive hover:bg-destructive/10"
        >
          <Trash2 className="w-3.5 h-3.5" />
          {t.clearCanvas}
        </Button>
      )}
    </>
  )

  if (isMobile) {
    return (
      <div className={cn('flex flex-col h-full relative', className)}>
        <div className="absolute top-2 right-2 z-50">{toolbar}</div>

        {aiDraw.isLoading && (
          <div className="absolute top-2 left-1/2 -translate-x-1/2 z-50 animate-in fade-in slide-in-from-top-2 duration-300">
            <div className="flex items-center gap-2 px-4 py-2.5 rounded-full bg-primary text-primary-foreground shadow-lg">
              <Loader2 className="w-4 h-4 animate-spin" />
              <span className="text-sm font-medium">{t.aiDrawing}</span>
            </div>
          </div>
        )}

        <div className="flex-1 min-h-0">
          <ExcalidrawWrapper
            ref={excalidrawRef}
            fileMode={fileMode}
            className="h-full"
            zenModeEnabled={true}
            onSelectionChange={handleSelectionChange}
            onThemeChange={setTheme}
            langCode={excalidrawLangCode(locale)}
          />
        </div>

        <MobileInput
          excalidrawRef={excalidrawRef}
          chatHistory={chatHistory}
          aiDraw={aiDraw}
          selectedElements={selectedElements}
          onClearCanvas={handleClearCanvas}
        />

        <ConfirmDialog
          open={confirmClearOpen}
          onOpenChange={setConfirmClearOpen}
          title={t.clearCanvasConfirm}
          destructive
          onConfirm={confirmClear}
        />
      </div>
    )
  }

  return (
    <div className={cn('relative flex h-full', className)}>
      <div
        className={cn(
          'border-border bg-card flex shrink-0 flex-col order-last',
          fileMode &&
            'max-[1380px]:absolute max-[1380px]:inset-y-0 max-[1380px]:right-0 max-[1380px]:z-30 max-[1380px]:shadow-xl',
          isChatOpen ? 'w-[320px] border-l' : 'w-0',
        )}
      >
        {isChatOpen && (
          <ChatPanel
            className="flex-1 min-h-0"
            excalidrawRef={excalidrawRef}
            chatHistory={chatHistory}
            aiDraw={aiDraw}
            selectedElements={selectedElements}
            onClose={() => setIsChatOpen(false)}
          />
        )}
      </div>

      <div className="flex-1 flex flex-col min-w-0">
        <div className="flex items-center gap-2 px-3 py-2 border-b border-border bg-secondary/5">
          <div className="flex-1" />
          {toolbar}
          {!isChatOpen && (
            <Button
              variant="ghost"
              size="sm"
              className="gap-1.5 text-xs"
              onClick={() => setIsChatOpen(true)}
              title={t.openAIPanel}
              aria-label={t.openAIPanel}
            >
              <PanelRightOpen className="w-4 h-4" />
              {t.appTitle}
            </Button>
          )}
        </div>

        <ExcalidrawWrapper
          ref={excalidrawRef}
          fileMode={fileMode}
          className="flex-1"
          onSelectionChange={handleSelectionChange}
          onThemeChange={setTheme}
          langCode={excalidrawLangCode(locale)}
        />
      </div>

      <ConfirmDialog
        open={confirmClearOpen}
        onOpenChange={setConfirmClearOpen}
        title={t.clearCanvasConfirm}
        destructive
        onConfirm={confirmClear}
      />
    </div>
  )
}

export function ExcalidrawEditor(props: ExcalidrawEditorProps) {
  return (
    <ToastProvider>
      <ExcalidrawEditorInner {...props} />
    </ToastProvider>
  )
}

export default ExcalidrawEditor
