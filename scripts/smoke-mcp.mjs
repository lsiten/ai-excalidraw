import assert from 'node:assert/strict'
import { randomUUID } from 'node:crypto'
import { Client } from '@modelcontextprotocol/sdk/client/index.js'
import { StdioClientTransport } from '@modelcontextprotocol/sdk/client/stdio.js'

const client = new Client({ name: 'canvas-smoke-test', version: '0.5.0' })
const server = process.argv[2] || new URL('./mcp-server.mjs', import.meta.url).pathname
await client.connect(new StdioClientTransport({ command: process.env.AI_EXCALIDRAW_BINARY || process.execPath, args: process.env.AI_EXCALIDRAW_BINARY ? ["--mcp-stdio"] : [server], env: process.env.AI_EXCALIDRAW_BRIDGE_FILE ? { AI_EXCALIDRAW_BRIDGE_FILE: process.env.AI_EXCALIDRAW_BRIDGE_FILE } : {} }))
const prefix = `smoke-${randomUUID()}`
const boxId = `${prefix}-box`
const textId = `${prefix}-text`
const observations = []
let originalIds
let canvasId
const call = async (name, args = {}) => {
  const result = await client.callTool({ name, arguments: canvasId ? { canvasId, ...args } : args })
  assert.notEqual(result.isError, true, JSON.stringify(result.content))
  return JSON.parse(result.content.find((item) => item.type === 'text').text)
}

try {
  // Given the real running desktop app, when MCP connects, then all twenty tools are discoverable.
  assert.equal((await client.listTools()).tools.length, 20)
  const before = await call('get_canvas')
  canvasId = before.canvasId
  assert.ok(canvasId, 'Open a test canvas before running smoke tests')
  originalIds = before.elements.map((element) => element.id).sort()
  observations.push('MCP initialization, discovery and live canvas read passed')

  // Given unique test IDs, when elements are added, then the real canvas contains them.
  await call('add_elements', { elements: [
    { id: boxId, type: 'rectangle', x: 200, y: 160, width: 220, height: 100, backgroundColor: '#dbeafe' },
    { id: textId, type: 'text', x: 220, y: 195, width: 180, height: 25, text: 'MCP 测试', fontSize: 20 },
  ] })
  const added = await call('get_canvas')
  assert.equal(added.elements.find((element) => element.id === textId)?.text, 'MCP 测试')
  observations.push('External create rendered two live scene elements')

  // Given a just-created batch, when undo is invoked, then it disappears from the real scene.
  assert.equal((await call('undo')).applied, true)
  assert.deepEqual((await call('get_canvas')).elements.map((element) => element.id).sort(), originalIds)
  // Given the undone batch, when redo is invoked, then both elements return.
  assert.equal((await call('redo')).applied, true)
  assert.equal((await call('get_canvas')).elements.some((element) => element.id === boxId), true)
  observations.push('External undo and redo operated on the editor history')

  // Given a live shape, when a partial color patch is sent, then its color changes without losing geometry.
  await call('update_elements', { elements: [{ id: boxId, backgroundColor: '#dcfce7' }, { id: textId, text: 'MCP 已修改' }] })
  const updated = await call('get_canvas')
  const box = updated.elements.find((element) => element.id === boxId)
  assert.equal(box?.backgroundColor, '#dcfce7')
  assert.equal(box?.width, 220)
  assert.equal(updated.elements.find((element) => element.id === textId)?.originalText, 'MCP 已修改')
  observations.push('External update preserved geometry and updated text')

  // Given malformed geometry, when a create tool is called, then it fails without changing the scene.
  const invalid = await client.callTool({ name: 'add_elements', arguments: { canvasId, elements: [{ id: `${prefix}-invalid`, type: 'rectangle', x: 0, y: 0, width: -1, height: 10 }] } })
  assert.equal(invalid.isError, true)
  const duplicate = await client.callTool({ name: 'add_elements', arguments: { canvasId, elements: [{ id: boxId, type: 'rectangle', x: 0, y: 0, width: 5, height: 5 }] } })
  assert.equal(duplicate.isError, true)
  assert.equal((await call('get_canvas')).elements.length, before.elements.length + 2)
  observations.push('Malformed geometry and duplicate IDs rejected without mutation')

  await call('fit_canvas', { ids: [boxId, textId] })
  assert.deepEqual((await call('delete_elements', { ids: [`${prefix}-missing`] })).notFound, [`${prefix}-missing`])
  observations.push('Viewport fitting and missing-ID error handling passed')
} finally {
  try {
    if (canvasId) await call('delete_elements', { ids: [boxId, textId] })
    const after = await call('get_canvas')
    assert.equal(after.elements.some((element) => element.id.startsWith(prefix)), false)
    if (originalIds) assert.deepEqual(after.elements.map((element) => element.id).sort(), originalIds)
    observations.push('External deletion verified; temporary elements removed')
  } finally {
    await client.close()
  }
}
console.log(JSON.stringify({ passed: true, observations }, null, 2))
