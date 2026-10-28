const id = { type: 'string', minLength: 1, maxLength: 200 }
const uuidPattern = '[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}'
const pathSegment = '(?!\\.{1,2}(?:/|$))[^/\\\\\\u0000-\\u001f:]+'
const relativePathPattern = `${pathSegment}(?:/${pathSegment})*`
const projectId = { type: 'string', pattern: `^${uuidPattern}$`, description: 'Project UUID returned by get_workspace, list_projects or open_project.' }
const path = { type: 'string', pattern: `^${relativePathPattern}$`, description: 'Project-relative file or directory path, with / separators; no absolute paths or . / .. segments.' }
const canvasPath = { ...path, pattern: `^${relativePathPattern}\\.excalidraw$`, description: 'Project-relative .excalidraw file path, with / separators.' }
const canvasId = {
  type: 'string', maxLength: 2100, pattern: `^${uuidPattern}:${relativePathPattern}\\.excalidraw$`,
  description: 'Explicit canvas identity: projectUUID:relative/path.excalidraw (at most 2100 characters). Use an ID returned by the app; the target must be a loaded tab.',
}
const elementProperties = {
  id, type: { type: 'string', enum: ['rectangle', 'ellipse', 'diamond', 'text', 'arrow', 'line'] },
  x: { type: 'number' }, y: { type: 'number' }, width: { type: 'number', minimum: 0 }, height: { type: 'number', minimum: 0 },
  text: { type: 'string' }, originalText: { type: 'string' },
  strokeColor: { type: 'string' }, backgroundColor: { type: 'string' },
  strokeWidth: { type: 'number', minimum: 0, maximum: 20 },
  opacity: { type: 'number', minimum: 0, maximum: 100 }, roughness: { type: 'number', minimum: 0, maximum: 5 },
  angle: { type: 'number' }, fontSize: { type: 'number', exclusiveMinimum: 0, maximum: 500 },
  fontFamily: { type: 'integer', minimum: 1, maximum: 8 },
  textAlign: { type: 'string', enum: ['left', 'center', 'right'] },
  verticalAlign: { type: 'string', enum: ['top', 'middle', 'bottom'] },
  fillStyle: { type: 'string', enum: ['solid', 'hachure', 'cross-hatch'] },
  points: { type: 'array', minItems: 2, maxItems: 1000, items: { type: 'array', items: { type: 'number' }, minItems: 2, maxItems: 2 } },
  startArrowhead: { type: ['string', 'null'], enum: ['arrow', 'bar', 'dot', 'triangle', null] },
  endArrowhead: { type: ['string', 'null'], enum: ['arrow', 'bar', 'dot', 'triangle', null] },
  containerId: { type: ['string', 'null'] },
  boundElements: { type: ['array', 'null'], items: { type: 'object', properties: { id, type: { enum: ['text', 'arrow'] } }, required: ['id', 'type'], additionalProperties: false } },
}
const object = (properties, required = []) => ({ type: 'object', properties, required, additionalProperties: false })
const presentation = object({
  decks: {
    type: 'object', maxProperties: 5000,
    propertyNames: { type: 'string', pattern: `^(?:${relativePathPattern})?$` },
    additionalProperties: object({
      order: { type: 'array', maxItems: 25000, items: canvasPath },
      includeSubfolders: { type: 'boolean' },
    }, ['order', 'includeSubfolders']),
    description: 'Folder settings keyed by project-relative folder path; the empty key means the project root. Order entries are project-relative .excalidraw paths.',
  },
  areas: {
    type: 'object', maxProperties: 5000, propertyNames: canvasPath,
    additionalProperties: object({
      x: { type: 'number', minimum: -1e9, maximum: 1e9 },
      y: { type: 'number', minimum: -1e9, maximum: 1e9 },
      width: { type: 'number', exclusiveMinimum: 0, maximum: 1e9 },
      height: { type: 'number', exclusiveMinimum: 0, maximum: 1e9 },
    }, ['x', 'y', 'width', 'height']),
    description: 'Playback rectangles in scene pixels keyed by project-relative .excalidraw path; omit a key to use full content bounds.',
  },
}, ['decks', 'areas'])
const elementList = (required) => object({ canvasId, elements: { type: 'array', minItems: 1, maxItems: 500, items: object(elementProperties, required) } }, ['canvasId', 'elements'])
export const tools = [
  ...['undo', 'redo'].map((name) => ({
    name,
    description: `${name === 'undo' ? 'Undo the last canvas history step' : 'Redo the last undone canvas history step'} for the required canvasId. Activates that loaded tab, uses the editor's own history action and saves the result. Returns applied=false if no history action is available. Only use at the user's request; history may include manual user edits.`,
    inputSchema: object({ canvasId }, ['canvasId']),
    annotations: { readOnlyHint: false, destructiveHint: true, openWorldHint: false },
  })),
  {
    name: 'get_canvas',
    description: 'Read a loaded canvas by canvasId, or omit canvasId to read the active canvas, including its canvasId, full elements, selected elements and bounds. Explicit IDs can read loaded background tabs without switching tabs. Read before editing. Coordinates are scene pixels, angles are radians. No separate AI API key is needed. Use get_workspace when no canvas is active.',
    inputSchema: object({ canvasId }),
    annotations: { readOnlyHint: true, openWorldHint: false },
  },
  {
    name: 'add_elements',
    description: 'Create up to 500 elements with NEW unique IDs in the required canvasId, including a loaded background tab without switching tabs. Required element fields: id,type,x,y,width,height. For text also provide text and preferably fontSize (default 20); width/height must fit it. A box label is a separate text element, optionally linked via containerId/boundElements. Arrow points are relative to x/y, e.g. [[0,0],[180,0]]. Do not overwrite existing IDs. Call fit_canvas with the same canvasId after drawing.',
    inputSchema: elementList(['id', 'type', 'x', 'y', 'width', 'height']),
    annotations: { readOnlyHint: false, destructiveHint: false, openWorldHint: false },
  },
  {
    name: 'update_elements',
    description: 'Patch existing elements by ID in the required canvasId, including a loaded background tab without switching tabs. Only send changed supported fields (position, size, text, colors, etc). Element types cannot change. Returns updated and notFound IDs. Read get_canvas with the same canvasId first. Text updates should include appropriate width/height.',
    inputSchema: elementList(['id']),
    annotations: { readOnlyHint: false, destructiveHint: true, openWorldHint: false },
  },
  {
    name: 'delete_elements',
    description: 'Delete the specified IDs from the required canvasId, including a loaded background tab without switching tabs; bound labels are also removed. Returns deleted and notFound IDs. Read get_canvas with the same canvasId first and only delete what the user requested.',
    inputSchema: object({ canvasId, ids: { type: 'array', minItems: 1, maxItems: 500, items: id } }, ['canvasId', 'ids']),
    annotations: { readOnlyHint: false, destructiveHint: true, openWorldHint: false },
  },
  {
    name: 'fit_canvas',
    description: 'Pan/zoom the required canvasId to fit all elements, or only the supplied IDs. Supports loaded background tabs without switching tabs. Call after creating a diagram; use open_canvas if the user wants to activate its tab.',
    inputSchema: object({ canvasId, ids: { type: 'array', maxItems: 500, items: id } }, ['canvasId']),
    annotations: { readOnlyHint: false, destructiveHint: false, openWorldHint: false },
  },
  {
    name: 'get_workspace',
    description: 'Read the desktop workspace, even when no canvas is active or playback is running. Returns projects, projectId, activeCanvasId, presenting and canvases [{canvasId,projectId,path,status}]. Discover explicit project and loaded canvas identities before operating. When presenting is true, ask the user to exit playback before other operations.',
    inputSchema: object({}),
    annotations: { readOnlyHint: true, openWorldHint: false },
  },
  {
    name: 'list_projects',
    description: 'List the desktop workspace projects and their IDs.',
    inputSchema: object({}),
    annotations: { readOnlyHint: true, openWorldHint: false },
  },
  {
    name: 'open_project',
    description: 'Open an existing local directory as a project. directory must be an absolute directory path. Read get_workspace afterward to inspect the selected project and loaded tabs.',
    inputSchema: object({ directory: { type: 'string', pattern: '^(?:/|[A-Za-z]:[\\\\/]|\\\\\\\\)[^\\u0000]*$', description: 'Absolute local directory path.' } }, ['directory']),
    annotations: { readOnlyHint: false, destructiveHint: false, openWorldHint: false },
  },
  {
    name: 'list_files',
    description: 'List files and directories in the specified project. Paths are relative to the project directory.',
    inputSchema: object({ projectId }, ['projectId']),
    annotations: { readOnlyHint: true, openWorldHint: false },
  },
  {
    name: 'get_presentation',
    description: 'Read project presentation metadata: {decks,areas}. Decks map folder paths (empty string for project root) to {order,includeSubfolders}; areas map .excalidraw paths to {x,y,width,height} in scene pixels. Read before set_presentation and preserve unrelated settings. Does not start playback. Unavailable during playback; get_workspace remains available.',
    inputSchema: object({ projectId }, ['projectId']),
    annotations: { readOnlyHint: true, openWorldHint: false },
  },
  {
    name: 'set_presentation',
    description: 'Replace only the project presentation metadata with {decks,areas}; never edits canvas elements. Read get_presentation first and merge unrelated entries. Deck order uses project-relative .excalidraw paths; eligible unlisted files follow in natural filename order, filtered by includeSubfolders. Area rectangles use finite scene coordinates and positive dimensions; omit an area to restore full content bounds. Native validation enforces safe paths, extensions, 4096-byte paths, 5000 decks/areas, 25000 total order entries and coordinate magnitude at most 1e9. Returns saved=true. No playback start/stop or annotation tool; user controls playback in the app. Rejected during playback.',
    inputSchema: object({ projectId, presentation }, ['projectId', 'presentation']),
    annotations: { readOnlyHint: false, destructiveHint: true, openWorldHint: false },
  },
  {
    name: 'create_canvas',
    description: 'Create a new .excalidraw file at a project-relative path and open its tab. Returns canvasId. Use an unused path and create any missing parent directories first.',
    inputSchema: object({ projectId, path: canvasPath }, ['projectId', 'path']),
    annotations: { readOnlyHint: false, destructiveHint: false, openWorldHint: false },
  },
  {
    name: 'open_canvas',
    description: 'Open an existing project-relative .excalidraw file and activate its tab. Returns canvasId; use that explicit ID for subsequent canvas operations.',
    inputSchema: object({ projectId, path: canvasPath }, ['projectId', 'path']),
    annotations: { readOnlyHint: false, destructiveHint: false, openWorldHint: false },
  },
  {
    name: 'rename_file',
    description: 'Rename or move a file or directory within a project using relative from/to paths. Associated conversations move with it. Canvas identities change with their paths; read get_workspace and list_files afterward before using new canvasIds.',
    inputSchema: object({ projectId, from: path, to: path }, ['projectId', 'from', 'to']),
    annotations: { readOnlyHint: false, destructiveHint: true, openWorldHint: false },
  },
  {
    name: 'duplicate_canvas',
    description: 'Duplicate an existing .excalidraw file within a project using relative from/to paths. Use an unused destination; read list_files afterward and open_canvas when the copy needs editing.',
    inputSchema: object({ projectId, from: canvasPath, to: canvasPath }, ['projectId', 'from', 'to']),
    annotations: { readOnlyHint: false, destructiveHint: false, openWorldHint: false },
  },
  {
    name: 'create_directory',
    description: 'Create a directory at a project-relative path. Its parent directory must already exist; create nested directories one level at a time.',
    inputSchema: object({ projectId, path }, ['projectId', 'path']),
    annotations: { readOnlyHint: false, destructiveHint: false, openWorldHint: false },
  },
  {
    name: 'delete_file',
    description: 'Move a real project file or directory to the system Trash. Deleting a directory includes ALL files inside it, not just canvases. Inspect list_files first and delete only the scope requested by the user. Read get_workspace and list_files afterward.',
    inputSchema: object({ projectId, path }, ['projectId', 'path']),
    annotations: { readOnlyHint: false, destructiveHint: true, openWorldHint: false },
  },
  {
    name: 'reveal_file',
    description: 'Reveal a project-relative file or directory in the system file manager. Omit path to reveal the project directory.',
    inputSchema: object({ projectId, path }, ['projectId']),
    annotations: { readOnlyHint: false, destructiveHint: false, openWorldHint: false },
  },
  {
    name: 'get_project_resources', description: 'Read project-first Skill/MCP metadata, confirmed memory and restrictive policy. Does not expose credentials.',
    inputSchema: object({ projectId }, ['projectId']), annotations: { readOnlyHint: true, openWorldHint: false },
  },
  {
    name: 'read_agent_file', description: 'Preview a scoped .agents Skill/resource/memory file and its revision before proposing or saving changes.',
    inputSchema: object({ projectId, path: { type: 'string', maxLength: 2048, description: 'Exact file path returned by get_project_resources, including approved global-skill:// references.' } }, ['projectId', 'path']), annotations: { readOnlyHint: true, openWorldHint: false },
  },
  {
    name: 'write_skill', description: 'Save a Skill package only inside the project of an active in-app Agent run whose user requested Skill creation or optimization. Read the current package revision before updating; use null for creation. Returns verified file artifacts; a canvas diagram alone is not a saved Skill.',
    inputSchema: object({ projectId, name: { type: 'string', pattern: '^[a-z0-9]+(?:-[a-z0-9]+)*$' }, content: { type: 'string', maxLength: 262144 },
      expectedRevision: { type: ['string', 'null'] }, files: { type: 'array', maxItems: 63, items: object({ path, content: { type: 'string', maxLength: 262144 } }, ['path', 'content']) },
    }, ['projectId', 'name', 'content', 'expectedRevision']), annotations: { readOnlyHint: false, destructiveHint: false, openWorldHint: false },
  },
  {
    name: 'request_user_input', description: 'For an active in-app run, show a real input request in the conversation and wait for the human response. Shapes and canvas labels are not approval. Do not request routine confirmations for authorized drawing.',
    inputSchema: object({ projectId, prompt: { type: 'string', maxLength: 8000 }, choices: { type: 'array', maxItems: 10, items: { type: 'string' } } }, ['projectId', 'prompt']), annotations: { readOnlyHint: true, openWorldHint: false },
  },
  {
    name: 'remember_project_fact', description: 'Propose a durable project fact. The app asks the human to confirm this exact content before writing .agents/memory. Requires an active in-app run.',
    inputSchema: object({ projectId, name: { type: 'string', pattern: '^[a-z0-9]+(?:-[a-z0-9]+)*$' }, content: { type: 'string', maxLength: 8192 } }, ['projectId', 'name', 'content']), annotations: { readOnlyHint: false, destructiveHint: false, openWorldHint: false },
  },
  {
    name: 'update_plan', description: 'Record a short task plan for a complex active in-app run. Simple explanations, direct drawing and selected edits do not require planning.',
    inputSchema: object({ projectId, steps: { type: 'array', minItems: 1, maxItems: 32, items: { type: 'string', maxLength: 2000 } } }, ['projectId', 'steps']), annotations: { readOnlyHint: true, openWorldHint: false },
  },
].map((tool) => tool.name === 'get_workspace' || tool.name.endsWith('_presentation') ? tool : {
  ...tool, description: `${tool.description} Unavailable during playback; check get_workspace.presenting and ask the user to exit first.`,
})

export const instructions = 'Operate the running local AI Excalidraw desktop app 0.5.0 through 26 tools. Read get_workspace to discover projects, loaded canvases and presenting. Use the editable canvas to show useful comparisons, plans, workflows and results; refer to labelled options in chat and read fresh canvas/selection feedback on the next turn. Direct drawing requests and explicit edits to selected elements should still be performed directly. Selection alone is reference context, not approval; ordinary shapes do not submit forms or execute actions. When creating or improving a Skill, consider visualizing its workflow and changes and incorporating appropriate canvas feedback into its instructions. Skill source belongs in the current project .agents/skills when file permissions allow; canvas diagrams do not mean a Skill file was saved or installed. During playback only get_workspace is available; ask the user to exit before other operations. get_presentation reads and set_presentation replaces only presentation metadata; read and merge existing decks/areas first. Playback of canvas files or folders and temporary annotations are controlled by the user in the app; annotations last until exit and are not canvas edits. No start/stop playback tool exists. Canvas IDs are projectUUID:relative/path.excalidraw, at most 2100 characters. Only get_canvas may omit canvasId to use the active canvas; all six other canvas tools require it. Loaded background tabs can be targeted without switching tabs, except undo/redo activate the target tab. Read the target canvas before editing and preserve unrelated elements. On timeout, save failure or conflict, read get_workspace/list_files and the affected get_canvas or get_presentation before deciding what to do; never blindly retry a mutation, which may already have applied.'
