import { useRef, useState, useCallback, useEffect, forwardRef, useImperativeHandle } from 'react'
import { Excalidraw, THEME, serializeAsJSON } from '@excalidraw/excalidraw'
import type {
  ExcalidrawImperativeAPI,
  AppState,
  BinaryFiles,
  ExcalidrawInitialDataState,
} from '@excalidraw/excalidraw/types'
import type { ExcalidrawElement } from '@excalidraw/excalidraw/element/types'
import type { ParsedElement } from './element-parser'
import { sceneMutations } from './scene-mutations'
import { sceneQueries } from './scene-queries'
import { summarizeElement } from './scene-summary'
import { createDebouncedWriter, removeStorage, writeStorage } from '@/lib/storage'
import { excalidrawLangCode, getLocale } from '@/lib/i18n'
import { persistDesktopCanvas } from '@/lib/desktop-storage'
import { parseAddLibraryFromHash, fetchLibraryItems } from './library'
import { getStorageKey, loadCanvasData } from './legacy-canvas'
import '@excalidraw/excalidraw/index.css'

export interface ElementSummary {
  id: string
  type: string
  text?: string
  x: number
  y: number
  width: number
  height: number
  strokeColor?: string
  backgroundColor?: string
  containerId?: string
}

export interface CanvasBounds {
  minX: number
  minY: number
  maxX: number
  maxY: number
}

export interface ExcalidrawWrapperRef {
  addElements: (elements: ParsedElement[]) => void
  clearCanvas: () => void
  getElements: () => readonly ExcalidrawElement[]
  getCanvasState: () => ElementSummary[]
  getSelectedElementsSummary: () => ElementSummary[]
  getCanvasBounds: () => CanvasBounds | null
  updateElements: (elements: ParsedElement[]) => { updated: string[]; notFound: string[] }
  deleteElements: (ids: string[]) => { deleted: string[]; notFound: string[] }
  scrollToElements: (ids?: string[]) => void
  switchToSession: (sessionId: string | null, useIndependentCanvas?: boolean) => void
  getCurrentSessionId: () => string | null
  isReady: () => boolean
  historyAction: (action: 'undo' | 'redo') => boolean
  flushCanvas: () => Promise<boolean>
}

export interface FileMode {
  readonly content: string
  readonly initialData: ExcalidrawInitialDataState
  readonly onChange: (content: string) => void
  readonly save: () => Promise<void>
}
interface ExcalidrawWrapperProps {
  fileMode?: FileMode
  className?: string
  onElementsChange?: (elements: readonly ExcalidrawElement[]) => void
  onSelectionChange?: (selectedElements: ElementSummary[]) => void
  onThemeChange?: (theme: 'light' | 'dark') => void
  zenModeEnabled?: boolean
  initialSessionId?: string | null
  langCode?: string
}

export const ExcalidrawWrapper = forwardRef<ExcalidrawWrapperRef, ExcalidrawWrapperProps>(
  function ExcalidrawWrapper(
    {
      className,
      onElementsChange,
      onSelectionChange,
      onThemeChange,
      zenModeEnabled = false,
      initialSessionId = null,
      langCode,
      fileMode,
    },
    ref,
  ) {
    const containerRef = useRef<HTMLDivElement>(null)
    const excalidrawAPIRef = useRef<ExcalidrawImperativeAPI | null>(null)
    const lastSelectedIdsRef = useRef<string>('')
    const currentSessionIdRef = useRef<string | null>(initialSessionId)
    const currentUseIndependentCanvasRef = useRef<boolean>(false)
    const writerRef = useRef(createDebouncedWriter(getStorageKey(initialSessionId, false), 500))
    const [initialData] = useState(() =>
      fileMode ? fileMode.initialData : loadCanvasData(initialSessionId, false),
    )
    const resolvedLang = langCode ?? excalidrawLangCode(getLocale())

    const resetWriter = useCallback((sessionId: string | null, independent: boolean) => {
      writerRef.current.flush()
      writerRef.current = createDebouncedWriter(getStorageKey(sessionId, independent), 500)
    }, [])

    useEffect(() => {
      const libraryParams = parseAddLibraryFromHash()
      if (!libraryParams) return

      const checkAndLoad = () => {
        const api = excalidrawAPIRef.current
        if (!api) {
          setTimeout(checkAndLoad, 200)
          return
        }

        api
          .updateLibrary({
            libraryItems: fetchLibraryItems(libraryParams.libraryUrl),
            merge: true,
            prompt: true,
            openLibraryMenu: true,
            defaultStatus: 'unpublished',
          })
          .then(() => {
            history.replaceState(null, '', window.location.pathname + window.location.search)
          })
          .catch((err: unknown) => {
            console.warn('Failed to load library from URL:', err)
          })
      }

      checkAndLoad()
    }, [])

    useEffect(() => {
      const flush = () => writerRef.current.flush()
      window.addEventListener('beforeunload', flush)
      return () => {
        flush()
        window.removeEventListener('beforeunload', flush)
      }
    }, [])

    useImperativeHandle(
      ref,
      () => ({
        ...sceneMutations(excalidrawAPIRef, () => {
          if (!fileMode)
            removeStorage(
              getStorageKey(currentSessionIdRef.current, currentUseIndependentCanvasRef.current),
            )
          writerRef.current.cancel()
        }),
        ...sceneQueries(excalidrawAPIRef),
        switchToSession: (sessionId: string | null, useIndependentCanvas = false) => {
          const api = excalidrawAPIRef.current
          if (!api) return
          if (fileMode) {
            currentSessionIdRef.current = sessionId
            return
          }

          const currentElements = api.getSceneElements()
          const activeElements = currentElements.filter((el) => !el.isDeleted)
          writerRef.current.write({ elements: activeElements })
          writerRef.current.flush()

          currentSessionIdRef.current = sessionId
          currentUseIndependentCanvasRef.current = useIndependentCanvas
          resetWriter(sessionId, useIndependentCanvas)

          const newData = loadCanvasData(sessionId, useIndependentCanvas)

          if (newData) {
            api.updateScene({ elements: newData.elements })
          } else if (useIndependentCanvas) {
            api.updateScene({ elements: [] })
          }
        },
        historyAction: (action) => {
          const button = containerRef.current?.querySelector<HTMLButtonElement>(
            `[data-testid="button-${action}"]`,
          )
          if (!button || button.disabled) return false
          button.click()
          return true
        },
        getCurrentSessionId: () => currentSessionIdRef.current,
        isReady: () => excalidrawAPIRef.current !== null,
        flushCanvas: async () => {
          const api = excalidrawAPIRef.current
          if (!api) return false
          if (fileMode) {
            fileMode.onChange(
              serializeAsJSON(api.getSceneElements(), api.getAppState(), api.getFiles(), 'local'),
            )
            await fileMode.save()
            return true
          }
          writerRef.current.cancel()
          const key = getStorageKey(
            currentSessionIdRef.current,
            currentUseIndependentCanvasRef.current,
          )
          const value = { elements: api.getSceneElements().filter((element) => !element.isDeleted) }
          if (!writeStorage(key, value)) return false
          await persistDesktopCanvas(key, value)
          return true
        },
      }),
      [resetWriter, fileMode],
    )

    const handleChange = useCallback(
      (elements: readonly ExcalidrawElement[], appState: AppState, files: BinaryFiles) => {
        const activeElements = elements.filter((el) => !el.isDeleted)
        if (fileMode) fileMode.onChange(serializeAsJSON(elements, appState, files, 'local'))
        else writerRef.current.write({ elements: activeElements })
        onElementsChange?.(activeElements)

        if (appState?.theme) {
          onThemeChange?.(appState.theme === THEME.DARK ? 'dark' : 'light')
        }

        if (appState?.selectedElementIds) {
          try {
            const currentKeys = Object.keys(appState.selectedElementIds).sort().join(',')
            if (currentKeys !== lastSelectedIdsRef.current) {
              lastSelectedIdsRef.current = currentKeys
              const api = excalidrawAPIRef.current
              if (api) {
                onSelectionChange?.(
                  elements
                    .filter((el) => appState.selectedElementIds[el.id])
                    .map((el) => summarizeElement(el, elements)),
                )
              }
            }
          } catch {
            // ignore
          }
        }
      },
      [onElementsChange, onSelectionChange, onThemeChange, fileMode],
    )

    return (
      <div className={className} ref={containerRef}>
        <Excalidraw
          excalidrawAPI={(api) => {
            excalidrawAPIRef.current = api
          }}
          initialData={initialData || undefined}
          onChange={handleChange}
          UIOptions={{
            canvasActions: {
              loadScene: !fileMode && !zenModeEnabled,
              saveToActiveFile: false,
              toggleTheme: true,
              clearCanvas: false,
              export: zenModeEnabled
                ? false
                : {
                    saveFileToDisk: true,
                  },
            },
          }}
          zenModeEnabled={zenModeEnabled}
          viewModeEnabled={false}
          langCode={resolvedLang}
        />
      </div>
    )
  },
)
