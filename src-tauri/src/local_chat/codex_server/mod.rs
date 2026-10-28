//! Native provider threads persist across bounded app-server process lifetimes.
pub(super) mod capabilities;
#[cfg(test)]
mod diagnostic_tests;
mod diagnostics;
#[cfg(test)]
mod human_wait_tests;
mod messages;
mod notifications;
mod session;
mod store;
mod tool_events;
mod transport;
mod watchdog;
pub(super) use store::Store;
pub(super) use transport::execute;
#[cfg(test)]
mod tests;

#[cfg(all(test, unix))]
mod human_transport_tests;
