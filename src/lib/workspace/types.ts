import { z } from 'zod'
import type { ExcalidrawInitialDataState } from '@excalidraw/excalidraw/types'
import type { ChatSession } from '@/components/excalidraw/use-chat-history'
import { localActivitySchema, localReplyPartSchema } from '@/lib/local-chat'
import { aiProviderSchema } from '@/lib/ai-config'
export const projectSchema = z.object({ id: z.string(), name: z.string(), directory: z.string() })
export type Project = z.infer<typeof projectSchema>
export const entrySchema = z.object({
  path: z.string(),
  name: z.string(),
  kind: z.enum(['canvas', 'directory']),
})
export type FileEntry = z.infer<typeof entrySchema>
export const fileSchema = z.object({ content: z.string(), revision: z.string() })
export const chatSchema = z.array(
  z.object({
    id: z.string(),
    title: z.string(),
    messages: z.array(
      z.object({
        id: z.string(),
        role: z.enum(['user', 'assistant']),
        content: z.string(),
        timestamp: z.number(),
        thinking: z.string().optional(),
        activities: z.array(localActivitySchema).optional(),
        parts: z.array(localReplyPartSchema).optional(),
        source: aiProviderSchema.optional(),
        runId: z.string().uuid().optional(),
      }),
    ),
    createdAt: z.number(),
    updatedAt: z.number(),
    useIndependentCanvas: z.boolean().optional(),
  }),
)
export interface CanvasFile {
  readonly tabId: string
  readonly scene: ExcalidrawInitialDataState
  readonly id: string
  readonly projectId: string
  readonly path: string
  readonly content: string
  readonly savedContent: string
  readonly revision: string
  readonly chats: ChatSession[]
  readonly savedChats: string
  readonly status: 'saved' | 'dirty' | 'saving' | 'conflict' | 'error'
  readonly error?: string
}
export interface WorkspaceSnapshot {
  readonly projects: readonly Project[]
  readonly projectId: string | null
  readonly entries: readonly FileEntry[]
  readonly documents: readonly CanvasFile[]
  readonly activeId: string | null
  readonly presenting?: boolean
  readonly aiBusy: boolean
  readonly localAgentCanvasId?: string | null
  readonly busy: boolean
  readonly error: string | null
  readonly loading: string | null
  readonly initialized: boolean
}
export const canvasId = (projectId: string, path: string) => `${projectId}:${path}`
export class WorkspaceError extends Error {
  override readonly name = 'WorkspaceError'
}
export const message = (error: unknown) => (error instanceof Error ? error.message : String(error))
