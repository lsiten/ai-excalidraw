import type { LocalChatActivity } from '../local-chat'

/** Keep public details within the native journal's byte bound without changing tool arguments. */
export function boundedAgentText(text: string, limit = 60000) {
  const bytes = new TextEncoder().encode(text)
  if (bytes.length <= limit) return text
  const marker = '\n…（显示内容已截断，完整结果请读取产物）'
  let end = Math.max(0, limit - new TextEncoder().encode(marker).length)
  while (end > 0 && (bytes[end] & 0xc0) === 0x80) end--
  return new TextDecoder().decode(bytes.subarray(0, end)) + marker
}
export function boundedActivity(activity: LocalChatActivity): LocalChatActivity {
  return activity.type === 'status' ? {...activity,message:boundedAgentText(activity.message)} : {
    ...activity,input:activity.input?boundedAgentText(activity.input):activity.input,output:activity.output?boundedAgentText(activity.output):activity.output,
  }
}
