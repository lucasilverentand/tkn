/// Package runner wrappers that should be stripped before matching.
const PACKAGE_RUNNERS: &[&str] = &["npx", "pnpx", "bunx"];

/// Patterns that indicate a long-running / streaming process.
const LONG_LIVED_PREFIXES: &[&str] = &[
    // Node / JS
    "npm run dev",
    "npm run serve",
    "npm run watch",
    "npm run start",
    "npm start",
    "yarn dev",
    "yarn start",
    "pnpm dev",
    "pnpm start",
    "bun dev",
    "bun start",
    "bun run dev",
    // Meta-frameworks
    "next dev",
    "nuxt dev",
    "remix dev",
    "astro dev",
    "expo start",
    "ng serve",
    "vite dev",
    "vite preview",
    "webpack serve",
    "storybook dev",
    "turbo dev",
    "turbo watch",
    // Python
    "python manage.py runserver",
    "python -m http.server",
    "flask run",
    "uvicorn",
    "gunicorn",
    // Ruby
    "rails server",
    // PHP
    "php artisan serve",
    "php -S",
    // Go
    "go run",
    // Rust
    "cargo watch",
    "cargo run",
    // Deno
    "deno task dev",
    "deno serve",
    // .NET
    "dotnet run",
    "dotnet watch",
    // Elixir
    "mix phx.server",
    // Static site generators
    "hugo server",
    "hugo serve",
    "jekyll serve",
    // Docker / k8s
    "docker compose up",
    "docker-compose up",
    "docker logs -f",
    "docker logs --follow",
    "kubectl logs -f",
    "kubectl logs --follow",
    "kubectl port-forward",
    // System
    "tail -f",
    "tail --follow",
    "watch ",
    // Misc
    "caddy run",
];

/// Commands that are long-lived only when they match exactly (no subcommands).
const LONG_LIVED_EXACT: &[&str] = &[
    "air",
    "nodemon",
    "live-server",
    "http-server",
    "concurrently",
    "vite",
    "rails s",
    "serve",
];

/// Flags that indicate a long-running / streaming mode.
const LONG_LIVED_FLAGS: &[&str] = &["--watch", "--serve", "--live-reload", "--hot"];

/// Strip package runner prefix (npx, pnpx, bunx) if present.
fn strip_package_runner(cmd: &str) -> &str {
    for runner in PACKAGE_RUNNERS {
        if let Some(rest) = cmd.strip_prefix(runner) {
            if let Some(stripped) = rest.strip_prefix(' ') {
                return stripped.trim_start();
            }
        }
    }
    cmd
}

pub fn is_long_lived(command: &str) -> bool {
    let cmd = command.trim();
    let effective = strip_package_runner(cmd);

    for pattern in LONG_LIVED_PREFIXES {
        if effective.starts_with(pattern) {
            return true;
        }
    }
    for exact in LONG_LIVED_EXACT {
        if effective == *exact || effective.starts_with(&format!("{exact} ")) {
            return true;
        }
    }
    for flag in LONG_LIVED_FLAGS {
        if cmd.contains(&format!(" {flag}")) {
            return true;
        }
    }
    false
}

/// Returns true if the command should be passed through without wrapping.
pub fn should_skip(command: &str) -> bool {
    command.is_empty()
        || command.starts_with("tkn ")
        || command.trim_start().starts_with('#')
        || has_complex_syntax(command)
}

/// Detects commands with pipes, logical operators, semicolons, or subshells.
/// These are intentionally complex and their output should not be optimized.
pub fn has_complex_syntax(command: &str) -> bool {
    let mut in_single = false;
    let mut in_double = false;
    let mut chars = command.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '\\' if !in_single => {
                chars.next(); // skip escaped char
            }
            '\'' if !in_double => in_single = !in_single,
            '"' if !in_single => in_double = !in_double,
            _ if in_single || in_double => {}
            '|' | ';' => return true,
            '&' => {
                if chars.peek() == Some(&'&') {
                    return true; // &&
                }
                return true; // background &
            }
            '(' | ')' => return true, // subshell
            _ => {}
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pipe() {
        assert!(has_complex_syntax("ls | grep foo"));
        assert!(has_complex_syntax("git log | head -20"));
    }

    #[test]
    fn test_and_chain() {
        assert!(has_complex_syntax("mkdir foo && cd foo"));
    }

    #[test]
    fn test_semicolon() {
        assert!(has_complex_syntax("ls; pwd"));
    }

    #[test]
    fn test_subshell() {
        assert!(has_complex_syntax("(cd /tmp && ls)"));
    }

    #[test]
    fn test_background() {
        assert!(has_complex_syntax("sleep 10 &"));
    }

    #[test]
    fn test_comments_skipped() {
        assert!(should_skip("# This is a comment"));
        assert!(should_skip("  # indented comment"));
        assert!(!should_skip("echo '# not a comment'"));
    }

    #[test]
    fn test_simple_commands() {
        assert!(!has_complex_syntax("git diff"));
        assert!(!has_complex_syntax("cargo test --release"));
        assert!(!has_complex_syntax("ls -la /tmp"));
    }

    #[test]
    fn test_quoted_pipes_ignored() {
        assert!(!has_complex_syntax(r#"echo "foo | bar""#));
        assert!(!has_complex_syntax("echo 'foo | bar'"));
        assert!(!has_complex_syntax(r#"grep "a && b" file.txt"#));
    }

    #[test]
    fn test_escaped_chars() {
        assert!(!has_complex_syntax(r"echo foo\|bar"));
        assert!(!has_complex_syntax(r"echo foo\;bar"));
    }

    #[test]
    fn test_complex_real_world() {
        assert!(has_complex_syntax(
            "gh issue list --state all --limit 500 --json number | jq -r '.[].number' | xargs -I {} gh issue delete {} --yes 2>&1 | tail -5"
        ));
    }

    #[test]
    fn test_long_lived_basic_prefixes() {
        assert!(is_long_lived("npm start"));
        assert!(is_long_lived("cargo run"));
        assert!(is_long_lived("flask run"));
        assert!(is_long_lived("rails server"));
        assert!(is_long_lived("docker compose up"));
        assert!(is_long_lived("tail -f /var/log/syslog"));
        assert!(is_long_lived("hugo server --port 3000"));
        assert!(is_long_lived("kubectl port-forward svc/api 8080:80"));
    }

    #[test]
    fn test_long_lived_exact_match() {
        assert!(is_long_lived("vite"));
        assert!(is_long_lived("vite --port 3000"));
        assert!(is_long_lived("air"));
        assert!(is_long_lived("nodemon server.js"));
        assert!(is_long_lived("serve"));
        assert!(is_long_lived("serve ."));
        assert!(is_long_lived("rails s"));
        assert!(is_long_lived("rails s -p 4000"));
    }

    #[test]
    fn test_long_lived_no_false_positives_on_exact() {
        // "vite" should not match "vitest"
        assert!(!is_long_lived("vitest"));
        assert!(!is_long_lived("vitest run"));
        // "air" should not match "airflow"
        assert!(!is_long_lived("airflow"));
        // "rails s" should not match "rails stats"
        assert!(!is_long_lived("rails stats"));
        // "serve" should not match "serverless"
        assert!(!is_long_lived("serverless deploy"));
    }

    #[test]
    fn test_long_lived_package_runners() {
        assert!(is_long_lived("npx vite"));
        assert!(is_long_lived("npx vite --port 3000"));
        assert!(is_long_lived("bunx next dev"));
        assert!(is_long_lived("pnpx nuxt dev"));
        assert!(is_long_lived("npx nodemon server.js"));
        // Runner without a long-lived command
        assert!(!is_long_lived("npx eslint ."));
        assert!(!is_long_lived("bunx vitest"));
    }

    #[test]
    fn test_long_lived_flags() {
        assert!(is_long_lived("cargo build --watch"));
        assert!(is_long_lived("webpack --watch"));
        assert!(is_long_lived("tsc --watch"));
        assert!(is_long_lived("parcel index.html --hot"));
        assert!(is_long_lived("eleventy --serve"));
    }

    #[test]
    fn test_not_long_lived() {
        assert!(!is_long_lived("cargo build"));
        assert!(!is_long_lived("cargo test"));
        assert!(!is_long_lived("npm install"));
        assert!(!is_long_lived("go build ./..."));
        assert!(!is_long_lived("git diff"));
        assert!(!is_long_lived("ls -la"));
        assert!(!is_long_lived("dotnet test"));
        assert!(!is_long_lived("python -m pytest"));
    }
}
