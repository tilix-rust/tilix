use std::env;
use std::path::PathBuf;

/// Parses an OSC 7 file URI into a canonical local PathBuf.
/// Handles `file://localhost/path`, `file:///path`, and percent-encoded characters.
pub fn parse_osc7_uri(uri: &str) -> Option<PathBuf> {
    if !uri.starts_with("file://") {
        return None;
    }
    let after_scheme = &uri["file://".len()..];
    let path_str = if after_scheme.starts_with('/') {
        after_scheme
    } else {
        let slash_pos = after_scheme.find('/')?;
        &after_scheme[slash_pos..]
    };

    let mut decoded = Vec::new();
    let bytes = path_str.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            if i + 2 >= bytes.len() {
                return None;
            }
            let h1 = char::from(bytes[i + 1]).to_digit(16)? as u8;
            let h2 = char::from(bytes[i + 2]).to_digit(16)? as u8;
            decoded.push((h1 << 4) | h2);
            i += 3;
        } else {
            decoded.push(bytes[i]);
            i += 1;
        }
    }
    let s = String::from_utf8(decoded).ok()?;
    Some(PathBuf::from(s))
}

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

    #[test]
    fn test_parse_osc7_uri_standard() {
        let path = parse_osc7_uri("file:///home/user/project");
        assert_eq!(path, Some(PathBuf::from("/home/user/project")));
    }

    #[test]
    fn test_parse_osc7_uri_with_localhost() {
        let path = parse_osc7_uri("file://localhost/home/user/project");
        assert_eq!(path, Some(PathBuf::from("/home/user/project")));
    }

    #[test]
    fn test_parse_osc7_uri_with_percent_encoding() {
        let path = parse_osc7_uri("file:///home/user/my%20documents/test%20dir");
        assert_eq!(path, Some(PathBuf::from("/home/user/my documents/test dir")));
    }

    #[test]
    fn test_parse_osc7_uri_invalid_scheme() {
        assert_eq!(parse_osc7_uri("http://localhost/home/user"), None);
        assert_eq!(parse_osc7_uri("ftp:///home/user"), None);
        assert_eq!(parse_osc7_uri("/home/user"), None);
        assert_eq!(parse_osc7_uri("file://"), None);
    }
}
