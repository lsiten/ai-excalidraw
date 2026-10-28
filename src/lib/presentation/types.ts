import { z } from 'zod'
import type { FileEntry } from '@/lib/workspace/types'

export const areaSchema = z
  .object({
    x: z.number().finite(),
    y: z.number().finite(),
    width: z.number().finite().positive(),
    height: z.number().finite().positive(),
  })
  .strict()
export const deckSchema = z
  .object({
    order: z.array(z.string()),
    includeSubfolders: z.boolean(),
  })
  .strict()
export const presentationSchema = z.object({
  decks: z.record(deckSchema),
  areas: z.record(areaSchema),
})
export type PlaybackArea = z.infer<typeof areaSchema>
export type DeckSettings = z.infer<typeof deckSchema>
export type PresentationData = z.infer<typeof presentationSchema>
export interface Slide {
  readonly path: string
  readonly url: string
  readonly width: number
  readonly height: number
}
export interface Deck {
  readonly projectId: string
  readonly folder: string
  readonly entries: readonly FileEntry[]
  readonly data: PresentationData
  readonly settings: DeckSettings
  readonly selected: string | null
}
export type PresentationState =
  | { readonly phase: 'closed' }
  | {
      readonly phase: 'setup'
      readonly deck: Deck
      readonly loading: string | null
      readonly error: string | null
    }
  | {
      readonly phase: 'playing'
      readonly slides: readonly Slide[]
      readonly index: number
      readonly title: string
    }

const natural = new Intl.Collator('zh-CN', { numeric: true, sensitivity: 'base' })
export function orderedPages(deck: Deck): string[] {
  const prefix = deck.folder ? `${deck.folder}/` : ''
  const pages = deck.entries
    .filter(
      (entry) =>
        entry.kind === 'canvas' &&
        entry.path.startsWith(prefix) &&
        (deck.settings.includeSubfolders || !entry.path.slice(prefix.length).includes('/')),
    )
    .map((entry) => entry.path)
    .sort(natural.compare)
  const available = new Set(pages)
  const saved = [...new Set(deck.settings.order)].filter((path) => available.has(path))
  const used = new Set(saved)
  return [...saved, ...pages.filter((path) => !used.has(path))]
}
export function movePage(order: readonly string[], from: string, to: string): string[] {
  const result = order.filter((path) => path !== from)
  const target = order.indexOf(to)
  if (target < 0 || !order.includes(from)) return [...order]
  result.splice(target, 0, from)
  return result
}
