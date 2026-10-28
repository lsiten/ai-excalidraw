import { expect, it } from 'vitest'
import { appendReplyPart, failChatReply, splitReplyParts } from './chat-reply'
import type { LocalReplyPart } from './local-chat'
import { chatSchema } from './workspace/types'

it('retains partial text, thinking, completed tools and the failure after save and reload', () => {
  // Given
  const partial = {
    content: 'Partial answer', thinking: 'Planning', source: 'codex' as const,
    activities: [
      { type: 'tool', id: 'read', name: 'get_canvas', status: 'completed', output: 'scene data' },
      { type: 'tool', id: 'write', name: 'add_elements', status: 'running', input: 'pending change' },
    ] as const,
  }
  // When
  const failed = failChatReply(partial, 'Connection lost')
  const restored = chatSchema.parse(JSON.parse(JSON.stringify([{ id: 'session', title: 'Chat', createdAt: 1, updatedAt: 2,
    messages: [{ id: 'answer', role: 'assistant', timestamp: 1, ...failed }] }])))
  // Then
  expect(restored[0]?.messages[0]).toMatchObject({ content: 'Partial answer', thinking: 'Planning', source: 'codex', activities: [
    { id: 'read', status: 'completed', output: 'scene data' },
    { id: 'write', status: 'failed', input: 'pending change', output: 'Connection lost' },
    { type: 'status', message: 'Connection lost' },
  ] })
})

it('keeps the latest answer outside execution details when trailing status events arrive', () => {
  // Given
  const parts: readonly LocalReplyPart[] = [
    { type: 'thinking', text: 'Inspect' },
    { type: 'chunk', text: 'A flowchart.' },
    { type: 'status', message: 'Completed' },
    { type: 'status', level: 'error', message: 'Connection lost after output' },
  ]
  // When
  const split = splitReplyParts(parts)
  // Then
  expect(split.answer).toBe('A flowchart.')
  expect(split.progress).toEqual([parts[0], parts[2], parts[3]])
})

it('keeps pre-tool narration inside the execution process when there is no final text', () => {
  // Given
  const parts: readonly LocalReplyPart[] = [
    { type: 'chunk', text: 'Reading canvas.' },
    { type: 'tool', id: 'read', name: 'get_canvas', status: 'completed', output: 'scene' },
    { type: 'status', message: 'Completed' },
  ]
  // When
  const split = splitReplyParts(parts)
  // Then
  expect(split.answer).toBe('')
  expect(split.progress).toEqual(parts)
})

it('preserves execution order while merging streamed text and updating the same tool call', () => {
  // Given
  const events: readonly LocalReplyPart[] = [
    { type: 'status', message: 'startup' },
    { type: 'status', message: 'startup' },
    { type: 'status', message: 'executing' },
    { type: 'thinking', text: 'Inspect ' },
    { type: 'thinking', text: 'the canvas' },
    { type: 'chunk', text: 'Reading now.' },
    { type: 'tool', id: 'read', name: 'get_canvas', status: 'running', input: '{"canvasId":"canvas"}' },
    { type: 'tool', id: 'read', name: 'get_canvas', status: 'running', output: 'partial' },
    { type: 'tool', id: 'read', name: 'get_canvas', status: 'completed', output: 'scene' },
    { type: 'thinking', text: 'Ready' },
    { type: 'chunk', text: 'A ' },
    { type: 'chunk', text: 'flowchart.' },
    { type: 'status', message: 'completed' },
  ]
  // When
  const parts = events.reduce<LocalReplyPart[]>(appendReplyPart, [])
  const restored = chatSchema.parse(JSON.parse(JSON.stringify([{ id: 's', title: 'chat', createdAt: 1, updatedAt: 1,
    messages: [{ id: 'm', role: 'assistant', timestamp: 1, content: 'Reading now.A flowchart.', parts, source: 'codex' }] }])))
  // Then
  expect(restored[0]?.messages[0]?.parts).toEqual([
    { type: 'status', message: 'startup' },
    { type: 'status', message: 'executing' },
    { type: 'thinking', text: 'Inspect the canvas' },
    { type: 'chunk', text: 'Reading now.' },
    { type: 'tool', id: 'read', name: 'get_canvas', status: 'completed', input: '{"canvasId":"canvas"}', output: 'scene' },
    { type: 'thinking', text: 'Ready' },
    { type: 'chunk', text: 'A flowchart.' },
    { type: 'status', message: 'completed' },
  ])
})

it('keeps partial tool output and explanation when a run fails or is stopped', () => {
  // Given
  const tool = { type: 'tool', id: 'run', name: 'command_execution', status: 'running', input: 'pwd', output: '/project' } as const
  const partial = { content: 'Inspecting', activities: [tool], parts: [{ type: 'chunk', text: 'Inspecting' }, tool] } satisfies {
    content: string; activities: readonly LocalReplyPart[]; parts: readonly LocalReplyPart[]
  }
  // When
  const failed = failChatReply(partial, 'Stopped')
  // Then
  expect(failed.parts).toEqual([
    { type: 'chunk', text: 'Inspecting' },
    { ...tool, status: 'failed', output: '/project\n\nStopped' },
    { type: 'status', level: 'error', message: 'Stopped' },
  ])
  expect(failed.activities[0]).toMatchObject({ status: 'failed', output: '/project\n\nStopped' })
})
