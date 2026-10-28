use super::{bounded, ChatError, LocalChatEvent, Parser, ToolStatus};

impl Parser {
    pub(super) fn settle_tools(&mut self) -> Result<(), ChatError> {
        let mut running: Vec<_> = self
            .activities
            .values()
            .filter_map(|event| match event {
                LocalChatEvent::Tool {
                    status: ToolStatus::Running,
                    ..
                } => Some(event.clone()),
                _ => None,
            })
            .collect();
        running.sort_by(|a, b| match (a, b) {
            (LocalChatEvent::Tool { id: a, .. }, LocalChatEvent::Tool { id: b, .. }) => a.cmp(b),
            _ => std::cmp::Ordering::Equal,
        });
        for event in running {
            if let LocalChatEvent::Tool {
                id,
                name,
                input,
                output,
                ..
            } = event
            {
                let (status, output) = if name == "update_plan" && self.failure.is_none() {
                    (ToolStatus::Completed, output)
                } else {
                    (
                        ToolStatus::Failed,
                        Some(output.unwrap_or_default() + "\n执行已结束，工具未返回完成事件"),
                    )
                };
                self.tool(&id, &name, status, input, output)?;
            }
        }
        Ok(())
    }

    pub(super) fn tool(
        &mut self,
        id: &str,
        name: &str,
        status: ToolStatus,
        input: Option<String>,
        output: Option<String>,
    ) -> Result<(), ChatError> {
        if id.is_empty() {
            return Ok(());
        }
        if status == ToolStatus::Running && !self.tool_names.contains_key(id) {
            self.latest_text.clear();
            self.latest_text_id = None;
        }
        let name = if name.is_empty() {
            self.tool_names
                .get(id)
                .cloned()
                .unwrap_or_else(|| "工具".into())
        } else {
            self.tool_names.insert(id.to_owned(), name.to_owned());
            name.to_owned()
        };
        let previous = self.activities.get(&format!("tool:{id}"));
        if status == ToolStatus::Running
            && matches!(
                previous,
                Some(LocalChatEvent::Tool {
                    status: ToolStatus::Completed | ToolStatus::Failed,
                    ..
                })
            )
        {
            return Ok(());
        }
        let (old_input, old_output) = match previous {
            Some(LocalChatEvent::Tool { input, output, .. }) => (input.clone(), output.clone()),
            _ => (None, None),
        };
        self.activity(LocalChatEvent::Tool {
            id: id.into(),
            name,
            status,
            input: input.or(old_input).map(bounded),
            output: output.or(old_output).map(bounded),
        })
    }

    fn activity(&mut self, event: LocalChatEvent) -> Result<(), ChatError> {
        let id = match &event {
            LocalChatEvent::Tool { id, .. } => format!("tool:{id}"),
            LocalChatEvent::Chunk { .. }
            | LocalChatEvent::Thinking { .. }
            | LocalChatEvent::Session { .. }
            | LocalChatEvent::Phase { .. }
            | LocalChatEvent::Plan { .. }
            | LocalChatEvent::Usage { .. }
            | LocalChatEvent::Input { .. }
            | LocalChatEvent::Status { .. } => return self.send(event),
        };
        if self.activities.get(&id) == Some(&event) {
            return Ok(());
        }
        self.activities.insert(id, event.clone());
        self.send(event)
    }
}
