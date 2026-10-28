import { z } from 'zod'
import { invoke } from '@tauri-apps/api/core'
import { canvasCommandSchema } from '../canvas-commands'
const text = z.string().max(65536)
const id = z.string().uuid()
export const AgentRunStateSchema = z.enum(['created','preparing','running','waiting_input','validating','repairing','succeeded','failed','cancelling','cancelled','interrupted','needs_reconciliation'])
export const OperationStatusSchema = z.enum(['prepared','applied','failed','unknown'])
const backend = z.enum(['api','codex','claude','cursor','pi'])
export const AgentRunEventSchema = z.discriminatedUnion('type', [
 z.object({type:z.literal('context'),snapshot:text.refine(v=>{try{JSON.parse(v);return true}catch{return false}}),capabilities:z.array(text).max(256)}),
 z.object({type:z.literal('session'),sessionId:text,backend,resumed:z.boolean()}),
 z.object({type:z.literal('state'),state:AgentRunStateSchema,message:text.nullish()}),
 z.object({type:z.literal('status'),message:text,level:z.enum(['info','warning','error'])}),
 z.object({type:z.literal('thinking'),text}),
 z.object({type:z.literal('message'),text,phase:text.nullish()}),
 z.object({type:z.literal('tool'),callId:text,name:text,status:z.enum(['running','completed','failed']),input:text.nullish(),output:text.nullish()}),
 z.object({type:z.literal('plan'),steps:z.array(text).max(256)}),
 z.object({type:z.literal('operation'),operationId:id,status:OperationStatusSchema,targetIds:z.array(text).max(256)}),
 z.object({type:z.literal('artifact'),path:text,kind:text,verified:z.boolean()}),
 z.object({type:z.literal('usage'),inputTokens:z.number().int().nonnegative().nullish(),outputTokens:z.number().int().nonnegative().nullish()}),
 z.object({type:z.literal('validation'),ok:z.boolean(),message:text}),
 z.object({type:z.literal('input'),requestId:id,prompt:text,choices:z.array(text).max(256).nullish(),response:text.nullish()}),
])
export const AgentRunHeaderSchema = z.object({
 id,providerSessionId:text.nullish(),conversationId:text,parentRunId:id.nullish(),backend:z.enum(['api','codex','claude','cursor','pi']),
 projectId:text.nullish(),canvasId:text.nullish(),contextSnapshot:text.refine(v=>{try{JSON.parse(v);return true}catch{return false}}),
 state:AgentRunStateSchema,createdAt:z.number(),updatedAt:z.number(),capabilities:z.array(text).max(256),
})
export const AgentRunEnvelopeSchema = z.object({runId:id,seq:z.number().int().positive(),at:z.number(),event:AgentRunEventSchema})
const receiptPreparation = {intendedAfterSnapshot:text.nullish(),tool:text.nullish(),fingerprint:text.nullish(),baseRevision:text.nullish(),beforeSnapshot:text.nullish()}
const receiptSettlement = {result:text.nullish(),afterRevision:text.nullish(),afterSnapshot:text.nullish()}
export const OperationReceiptSchema = z.object({...receiptPreparation,...receiptSettlement,proof:text.nullish(),operationId:id,status:OperationStatusSchema,targetIds:z.array(text).max(256)})
export const AgentRunSchema = z.object({header:AgentRunHeaderSchema,events:z.array(AgentRunEnvelopeSchema),operations:z.record(OperationReceiptSchema)})
const scope = {projectId:text.nullish()}
export const AgentRunRequestSchema = z.discriminatedUnion('action',[
 z.object({action:z.literal('create'),header:AgentRunHeaderSchema}),
 z.object({action:z.literal('list'),...scope}),
 z.object({action:z.literal('read'),runId:id,...scope}),
 z.object({action:z.literal('append'),runId:id,event:AgentRunEventSchema,...scope}),
 z.object({action:z.literal('prepare_operation'),...receiptPreparation,runId:id,operationId:id,targetIds:z.array(text).max(256),...scope}),
 z.object({action:z.literal('reconcile_operation'),runId:id,operationId:id,expectedAfterRevision:text.nullish(),currentRevision:text.nullish(),status:z.enum(['applied','failed']),proof:text.nullish(),...scope}),
 z.object({action:z.literal('settle_operation'),...receiptSettlement,runId:id,operationId:id,status:OperationStatusSchema,...scope}),
])
export type OperationReceipt = z.infer<typeof OperationReceiptSchema>
export const AgentRunResponseSchema = z.union([AgentRunSchema,z.array(AgentRunSchema)])
export type AgentRun = z.infer<typeof AgentRunSchema>
export type AgentRunHeader = z.infer<typeof AgentRunHeaderSchema>
export type AgentRunEvent = z.infer<typeof AgentRunEventSchema>
export type AgentRunEnvelope = z.infer<typeof AgentRunEnvelopeSchema>
export type AgentRunState = z.infer<typeof AgentRunStateSchema>
export type AgentRunRequest = z.infer<typeof AgentRunRequestSchema>
export async function agentRunCommand(request:AgentRunRequest) {
 return AgentRunResponseSchema.parse(await invoke<unknown>('agent_run_command',{request:AgentRunRequestSchema.parse(request)}))
}

/** Persist JSON.stringify(parsedCommand) before mutation; projections are verified against disk. */
export const IntendedAfterSnapshotSchema = z.union([
 canvasCommandSchema.options[3], canvasCommandSchema.options[4], canvasCommandSchema.options[5],
])
export type IntendedAfterSnapshot = z.infer<typeof IntendedAfterSnapshotSchema>
/** Summary or raw elements; missing changed fields cannot prove a failed operation. */
export const BeforeSnapshotSchema = z.array(z.record(z.unknown())).max(256)

/** Skill intent is persisted before write; hashes cover exact UTF-8 file content. */
export const SkillIntendedAfterSnapshotSchema = z.object({
 kind:z.literal('skill'),
 name:z.string().min(1).max(64).regex(/^[a-z0-9]+(?:-[a-z0-9]+)*$/),
 files:z.array(z.object({path:z.string().max(1024),revision:z.string().regex(/^sha256:[a-f0-9]{64}$/)}).strict()).min(1).max(64),
}).strict()
export type SkillIntendedAfterSnapshot = z.infer<typeof SkillIntendedAfterSnapshotSchema>

/** Native WriteMemory stores .agents/memory/<name>.json; intent never stores the confirmed text. */
export const MemoryIntendedAfterSnapshotSchema = z.object({
 kind:z.literal('memory'),
 name:z.string().min(1).max(64).regex(/^[a-z0-9]+(?:-[a-z0-9]+)*$/),
 contentRevision:z.string().regex(/^sha256:[a-f0-9]{64}$/),
}).strict()
export type MemoryIntendedAfterSnapshot = z.infer<typeof MemoryIntendedAfterSnapshotSchema>
/** Matches native serde_json::to_string_pretty's sorted keys and no trailing newline. */
export async function createMemoryIntendedAfterSnapshot(name:string, content:string):Promise<MemoryIntendedAfterSnapshot> {
 const stored = JSON.stringify({confirmed:true,content,source:'explicit-user-command',version:1},null,2)
 const digest = await crypto.subtle.digest('SHA-256',new TextEncoder().encode(stored))
 const contentRevision = 'sha256:' + [...new Uint8Array(digest)].map(v=>v.toString(16).padStart(2,'0')).join('')
 return MemoryIntendedAfterSnapshotSchema.parse({kind:'memory',name,contentRevision})
}
