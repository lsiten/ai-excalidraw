use std::{collections::HashSet, time::Duration};
use tokio::time::Instant;
pub(super) struct Limits {
    pub(super) handshake: Duration,
    pub(super) startup: Duration,
    pub(super) idle: Duration,
    pub(super) tool: Duration,
    pub(super) human: Duration,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            handshake: Duration::from_secs(30),
            startup: Duration::from_secs(60),
            idle: Duration::from_secs(120),
            tool: Duration::from_secs(120),
            human: Duration::from_secs(300),
        }
    }
}

pub(super) struct Watchdog {
    deadline: Instant,
    tool_deadline: Option<Instant>,
    active_tools: HashSet<String>,
    human_tools: HashSet<String>,
    human_deadline: Instant,
}
impl Watchdog {
    pub(super) fn new(limits: &Limits) -> Self {
        Self {
            deadline: Instant::now() + limits.handshake,
            tool_deadline: None,
            active_tools: HashSet::new(),
            human_tools: HashSet::new(),
            human_deadline: Instant::now() + limits.human.min(Duration::from_secs(300)),
        }
    }
    pub(super) fn expiry(&self) -> Instant {
        if self.waiting_for_human() {
            return self.human_deadline;
        }
        self.tool_deadline
            .map_or(self.deadline, |d| d.min(self.deadline))
    }
    pub(super) fn startup(&mut self, limits: &Limits) {
        self.deadline = Instant::now() + limits.startup;
    }
    pub(super) fn semantic(&mut self, limits: &Limits) {
        self.deadline = Instant::now() + limits.idle;
    }
    pub(super) fn tool_started(&mut self, id: String, limits: &Limits) {
        self.active_tools.insert(id);
        self.tool_deadline
            .get_or_insert(Instant::now() + limits.tool);
    }
    pub(super) fn waiting_for_human(&self) -> bool {
        !self.human_tools.is_empty()
    }
    pub(super) fn human_started(&mut self, id: String) {
        self.human_tools.insert(id);
    }
    pub(super) fn tool_completed(&mut self, id: &str, limits: &Limits) {
        let human_finished = self.human_tools.remove(id) && !self.waiting_for_human();
        self.active_tools.remove(id);
        if human_finished {
            self.semantic(limits);
            self.tool_deadline =
                (!self.active_tools.is_empty()).then(|| Instant::now() + limits.tool);
        }
        if self.active_tools.is_empty() {
            self.tool_deadline = None;
        }
    }
}
