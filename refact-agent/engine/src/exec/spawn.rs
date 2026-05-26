use std::process::Stdio;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use process_wrap::tokio::{TokioChildWrapper, TokioCommandWrap};
#[cfg(unix)]
use process_wrap::tokio::ProcessGroup;
#[cfg(windows)]
use process_wrap::tokio::JobObject;
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::sync::{mpsc, Mutex, Notify};
use tokio::task::JoinHandle;

use crate::exec::registry::{ExecProcessCommand, ExecProcessRuntime};
use crate::exec::types::{
    ExecMode, ExecOutputStream, ExecProcessMeta, ExecProcessSnapshot, ExecSpawnRequest, ExecStatus,
};
use crate::exec::ExecRegistry;

const PIPE_READ_BYTES: usize = 8192;
const KILL_REAP_TIMEOUT: Duration = Duration::from_secs(2);
const KILL_PUMP_DRAIN_TIMEOUT: Duration = Duration::from_millis(500);
const ABORT_POLL_INTERVAL: Duration = Duration::from_millis(50);

pub struct ExecSpawnResult {
    pub snapshot: ExecProcessSnapshot,
}

fn shell_command(request: &ExecSpawnRequest) -> Result<tokio::process::Command, String> {
    if request.command.trim().is_empty() {
        return Err("Command is empty".to_string());
    }

    let shell = if cfg!(target_os = "windows") {
        "powershell.exe"
    } else {
        "sh"
    };
    let shell_arg = if cfg!(target_os = "windows") {
        "-Command"
    } else {
        "-c"
    };
    let mut command = tokio::process::Command::new(shell);
    command.kill_on_drop(true);
    command.arg(shell_arg).arg(&request.command);
    command.stdin(Stdio::null());
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());
    if let Some(cwd) = request.cwd.as_ref() {
        command.current_dir(cwd);
    }
    for (key, value) in &request.env {
        command.env(key, value);
    }
    Ok(command)
}

fn wrap_command(command: tokio::process::Command) -> TokioCommandWrap {
    let mut command_wrap = TokioCommandWrap::from(command);
    #[cfg(unix)]
    command_wrap.wrap(ProcessGroup::leader());
    #[cfg(windows)]
    command_wrap.wrap(JobObject);
    command_wrap
}

fn output_to_text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(&strip_ansi_escapes::strip(bytes)).to_string()
}

fn pump_output(
    registry: ExecRegistry,
    process_id: crate::exec::types::ExecProcessId,
    stream: ExecOutputStream,
    mut pipe: impl AsyncRead + Unpin + Send + 'static,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut buffer = [0; PIPE_READ_BYTES];
        loop {
            match pipe.read(&mut buffer).await {
                Ok(0) => break,
                Ok(bytes_read) => {
                    let text = output_to_text(&buffer[..bytes_read]);
                    if !text.is_empty() {
                        let _ = registry
                            .append_output(&process_id, stream.clone(), text)
                            .await;
                    }
                }
                Err(error) => {
                    tracing::warn!("exec output pump failed for {process_id}: {error}");
                    break;
                }
            }
        }
    })
}

async fn await_pump(handle: JoinHandle<()>) {
    let _ = handle.await;
}

async fn finish_pumps(stdout_task: JoinHandle<()>, stderr_task: JoinHandle<()>) {
    let _ = tokio::join!(await_pump(stdout_task), await_pump(stderr_task));
}

async fn finish_pumps_with_timeout(
    mut stdout_task: JoinHandle<()>,
    mut stderr_task: JoinHandle<()>,
    timeout: Duration,
) {
    let wait = async {
        let _ = tokio::join!(&mut stdout_task, &mut stderr_task);
    };
    if tokio::time::timeout(timeout, wait).await.is_err() {
        stdout_task.abort();
        stderr_task.abort();
    }
}

async fn kill_and_reap(child: &Arc<Mutex<Box<dyn TokioChildWrapper>>>) -> Result<(), String> {
    let mut child = child.lock().await;
    let kill_result = child.start_kill();
    let wait_result = tokio::time::timeout(KILL_REAP_TIMEOUT, Box::into_pin(child.wait())).await;
    match (kill_result, wait_result) {
        (Ok(()), Ok(Ok(_))) => Ok(()),
        (Err(kill_error), Ok(Ok(_))) => Err(format!("failed to kill process: {kill_error}")),
        (Ok(()), Ok(Err(wait_error))) => Err(format!("failed to reap process: {wait_error}")),
        (Err(kill_error), Ok(Err(wait_error))) => Err(format!(
            "failed to kill process: {kill_error}; failed to reap process: {wait_error}"
        )),
        (Ok(()), Err(_)) => Err("timed out while reaping process".to_string()),
        (Err(kill_error), Err(_)) => Err(format!(
            "failed to kill process: {kill_error}; timed out while reaping process"
        )),
    }
}

async fn wait_child(child: &Arc<Mutex<Box<dyn TokioChildWrapper>>>) -> Result<Option<i32>, String> {
    let mut child = child.lock().await;
    let status = Box::into_pin(child.wait())
        .await
        .map_err(|error| format!("failed to wait for process: {error}"))?;
    Ok(status.code())
}

async fn try_wait_child(
    child: &Arc<Mutex<Box<dyn TokioChildWrapper>>>,
) -> Result<Option<Option<i32>>, String> {
    let mut child = child.lock().await;
    child
        .try_wait()
        .map(|status| status.map(|status| status.code()))
        .map_err(|error| format!("failed to check process status: {error}"))
}

async fn status_or_killed(child: &Arc<Mutex<Box<dyn TokioChildWrapper>>>) -> ExecStatus {
    match try_wait_child(child).await {
        Ok(Some(exit_code)) => ExecStatus::Exited { exit_code },
        Ok(None) => ExecStatus::Killed,
        Err(message) => ExecStatus::Failed { message },
    }
}

async fn status_or_timed_out(child: &Arc<Mutex<Box<dyn TokioChildWrapper>>>) -> ExecStatus {
    match try_wait_child(child).await {
        Ok(Some(exit_code)) => ExecStatus::Exited { exit_code },
        Ok(None) => ExecStatus::TimedOut,
        Err(message) => ExecStatus::Failed { message },
    }
}

async fn monitor_process(
    registry: ExecRegistry,
    process_id: crate::exec::types::ExecProcessId,
    child: Arc<Mutex<Box<dyn TokioChildWrapper>>>,
    mut control_rx: mpsc::Receiver<ExecProcessCommand>,
    timeout: Option<Duration>,
    abort_flag: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
    stdout_task: JoinHandle<()>,
    stderr_task: JoinHandle<()>,
) {
    let (terminal_status, kill_response) = loop {
        let abort_wait = async {
            loop {
                tokio::time::sleep(ABORT_POLL_INTERVAL).await;
                if abort_flag
                    .as_ref()
                    .map(|flag| flag.load(Ordering::Relaxed))
                    .unwrap_or(false)
                {
                    break;
                }
            }
        };

        match timeout {
            Some(timeout) => {
                tokio::select! {
                    result = wait_child(&child) => {
                        break (
                            match result {
                                Ok(exit_code) => ExecStatus::Exited { exit_code },
                                Err(message) => ExecStatus::Failed { message },
                            },
                            None,
                        );
                    }
                    _ = tokio::time::sleep(timeout) => {
                        break (status_or_timed_out(&child).await, None);
                    }
                    _ = abort_wait => {
                        break (status_or_killed(&child).await, None);
                    }
                    command = control_rx.recv() => {
                        if let Some(ExecProcessCommand::Kill { response }) = command {
                            let status = status_or_killed(&child).await;
                            break (status, Some(response));
                        }
                    }
                }
            }
            None => {
                tokio::select! {
                    result = wait_child(&child) => {
                        break (
                            match result {
                                Ok(exit_code) => ExecStatus::Exited { exit_code },
                                Err(message) => ExecStatus::Failed { message },
                            },
                            None,
                        );
                    }
                    _ = abort_wait => {
                        break (status_or_killed(&child).await, None);
                    }
                    command = control_rx.recv() => {
                        if let Some(ExecProcessCommand::Kill { response }) = command {
                            let status = status_or_killed(&child).await;
                            break (status, Some(response));
                        }
                    }
                }
            }
        }
    };

    match terminal_status {
        ExecStatus::TimedOut | ExecStatus::Killed => {
            if let Err(error) = kill_and_reap(&child).await {
                tracing::warn!("exec kill/reap failed for {process_id}: {error}");
            }
        }
        ExecStatus::Starting
        | ExecStatus::Running
        | ExecStatus::Exited { .. }
        | ExecStatus::Failed { .. } => {}
    }
    match terminal_status {
        ExecStatus::TimedOut | ExecStatus::Killed => {
            finish_pumps_with_timeout(stdout_task, stderr_task, KILL_PUMP_DRAIN_TIMEOUT).await;
        }
        ExecStatus::Starting
        | ExecStatus::Running
        | ExecStatus::Exited { .. }
        | ExecStatus::Failed { .. } => {
            finish_pumps(stdout_task, stderr_task).await;
        }
    }
    let final_snapshot = registry.complete_status(&process_id, terminal_status).await;
    if let Some(response) = kill_response {
        let _ = response.send(final_snapshot);
    }
}

impl ExecRegistry {
    pub async fn spawn(&self, request: ExecSpawnRequest) -> Result<ExecSpawnResult, String> {
        let mut command = wrap_command(shell_command(&request)?);
        let mut child = command
            .spawn()
            .map_err(|error| format!("failed to spawn command: {error}"))?;
        let stdout = child
            .stdout()
            .take()
            .ok_or_else(|| "failed to capture stdout".to_string())?;
        let stderr = child
            .stderr()
            .take()
            .ok_or_else(|| "failed to capture stderr".to_string())?;
        let mut meta = ExecProcessMeta::new(request.mode.clone(), request.command.clone())
            .with_owner(request.owner.clone());
        if let Some(cwd) = request.cwd.clone() {
            meta = meta.with_cwd(cwd);
        }
        if let Some(short_description) = request.short_description.clone() {
            meta = meta.with_short_description(short_description);
        }
        let startup_wait = request.startup_wait;
        let process_id = meta.process_id.clone();
        self.register(meta, request.output_limits.transcript_max_bytes)
            .await;
        let child = Arc::new(Mutex::new(child));
        let stdout_task = pump_output(
            self.clone(),
            process_id.clone(),
            ExecOutputStream::Stdout,
            stdout,
        );
        let stderr_task = pump_output(
            self.clone(),
            process_id.clone(),
            ExecOutputStream::Stderr,
            stderr,
        );
        let (control_tx, control_rx) = mpsc::channel(8);
        let terminal = Arc::new(Notify::new());
        tokio::spawn(monitor_process(
            self.clone(),
            process_id.clone(),
            child,
            control_rx,
            request.timeout,
            request.abort_flag.clone(),
            stdout_task,
            stderr_task,
        ));
        self.attach_runtime(
            &process_id,
            ExecProcessRuntime {
                control_tx,
                terminal,
            },
        )
        .await?;
        let snapshot = self.mark_started(&process_id).await?;
        if matches!(request.mode, ExecMode::Foreground) {
            return Ok(ExecSpawnResult {
                snapshot: self.wait(&process_id).await?,
            });
        }
        if let Some(startup_wait) = startup_wait {
            tokio::time::sleep(startup_wait).await;
        }
        Ok(ExecSpawnResult {
            snapshot: self.get(&process_id).await.unwrap_or(snapshot),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};

    use super::*;
    use crate::exec::types::{ExecProcessFilter, ExecStatusKind};

    #[cfg(windows)]
    fn shell_script(script: &str) -> String {
        script.to_string()
    }

    #[cfg(not(windows))]
    fn shell_script(script: &str) -> String {
        script.to_string()
    }

    #[tokio::test]
    async fn foreground_success_captures_stdout() {
        let registry = ExecRegistry::new();
        let result = registry
            .spawn(ExecSpawnRequest::foreground(shell_script("printf hello")))
            .await
            .unwrap();

        assert_eq!(
            result.snapshot.status,
            ExecStatus::Exited { exit_code: Some(0) }
        );
        let read = registry
            .read(&result.snapshot.meta.process_id, 0, None)
            .await;
        assert_eq!(read.chunks.len(), 1);
        assert_eq!(read.chunks[0].stream, ExecOutputStream::Stdout);
        assert_eq!(read.chunks[0].text, "hello");
    }

    #[tokio::test]
    async fn foreground_captures_stderr() {
        let registry = ExecRegistry::new();
        let command = if cfg!(windows) {
            "[Console]::Error.Write('warn')"
        } else {
            "printf warn >&2"
        };
        let result = registry
            .spawn(ExecSpawnRequest::foreground(shell_script(command)))
            .await
            .unwrap();

        assert_eq!(
            result.snapshot.status,
            ExecStatus::Exited { exit_code: Some(0) }
        );
        let read = registry
            .read(&result.snapshot.meta.process_id, 0, None)
            .await;
        assert_eq!(read.chunks.len(), 1);
        assert_eq!(read.chunks[0].stream, ExecOutputStream::Stderr);
        assert_eq!(read.chunks[0].text, "warn");
    }

    #[tokio::test]
    async fn foreground_reports_non_zero_exit_code() {
        let registry = ExecRegistry::new();
        let command = if cfg!(windows) { "exit 7" } else { "exit 7" };
        let result = registry
            .spawn(ExecSpawnRequest::foreground(shell_script(command)))
            .await
            .unwrap();

        assert_eq!(
            result.snapshot.status,
            ExecStatus::Exited { exit_code: Some(7) }
        );
    }

    #[tokio::test]
    async fn timeout_kills_and_keeps_partial_output() {
        let registry = ExecRegistry::new();
        let command = if cfg!(windows) {
            "[Console]::Out.Write('start'); Start-Sleep -Seconds 5"
        } else {
            "printf start; sleep 5"
        };
        let result = registry
            .spawn(
                ExecSpawnRequest::foreground(shell_script(command))
                    .with_timeout(Duration::from_millis(200)),
            )
            .await
            .unwrap();

        assert_eq!(result.snapshot.status, ExecStatus::TimedOut);
        let read = registry
            .read(&result.snapshot.meta.process_id, 0, None)
            .await;
        assert!(read.chunks.iter().any(|chunk| chunk.text.contains("start")));
    }

    #[tokio::test]
    async fn abort_flag_kills_and_keeps_partial_output() {
        let registry = ExecRegistry::new();
        let abort_flag = Arc::new(AtomicBool::new(false));
        let command = if cfg!(windows) {
            "[Console]::Out.Write('start'); Start-Sleep -Seconds 5"
        } else {
            "printf start; sleep 5"
        };
        let request = ExecSpawnRequest::foreground(shell_script(command))
            .with_abort_flag(abort_flag.clone())
            .with_timeout(Duration::from_secs(10));
        let abort_task = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(200)).await;
            abort_flag.store(true, Ordering::Relaxed);
        });
        let result = registry.spawn(request).await.unwrap();
        abort_task.await.unwrap();

        assert_eq!(result.snapshot.status, ExecStatus::Killed);
        let read = registry
            .read(&result.snapshot.meta.process_id, 0, None)
            .await;
        assert!(read.chunks.iter().any(|chunk| chunk.text.contains("start")));
    }

    #[tokio::test]
    async fn large_output_is_bounded() {
        let registry = ExecRegistry::new();
        let command = if cfg!(windows) {
            "[Console]::Out.Write(('x' * 4096))"
        } else {
            "python3 -c 'print(\"x\" * 4096, end=\"\")'"
        };
        let result = registry
            .spawn(ExecSpawnRequest::foreground(shell_script(command)).with_transcript_limit(1024))
            .await
            .unwrap();

        assert_eq!(
            result.snapshot.status,
            ExecStatus::Exited { exit_code: Some(0) }
        );
        let read = registry
            .read(&result.snapshot.meta.process_id, 0, None)
            .await;
        assert!(read.current_bytes <= 1024);
        assert!(read.is_truncated);
    }

    #[tokio::test]
    async fn background_can_be_listed_read_and_killed() {
        let registry = ExecRegistry::new();
        let command = if cfg!(windows) {
            "[Console]::Out.Write('ready'); Start-Sleep -Seconds 5"
        } else {
            "printf ready; sleep 5"
        };
        let result = registry
            .spawn(ExecSpawnRequest::background(shell_script(command)))
            .await
            .unwrap();
        assert_eq!(result.snapshot.status, ExecStatus::Running);

        let listed = registry
            .list(ExecProcessFilter {
                status: Some(ExecStatusKind::Running),
                ..ExecProcessFilter::default()
            })
            .await;
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].meta.process_id, result.snapshot.meta.process_id);

        tokio::time::sleep(Duration::from_millis(100)).await;
        let read = registry
            .read(&result.snapshot.meta.process_id, 0, None)
            .await;
        assert!(read.chunks.iter().any(|chunk| chunk.text.contains("ready")));

        let killed = registry
            .kill(&result.snapshot.meta.process_id)
            .await
            .unwrap();
        assert_eq!(killed.status, ExecStatus::Killed);
        let waited = registry
            .wait(&result.snapshot.meta.process_id)
            .await
            .unwrap();
        assert_eq!(waited.status, ExecStatus::Killed);
    }
}
