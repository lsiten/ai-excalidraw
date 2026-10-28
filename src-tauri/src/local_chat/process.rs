use super::{
    cli::Invocation,
    error::{cli_failure, io, ChatError, FailureOrigin},
    state::cancelled,
};
use std::{
    future::Future,
    process::ExitStatus,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWriteExt},
    process::{Child, Command},
    sync::watch,
};

pub(super) const RESPONSE_LIMIT: usize = 4 * 1024 * 1024;
const DIAGNOSTIC_LIMIT: usize = 64 * 1024;
pub(super) const TIME_LIMIT: Duration = Duration::from_secs(300);

pub(super) struct Process {
    pub(super) child: Child,
    registered: Arc<Mutex<Option<u32>>>,
}

pub(super) struct ProcessControl {
    pub cancel: watch::Receiver<bool>,
    pub process: Arc<Mutex<Option<u32>>>,
    pub duration: Duration,
}

pub(super) fn kill_registered(registered: &Mutex<Option<u32>>) {
    if let Ok(pid) = registered.lock() {
        #[cfg(unix)]
        if let Some(group) = pid
            .and_then(|id| i32::try_from(id).ok())
            .and_then(rustix::process::Pid::from_raw)
        {
            match rustix::process::kill_process_group(group, rustix::process::Signal::KILL) {
                Ok(()) | Err(rustix::io::Errno::SRCH) => {}
                Err(error) => eprintln!("Cannot stop local chat process group: {error}"),
            }
        }
        #[cfg(windows)]
        if let Some(pid) = *pid {
            if let Err(error) = std::process::Command::new("taskkill")
                .args(["/F", "/T", "/PID", &pid.to_string()])
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
            {
                eprintln!("Cannot stop local chat process tree: {error}");
            }
        }
    }
}

impl Process {
    pub(super) fn spawn(
        command: &mut Command,
        control: &ProcessControl,
    ) -> Result<Self, ChatError> {
        let mut registered = control.process.lock().map_err(|_| ChatError::State)?;
        if *control.cancel.borrow() {
            return Err(ChatError::Cancelled);
        }
        command.kill_on_drop(true);
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.as_std_mut().process_group(0);
        }
        let child = command.spawn().map_err(io("start local CLI"))?;
        *registered = child.id();
        Ok(Self {
            child,
            registered: Arc::clone(&control.process),
        })
    }

    pub(super) async fn stop(&mut self) {
        kill_registered(&self.registered);
        match tokio::time::timeout(Duration::from_secs(2), self.child.kill()).await {
            Ok(Ok(())) => {}
            Ok(Err(error)) => eprintln!("Cannot reap local chat process: {error}"),
            Err(_) => eprintln!("Timed out reaping local chat process"),
        }
    }
}

impl Drop for Process {
    fn drop(&mut self) {
        kill_registered(&self.registered);
        if let Ok(mut pid) = self.registered.lock() {
            *pid = None;
        }
        // Child also has kill_on_drop, including on platforms without process groups.
    }
}

pub(super) async fn read_bounded(
    reader: impl AsyncRead + Unpin,
    limit: usize,
) -> Result<Vec<u8>, ChatError> {
    let bound = u64::try_from(limit)
        .map_err(|_| ChatError::OutputLimit)?
        .saturating_add(1);
    let mut bytes = Vec::new();
    reader
        .take(bound)
        .read_to_end(&mut bytes)
        .await
        .map_err(io("read CLI output"))?;
    if bytes.len() > limit {
        return Err(ChatError::OutputLimit);
    }
    Ok(bytes)
}

#[derive(Debug)]
pub(super) struct Captured<T = Vec<u8>> {
    pub status: ExitStatus,
    pub stdout: T,
    pub stderr: Vec<u8>,
}

async fn read_diagnostics(
    mut stderr: impl AsyncRead + Unpin,
    output_closed: &mut watch::Receiver<bool>,
) -> Result<Vec<u8>, ChatError> {
    let mut bytes = Vec::new();
    let mut buffer = [0u8; 4096];
    loop {
        tokio::select! {
            biased;
            read = stderr.read(&mut buffer) => {
                let length = read.map_err(io("读取工具诊断信息"))?;
                if length == 0 { return Ok(bytes); }
                if bytes.len().saturating_add(length) > DIAGNOSTIC_LIMIT { return Err(ChatError::OutputLimit); }
                bytes.extend_from_slice(&buffer[..length]);
            }
            _ = output_closed.changed() => return Ok(bytes),
        }
    }
}

pub(super) struct ChildInput<'a> {
    pub command: &'a mut Command,
    pub prompt: &'a str,
}

async fn run<T, F, R>(
    input: ChildInput<'_>,
    control: &mut ProcessControl,
    read_stdout: F,
) -> Result<Captured<T>, ChatError>
where
    F: FnOnce(tokio::process::ChildStdout, Arc<Mutex<Option<u32>>>) -> R,
    R: Future<Output = Result<T, ChatError>>,
{
    let ChildInput { command, prompt } = input;
    let mut process = Process::spawn(command, control)?;
    let mut stdin = process.child.stdin.take().ok_or(ChatError::Channel)?;
    let stdout = process.child.stdout.take().ok_or(ChatError::Channel)?;
    let stderr = process.child.stderr.take().ok_or(ChatError::Channel)?;
    let (output_done, mut output_closed) = watch::channel(false);
    let operation = async {
        let diagnostics = read_diagnostics(stderr, &mut output_closed);
        let output = async {
            let result = read_stdout(stdout, Arc::clone(&control.process)).await;
            // A terminal result must not wait for a descendant holding stderr open.
            output_done.send_replace(true);
            result
        };
        let input = async {
            let result = stdin.write_all(prompt.as_bytes()).await;
            drop(stdin);
            // A CLI can close stdin early to report authentication or argument errors.
            match result {
                Ok(()) => Ok(()),
                Err(error) if error.kind() == std::io::ErrorKind::BrokenPipe => Ok(()),
                Err(error) => Err(io("send prompt to CLI")(error)),
            }
        };
        let ((), stdout, stderr, status) = tokio::try_join!(input, output, diagnostics, async {
            process.child.wait().await.map_err(io("wait for local CLI"))
        })?;
        Ok(Captured {
            status,
            stdout,
            stderr,
        })
    };
    let result = tokio::select! {
        biased;
        () = cancelled(&mut control.cancel) => Err(ChatError::Cancelled),
        result = tokio::time::timeout(control.duration, operation) => result.unwrap_or(Err(ChatError::Timeout)),
    };
    // Cleanup runs before returning or releasing the temp directory, even after early errors.
    process.stop().await;
    result
}

pub(super) async fn capture(
    command: &mut Command,
    prompt: &str,
    control: &mut ProcessControl,
) -> Result<Captured, ChatError> {
    run(ChildInput { command, prompt }, control, |stdout, _| {
        read_bounded(stdout, RESPONSE_LIMIT)
    })
    .await
}

pub(super) struct ChatInput<'a> {
    pub prompt: &'a str,
    pub events: super::stream::Sink,
}

pub(super) async fn execute(
    mut invocation: Invocation,
    input: ChatInput<'_>,
    control: &mut ProcessControl,
) -> Result<(), ChatError> {
    // The runtime directory owns temporary provider adapters, never the project itself.
    let _runtime = invocation.directory;
    let _project = invocation.project_root;
    let parser = super::stream::Parser::new(invocation.tool, input.events);
    let output = run(
        ChildInput {
            command: &mut invocation.command,
            prompt: input.prompt,
        },
        control,
        |stdout, process| async move { super::stream::read(stdout, parser, &process).await },
    )
    .await?;
    // A terminal protocol result is authoritative even if cleanup had to kill a lingering CLI.
    if !output.stdout.terminal && !output.status.success() {
        return Err(cli_failure(
            invocation.tool,
            &String::from_utf8_lossy(&output.stderr),
            FailureOrigin::Command,
        ));
    }
    output.stdout.finish()
}

/// Compatibility CLIs receive the complete caller-supplied prompt/history unchanged.
pub(super) async fn execute_compatibility(
    invocation: Invocation,
    mut input: ChatInput<'_>,
    control: &mut ProcessControl,
) -> Result<(), ChatError> {
    (input.events)(super::LocalChatEvent::Status{message:"Native session was not resumed. Continuing with the complete supplied prompt and chat history using the compatibility CLI.".into()})?;
    execute(invocation, input, control).await
}
