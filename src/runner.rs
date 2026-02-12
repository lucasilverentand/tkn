use std::process::Command;
use std::time::Instant;

use crate::types::CommandResult;

pub fn run_command(command: &str) -> (CommandResult, u64) {
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());

    let start = Instant::now();
    let output = Command::new(&shell)
        .arg("-c")
        .arg(command)
        .output();

    let duration_ms = start.elapsed().as_millis() as u64;

    match output {
        Ok(output) => {
            let exit_code = output.status.code().unwrap_or(1);
            (
                CommandResult {
                    stdout: output.stdout,
                    stderr: output.stderr,
                    exit_code,
                },
                duration_ms,
            )
        }
        Err(e) => {
            let err_msg = format!("tkn: failed to execute shell: {e}\n");
            (
                CommandResult {
                    stdout: Vec::new(),
                    stderr: err_msg.into_bytes(),
                    exit_code: 127,
                },
                duration_ms,
            )
        }
    }
}
