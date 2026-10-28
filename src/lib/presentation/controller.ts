import { z } from 'zod'
import { isTauri } from '@tauri-apps/api/core'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { workspace } from '@/lib/workspace/store'
import { native } from '@/lib/workspace/documents'
import { entrySchema, message, WorkspaceError } from '@/lib/workspace/types'
import {
  presentationSchema,
  orderedPages,
  type Deck,
  type PlaybackArea,
  type PresentationState,
  type Slide,
} from './types'
import { renderSlide, selectionArea } from './render'

export class PresentationController {
  private state: PresentationState = { phase: 'closed' }
  private readonly listeners = new Set<() => void>()
  private fullscreenBefore = false
  private generation = 0
  getSnapshot = () => this.state
  subscribe = (listener: () => void) => {
    this.listeners.add(listener)
    return () => this.listeners.delete(listener)
  }
  private set(state: PresentationState) {
    this.state = state
    this.listeners.forEach((listener) => listener())
  }
  async prepare(projectId: string, folder: string): Promise<void> {
    await workspace.run(async () => {
      await workspace.flushAll()
      const data = presentationSchema.parse(await native('read_presentation', { projectId }))
      const { entries } = z
        .object({ entries: z.array(entrySchema) })
        .parse(await native('list_files', { projectId }))
      const active = workspace
        .getSnapshot()
        .documents.find((file) => file.id === workspace.getSnapshot().activeId)
      const deck: Deck = {
        projectId,
        folder,
        entries,
        data,
        settings: data.decks[folder] ?? { order: [], includeSubfolders: false },
        selected: active?.projectId === projectId ? active.path : null,
      }
      this.set({ phase: 'setup', deck, loading: null, error: null })
    })
  }
  update(deck: Deck) {
    if (this.state.phase === 'setup' && !this.state.loading)
      this.set({ ...this.state, deck, error: null })
  }
  async save(deck: Deck): Promise<void> {
    await workspace.run(async () => {
      // Always merge the latest metadata, including another folder's edits and canvas renames.
      const latest = presentationSchema.parse(
        await native('read_presentation', { projectId: deck.projectId }),
      )
      await native('write_presentation', {
        projectId: deck.projectId,
        presentation: {
          ...latest,
          decks: {
            ...latest.decks,
            [deck.folder]: { ...deck.settings, order: orderedPages(deck) },
          },
        },
      })
    })
  }
  async start(deck: Deck, from: string | null): Promise<void> {
    if (this.state.phase !== 'setup' || this.state.loading) return
    const token = ++this.generation
    const slides: Slide[] = []
    this.set({ phase: 'setup', deck, loading: '正在保存演示顺序…', error: null })
    try {
      await this.save(deck)
      await workspace.run(async () => {
        await workspace.flushAll()
        const data = presentationSchema.parse(
          await native('read_presentation', { projectId: deck.projectId }),
        )
        const pages = orderedPages(deck)
        if (!pages.length) throw new WorkspaceError('此文件夹没有可播放的画布')
        for (const [index, path] of pages.entries()) {
          if (token !== this.generation) throw new WorkspaceError('演示准备已取消')
          this.set({
            phase: 'setup',
            deck,
            loading: `正在准备第 ${index + 1} / ${pages.length} 页`,
            error: null,
          })
          slides.push(await renderSlide(deck.projectId, path, data.areas[path]))
        }
        if (token !== this.generation) throw new WorkspaceError('演示准备已取消')
        this.fullscreenBefore = isTauri()
          ? await getCurrentWindow().isFullscreen()
          : !!document.fullscreenElement
        if (isTauri()) await getCurrentWindow().setFullscreen(true)
        else if (document.documentElement.requestFullscreen)
          await document.documentElement.requestFullscreen()
        if (token !== this.generation) {
          if (isTauri()) await getCurrentWindow().setFullscreen(this.fullscreenBefore)
          throw new WorkspaceError('演示准备已取消')
        }
        workspace.set({ presenting: true })
        this.set({
          phase: 'playing',
          slides,
          index: Math.max(0, pages.indexOf(from ?? '')),
          title: deck.folder.split('/').at(-1) || '项目演示',
        })
      })
    } catch (error) {
      slides.forEach((slide) => URL.revokeObjectURL(slide.url))
      if (token === this.generation)
        this.set({ phase: 'setup', deck, loading: null, error: message(error) })
    }
  }
  go(index: number) {
    if (this.state.phase === 'playing')
      this.set({ ...this.state, index: Math.max(0, Math.min(this.state.slides.length - 1, index)) })
  }
  async stop(): Promise<void> {
    ++this.generation
    const current = this.state
    this.set({ phase: 'closed' })
    workspace.set({ presenting: false })
    if (current.phase === 'playing') {
      current.slides.forEach((slide) => URL.revokeObjectURL(slide.url))
      if (isTauri()) await getCurrentWindow().setFullscreen(this.fullscreenBefore)
      else if (!this.fullscreenBefore && document.fullscreenElement) await document.exitFullscreen()
    }
  }
  async setArea(projectId: string, path: string, area: PlaybackArea | null): Promise<void> {
    await workspace.run(async () => {
      const data = presentationSchema.parse(await native('read_presentation', { projectId }))
      if (area) data.areas[path] = area
      else delete data.areas[path]
      await native('write_presentation', { projectId, presentation: data })
    })
  }
  async selection(): Promise<void> {
    const state = workspace.getSnapshot(),
      file = state.documents.find((doc) => doc.id === state.activeId)
    if (!file) throw new WorkspaceError('请先打开画布')
    const editor = await workspace.documents.ready(file.id)
    const area = selectionArea(
      editor.getElements(),
      editor.getSelectedElementsSummary().map((element) => element.id),
    )
    await this.setArea(file.projectId, file.path, area)
  }
}
export const presentations = new PresentationController()
