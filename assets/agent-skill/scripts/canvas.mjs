#!/usr/bin/env node
import { readFile } from 'node:fs/promises'
import { homedir } from 'node:os'
import { join } from 'node:path'
const usage = 'Usage: canvas.mjs status | tools | call <tool> [--args-file /absolute/path.json]'
async function main() {
  const [mode, tool, flag, argumentPath, ...extra] = process.argv.slice(2)
  if (mode === 'help' || mode === '--help') { console.log(usage); return }
  if (!['status','tools','call'].includes(mode) || extra.length || (mode !== 'call' && tool !== undefined) || (mode === 'call' && (!tool || (flag !== undefined && (flag !== '--args-file' || !argumentPath))))) throw new Error(usage)
  const root = process.platform === 'darwin' ? join(homedir(),'Library','Application Support') : process.platform === 'win32' ? process.env.APPDATA || join(homedir(),'AppData','Roaming') : process.env.XDG_DATA_HOME || join(homedir(),'.local','share')
  let connection
  try { connection = JSON.parse(await readFile(process.env.AI_EXCALIDRAW_BRIDGE_FILE || join(root,'com.local.ai-excalidraw','bridge.json'),'utf8')) } catch { throw new Error('Open AI Excalidraw desktop first. Its MCP service is not running.') }
  if (!Number.isInteger(connection.port) || connection.port < 1 || connection.port > 65535 || typeof connection.token !== 'string') throw new Error('Invalid desktop connection')
  let nextId = 0
  const request = async (method, params = {}) => {
    const response = await fetch(`http://127.0.0.1:${connection.port}/mcp`, { method:'POST',redirect:'error',signal:AbortSignal.timeout(20000), headers:{Authorization:`Bearer ${connection.token}`,'Content-Type':'application/json',Accept:'application/json, text/event-stream'},body:JSON.stringify({jsonrpc:'2.0',id:++nextId,method,params}) })
    if (!response.ok) throw new Error(`Desktop MCP HTTP ${response.status}. Inspect the target before retrying a mutation.`)
    const message = await response.json()
    if (message.error) throw new Error(message.error.message)
    return message.result
  }
  await request('initialize',{protocolVersion:'2025-11-25',capabilities:{},clientInfo:{name:'ai-excalidraw-skill',version:'1.5.0'}})
  const definitions = await request('tools/list')
  if (mode === 'tools') { console.log(JSON.stringify(definitions,null,2)); return }
  const name = mode === 'status' ? 'get_workspace' : tool
  if (!definitions.tools?.some(item=>item.name===name)) throw new Error(`Unknown canvas tool: ${name}`)
  const args = argumentPath ? JSON.parse(await readFile(argumentPath,'utf8')) : {}
  if (!args || Array.isArray(args) || typeof args !== 'object') throw new Error('Arguments must be a JSON object')
  const result = await request('tools/call',{name,arguments:args})
  const text = result.content.filter(item=>item.type==='text').map(item=>item.text).join('\n')
  if (result.isError) throw new Error(text)
  const value = JSON.parse(text)
  console.log(JSON.stringify(mode === 'status' ? {connected:true,...value} : value,null,2))
}
main().catch(error=> { console.error(error instanceof Error ? error.message : String(error)); process.exitCode=1 })
