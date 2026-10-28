# 元素格式

以 `tools` 返回的运行时 JSON Schema 为准。以下对应本机桌面版 0.2.0，新增或修改额外字段会被拒绝。所有写入与定位请求都必须包含 `canvasId`；示例中的 ID 仅表示格式，使用时替换为 `get_workspace`、`create_canvas` 或 `open_canvas` 返回的实际 ID。

## 新建与修改

`add_elements` 每项必需：`id`、`type`、`x`、`y`、`width`、`height`。`type` 为 `rectangle`、`ellipse`、`diamond`、`text`、`arrow` 或 `line`。ID 为 1–200 个字符且在当前画布和本批中均不重复；位置为有限数，宽高非负。

`update_elements` 每项只要求 `id`，其余只传需要变化的字段。不支持改变已有元素的 `type`；不要回传 `version`、`index`、`seed`、`isDeleted` 等原始元数据。

| 可选字段 | 范围或说明 |
|---|---|
| `text`, `originalText` | 字符串，最多 20000 字符；修改 text 时应用会同步 originalText |
| `strokeColor`, `backgroundColor` | 颜色字符串；透明填充用 `transparent` |
| `strokeWidth` | 0–20 |
| `opacity` | 0–100 |
| `roughness` | 0–5 |
| `angle` | 有限数，弧度 |
| `fontSize` | 大于 0 且不大于 500；默认 20 |
| `fontFamily` | 1–8 的整数；默认 1 |
| `textAlign` | `left` / `center` / `right` |
| `verticalAlign` | `top` / `middle` / `bottom` |
| `fillStyle` | `solid` / `hachure` / `cross-hatch` |
| `points` | 2–1000 个 `[dx, dy]` 相对坐标对 |
| `startArrowhead`, `endArrowhead` | `arrow` / `bar` / `dot` / `triangle` / null |
| `containerId` | 容器 ID 或 null |
| `boundElements` | `[{ "id": "...", "type": "text" 或 "arrow" }]` 或 null |

不支持传入 `link`、`groupIds`、`startBinding`、`endBinding`、`roundness` 等未列字段。当前工具也不支持图片、自由绘制或其他元素类型。

## 最小双节点示例

以下 JSON 是 `add_elements` 的完整 arguments；真实使用时生成新 ID，并根据已有包围盒调整位置：

```json
{
  "canvasId": "00000000-0000-4000-8000-000000000001:diagrams/flow.excalidraw",
  "elements": [
    {"id":"start-a1","type":"rectangle","x":100,"y":100,"width":220,"height":100,"backgroundColor":"#dbeafe"},
    {"id":"label-a1","type":"text","x":120,"y":135,"width":180,"height":30,"text":"用户请求","fontSize":24},
    {"id":"arrow-a1","type":"arrow","x":330,"y":150,"width":100,"height":0,"points":[[0,0],[100,0]],"endArrowhead":"arrow"},
    {"id":"end-a1","type":"rectangle","x":440,"y":100,"width":220,"height":100,"backgroundColor":"#dcfce7"},
    {"id":"label-b1","type":"text","x":460,"y":135,"width":180,"height":30,"text":"处理结果","fontSize":24}
  ]
}
```

中文可先按每个字约一个字号、每行高约 `1.25 × fontSize` 估算，留出内边距；这是排版起点，不是精确字体测量。长标签换行或增大容器，不用缩到难读的字号。

需绑定标签时，在形状中设置 `boundElements: [{"id":"label-id","type":"text"}]`，对应文字设置 `containerId: "shape-id"`，两边 ID 保持一致。删除形状会连带删除绑定文字。普通独立标签也可使用，但移动形状时要同时移动标签。

修改文字示例：

```json
{"canvasId":"00000000-0000-4000-8000-000000000001:diagrams/flow.excalidraw","elements":[{"id":"label-b1","text":"PostgreSQL","width":190,"height":30}]}
```

定位示例：

```json
{"canvasId":"00000000-0000-4000-8000-000000000001:diagrams/flow.excalidraw","ids":["start-a1","label-a1","arrow-a1","end-a1","label-b1"]}
```
