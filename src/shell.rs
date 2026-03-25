/// Build a shell command string from parsed argv.
///
/// `tkn hook run` passes the entire original command as a single argument,
/// so we preserve single-arg inputs verbatim. For normal CLI usage with
/// multiple args, quote each arg so shell metacharacters stay literal.
pub fn args_to_shell_command(args: &[String]) -> String {
    match args {
        [] => String::new(),
        [single] => single.clone(),
        _ => args
            .iter()
            .map(|arg| shell_escape_arg(arg))
            .collect::<Vec<_>>()
            .join(" "),
    }
}

fn shell_escape_arg(arg: &str) -> String {
    if arg.is_empty() {
        return "''".to_string();
    }

    if arg
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || "_@%+=:,./-".contains(c))
    {
        return arg.to_string();
    }

    format!("'{}'", arg.replace('\'', "'\\''"))
}

#[cfg(test)]
mod tests {
    use super::args_to_shell_command;

    #[test]
    fn preserves_single_arg_raw_command() {
        let args = vec!["rg -n 'a|b' src".to_string()];
        assert_eq!(args_to_shell_command(&args), "rg -n 'a|b' src");
    }

    #[test]
    fn escapes_multi_arg_metacharacters() {
        let args = vec![
            "rg".to_string(),
            "-n".to_string(),
            "a|b".to_string(),
            "src".to_string(),
        ];
        assert_eq!(args_to_shell_command(&args), "rg -n 'a|b' src");
    }

    #[test]
    fn escapes_single_quotes_in_args() {
        let args = vec!["echo".to_string(), "it's".to_string()];
        assert_eq!(args_to_shell_command(&args), "echo 'it'\\''s'");
    }
}
