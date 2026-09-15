use std::env;

pub fn resolve_shell(shell_var: Result<String, env::VarError>) -> String {
    match shell_var {
        Ok(s) if !s.trim().is_empty() => s,
        _ => "/bin/sh".to_string(),
    }
}

pub fn detect_shell() -> String {
    resolve_shell(env::var("SHELL"))
}

pub fn default_env() -> Vec<String> {
    let mut env = Vec::new();
    for (k, v) in env::vars() {
        if k != "TERM" && k != "COLORTERM" {
            env.push(format!("{k}={v}"));
        }
    }
    env.push("TERM=xterm-256color".to_string());
    env.push("COLORTERM=truecolor".to_string());
    env
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_shell() {
        let shell = detect_shell();
        assert!(!shell.trim().is_empty());
    }

    #[test]
    fn test_resolve_shell() {
        assert_eq!(resolve_shell(Ok("/bin/zsh".to_string())), "/bin/zsh");
        assert_eq!(resolve_shell(Ok("".to_string())), "/bin/sh");
        assert_eq!(resolve_shell(Ok("   \t\n ".to_string())), "/bin/sh");
        assert_eq!(resolve_shell(Err(env::VarError::NotPresent)), "/bin/sh");
    }

    #[test]
    fn test_default_env() {
        let env = default_env();
        assert!(env.iter().any(|e| e == "TERM=xterm-256color"));
        assert!(env.iter().any(|e| e == "COLORTERM=truecolor"));
    }
}
