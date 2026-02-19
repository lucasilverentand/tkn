use crate::shell;

pub fn run(args: &[String]) -> i32 {
    let command = shell::args_to_shell_command(args);

    if command.is_empty() {
        eprintln!("tkn: no command provided");
        return 1;
    }

    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());

    let status = std::process::Command::new(&shell)
        .arg("-c")
        .arg(&command)
        .status();

    match status {
        Ok(s) => s.code().unwrap_or(1),
        Err(e) => {
            eprintln!("tkn: failed to execute command: {e}");
            1
        }
    }
}
