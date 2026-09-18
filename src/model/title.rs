use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TitleEditScope {
    Terminal,
    Session,
    Window,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TokenDef {
    pub token: &'static str,
    pub scope: TitleEditScope,
    pub description: &'static str,
}

pub const TERMINAL_TOKEN_DEFS: &[TokenDef] = &[
    TokenDef { token: "${title}", scope: TitleEditScope::Terminal, description: "Terminal title" },
    TokenDef { token: "${iconTitle}", scope: TitleEditScope::Terminal, description: "Terminal icon title" },
    TokenDef { token: "${id}", scope: TitleEditScope::Terminal, description: "Terminal ID" },
    TokenDef { token: "${directory}", scope: TitleEditScope::Terminal, description: "Current directory" },
    TokenDef { token: "${hostname}", scope: TitleEditScope::Terminal, description: "Hostname" },
    TokenDef { token: "${username}", scope: TitleEditScope::Terminal, description: "Username" },
    TokenDef { token: "${columns}", scope: TitleEditScope::Terminal, description: "Terminal width in columns" },
    TokenDef { token: "${rows}", scope: TitleEditScope::Terminal, description: "Terminal height in rows" },
    TokenDef { token: "${process}", scope: TitleEditScope::Terminal, description: "Current process" },
    TokenDef { token: "${status.readonly}", scope: TitleEditScope::Terminal, description: "Read-only indicator" },
    TokenDef { token: "${status.silence}", scope: TitleEditScope::Terminal, description: "Silence monitor indicator" },
    TokenDef { token: "${status.input-sync}", scope: TitleEditScope::Terminal, description: "Synchronized input indicator" },
    TokenDef { token: "${profile}", scope: TitleEditScope::Terminal, description: "Profile name" },
];

pub const SESSION_TOKEN_DEFS: &[TokenDef] = &[
    TokenDef { token: "${activeTerminalTitle}", scope: TitleEditScope::Session, description: "Active terminal title" },
    TokenDef { token: "${terminalCount}", scope: TitleEditScope::Session, description: "Total terminals in session" },
    TokenDef { token: "${terminalNumber}", scope: TitleEditScope::Session, description: "Active terminal number" },
];

pub const WINDOW_TOKEN_DEFS: &[TokenDef] = &[
    TokenDef { token: "${appName}", scope: TitleEditScope::Window, description: "Application name" },
    TokenDef { token: "${sessionName}", scope: TitleEditScope::Window, description: "Active session name" },
    TokenDef { token: "${sessionNumber}", scope: TitleEditScope::Window, description: "Active session number" },
    TokenDef { token: "${sessionCount}", scope: TitleEditScope::Window, description: "Total sessions in window" },
];

#[derive(Debug, Clone, Default)]
pub struct TokenContext {
    // Terminal scope
    pub title: String,
    pub icon_title: Option<String>,
    pub id: Option<u64>,
    pub directory: Option<PathBuf>,
    pub hostname: Option<String>,
    pub username: Option<String>,
    pub columns: Option<u32>,
    pub rows: Option<u32>,
    pub process: Option<String>,
    pub readonly: bool,
    pub silence: bool,
    pub input_sync: bool,
    pub profile_name: Option<String>,

    // Session scope
    pub active_terminal_title: Option<String>,
    pub terminal_count: Option<usize>,
    pub terminal_number: Option<usize>,

    // Window scope
    pub app_name: Option<String>,
    pub session_name: Option<String>,
    pub session_number: Option<usize>,
    pub session_count: Option<usize>,
}

impl TokenContext {
    pub fn new_terminal(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            ..Default::default()
        }
    }

    pub fn new_session(title: impl Into<String>, terminal_count: usize, terminal_number: usize) -> Self {
        let t = title.into();
        Self {
            title: t.clone(),
            active_terminal_title: Some(t),
            terminal_count: Some(terminal_count),
            terminal_number: Some(terminal_number),
            ..Default::default()
        }
    }

    pub fn new_window(session_name: impl Into<String>, session_count: usize, session_number: usize) -> Self {
        let s = session_name.into();
        Self {
            title: s.clone(),
            session_name: Some(s),
            session_count: Some(session_count),
            session_number: Some(session_number),
            app_name: Some("Tilix".to_string()),
            ..Default::default()
        }
    }
}

fn resolve_token(token_name: &str, scope: TitleEditScope, ctx: &TokenContext) -> Option<String> {
    match token_name {
        // Terminal scope tokens (also available in Session and Window scopes)
        "title" => Some(if ctx.title.is_empty() {
            "Terminal".to_string()
        } else {
            ctx.title.clone()
        }),
        "iconTitle" => Some(ctx.icon_title.clone().unwrap_or_else(|| {
            if ctx.title.is_empty() {
                "Terminal".to_string()
            } else {
                ctx.title.clone()
            }
        })),
        "id" => Some(ctx.id.map(|i| i.to_string()).unwrap_or_else(|| "1".to_string())),
        "directory" => Some(
            ctx.directory
                .as_ref()
                .map(|d| d.to_string_lossy().to_string())
                .unwrap_or_default(),
        ),
        "hostname" => Some(
            ctx.hostname
                .clone()
                .unwrap_or_else(|| glib::host_name().to_string()),
        ),
        "username" => Some(
            ctx.username
                .clone()
                .unwrap_or_else(|| glib::user_name().to_string_lossy().to_string()),
        ),
        "columns" => Some(
            ctx.columns
                .map(|c| c.to_string())
                .unwrap_or_else(|| "80".to_string()),
        ),
        "rows" => Some(
            ctx.rows
                .map(|r| r.to_string())
                .unwrap_or_else(|| "24".to_string()),
        ),
        "process" => Some(ctx.process.clone().unwrap_or_default()),
        "status.readonly" => Some(if ctx.readonly {
            "[RO]".to_string()
        } else {
            String::new()
        }),
        "status.silence" => Some(if ctx.silence {
            "[Silence]".to_string()
        } else {
            String::new()
        }),
        "status.input-sync" => Some(if ctx.input_sync {
            "[Sync]".to_string()
        } else {
            String::new()
        }),
        "profile" => Some(
            ctx.profile_name
                .clone()
                .unwrap_or_else(|| "Default".to_string()),
        ),
        "appName" => Some(
            ctx.app_name
                .clone()
                .unwrap_or_else(|| "Tilix".to_string()),
        ),

        // Session scope tokens (available in Session and Window scopes)
        "activeTerminalTitle" if scope != TitleEditScope::Terminal => Some(
            ctx.active_terminal_title
                .clone()
                .unwrap_or_else(|| if ctx.title.is_empty() {
                    "Terminal".to_string()
                } else {
                    ctx.title.clone()
                }),
        ),
        "terminalCount" if scope != TitleEditScope::Terminal => Some(
            ctx.terminal_count
                .map(|c| c.to_string())
                .unwrap_or_else(|| "1".to_string()),
        ),
        "terminalNumber" if scope != TitleEditScope::Terminal => Some(
            ctx.terminal_number
                .map(|n| n.to_string())
                .unwrap_or_else(|| "1".to_string()),
        ),

        // Window scope tokens (only available in Window scope)
        "sessionName" if scope == TitleEditScope::Window => Some(
            ctx.session_name
                .clone()
                .unwrap_or_else(|| if ctx.title.is_empty() {
                    "Terminal".to_string()
                } else {
                    ctx.title.clone()
                }),
        ),
        "sessionNumber" if scope == TitleEditScope::Window => Some(
            ctx.session_number
                .map(|n| n.to_string())
                .unwrap_or_else(|| "1".to_string()),
        ),
        "sessionCount" if scope == TitleEditScope::Window => Some(
            ctx.session_count
                .map(|c| c.to_string())
                .unwrap_or_else(|| "1".to_string()),
        ),

        _ => None,
    }
}

pub fn expand_title_tokens_scoped(
    format_str: &str,
    scope: TitleEditScope,
    ctx: &TokenContext,
) -> String {
    let mut result = String::with_capacity(format_str.len());
    let mut remaining = format_str;

    while let Some(start_idx) = remaining.find("${") {
        result.push_str(&remaining[..start_idx]);
        let after_start = &remaining[start_idx + 2..];
        if let Some(end_idx) = after_start.find('}') {
            let token_name = &after_start[..end_idx];
            if !token_name.contains("${") {
                if let Some(val) = resolve_token(token_name, scope, ctx) {
                    result.push_str(&val);
                    remaining = &after_start[end_idx + 1..];
                    continue;
                }
            }
            result.push_str("${");
            remaining = &remaining[start_idx + 2..];
        } else {
            result.push_str(&remaining[start_idx..]);
            remaining = "";
            break;
        }
    }
    result.push_str(remaining);
    result
}

pub fn expand_title_tokens(format_str: &str, ctx: &TokenContext) -> String {
    expand_title_tokens_scoped(format_str, TitleEditScope::Window, ctx)
}
