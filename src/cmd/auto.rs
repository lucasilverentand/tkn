use crate::cmd::{exec, pass, routing};
use crate::shell;

/// Route a command to `exec` (optimized) or `pass` (streaming) automatically.
pub fn run(args: &[String]) -> i32 {
    let command = shell::args_to_shell_command(args);

    if command.is_empty() {
        eprintln!("tkn: no command provided");
        return 1;
    }

    if routing::should_skip(&command) || routing::is_long_lived(&command) {
        pass::run(args)
    } else {
        exec::run(args)
    }
}
