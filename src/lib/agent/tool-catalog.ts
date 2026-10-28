const definition = (name: string, description: string, properties: object, required: readonly string[]) => ({
  type: 'function', function: { name, description, parameters: { type: 'object', properties, required, additionalProperties: false } },
})
const text = { type: 'string' }, texts = { type: 'array', items: text }
export const agentToolCatalog = [
  definition('update_plan', '记录多步骤任务的执行计划；简单问答和直接绘图无需额外规划。', { steps: texts }, ['steps']),
  definition('request_user_input', '任务缺少必要信息时等待用户回答；不要为已经授权的普通绘图或修改重复确认。', { prompt: text, choices: texts }, ['prompt']),
  definition('project_manifest', '按需读取当前项目 Skill、MCP、策略及已确认记忆的清单。', {}, []),
  definition('read_skill', '读取项目 .agents/skills 下的 Skill 文件或资源。', { path: text }, ['path']),
  definition('write_skill', '仅当用户已要求创建或优化 Skill 时，保存到当前项目 .agents/skills。更新前先读取 revision；画布图形不等于文件保存。', {
    name: text, content: text, expectedRevision: { type: ['string', 'null'] },
    files: { type: 'array', items: { type: 'object', properties: { path: text, content: text }, required: ['path', 'content'], additionalProperties: false } },
  }, ['name', 'content', 'expectedRevision']),
  definition('remember_fact', '提出需要长期保留的项目事实。应用会展示具体内容并等待用户明确确认。', { name: text, content: text }, ['name', 'content']),
]
