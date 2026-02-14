/// Environment variable that carries the original command string verbatim
/// from the hook, bypassing shell arg splitting.
const ENV_ORIGINAL_CMD: &str = "TKN_ORIGINAL_CMD";

pub fn run(args: &[String]) -> i32 {
    let command = std::env::var(ENV_ORIGINAL_CMD)
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| args.join(" "));

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
