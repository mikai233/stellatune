use super::state::CommandSuggestion;

#[derive(Debug, Clone)]
pub enum Command {
    Help,
    Quit,
    Refresh,
    Scan { force: bool },
    RootAdd { path: String },
    RootRemove { path: String },
    Search { query: String },
    Play { path: String },
    SeekTo { position_ms: i64 },
    SeekBy { delta_ms: i64 },
    Next,
    Prev,
    QueueAdd { path: String },
    QueueAddCurrent,
    QueueClear,
    QueueShow,
    PlaylistCreate { name: String },
    PlaylistRename { id: i64, name: String },
    PlaylistDelete { id: i64 },
    PlaylistAddTrack { playlist_id: i64, track_id: i64 },
    PlaylistRemoveTrack { playlist_id: i64, track_id: i64 },
    PluginInstall { artifact_path: String },
    PluginUninstall { plugin_id: String },
    PluginEnable { plugin_id: String },
    PluginDisable { plugin_id: String },
    PluginApply,
}

const COMMAND_HINTS: &[(&str, &str)] = &[
    ("help", "help"),
    ("quit", "quit"),
    ("refresh", "refresh"),
    ("search ", "search <query>"),
    ("scan", "scan"),
    ("scan!", "scan!"),
    ("root add ", "root add <path>"),
    ("root rm ", "root rm <path>"),
    ("play ", "play <path>"),
    ("seek +5000", "seek <ms|+ms|-ms|10s|+10s|-10s>"),
    ("next", "next"),
    ("prev", "prev"),
    ("queue add-current", "queue add-current"),
    ("queue add ", "queue add <path>"),
    ("queue show", "queue show"),
    ("queue clear", "queue clear"),
    ("playlist create ", "playlist create <name>"),
    ("playlist rename ", "playlist rename <id> <name>"),
    ("playlist delete ", "playlist delete <id>"),
    ("playlist add ", "playlist add <playlist_id> <track_id>"),
    ("playlist rm ", "playlist rm <playlist_id> <track_id>"),
    ("plugin install ", "plugin install <artifact_path>"),
    ("plugin uninstall ", "plugin uninstall <plugin_id>"),
    ("plugin enable ", "plugin enable <plugin_id>"),
    ("plugin disable ", "plugin disable <plugin_id>"),
    ("plugin apply", "plugin apply"),
];

pub fn parse_command(input: &str) -> Result<Command, String> {
    let raw = input.trim();
    if raw.is_empty() {
        return Err("empty command".to_string());
    }
    let mut parts = raw.split_whitespace();
    let head = parts
        .next()
        .ok_or_else(|| "empty command".to_string())?
        .to_ascii_lowercase();

    match head.as_str() {
        "help" | "h" | "?" => Ok(Command::Help),
        "quit" | "q" => Ok(Command::Quit),
        "refresh" | "r" => Ok(Command::Refresh),
        "scan" => Ok(Command::Scan { force: false }),
        "scan!" => Ok(Command::Scan { force: true }),
        "root" => parse_root_command(parts.collect()),
        "search" | "find" => {
            let query = parts.collect::<Vec<_>>().join(" ").trim().to_string();
            if query.is_empty() {
                return Err("usage: search <query>".to_string());
            }
            Ok(Command::Search { query })
        },
        "play" => {
            let path = parts.collect::<Vec<_>>().join(" ").trim().to_string();
            if path.is_empty() {
                return Err("usage: play <path>".to_string());
            }
            Ok(Command::Play { path })
        },
        "seek" => {
            let value = parts.collect::<Vec<_>>().join(" ");
            let value = value.trim();
            if value.is_empty() {
                return Err("usage: seek <ms|+ms|-ms|10s|+10s|-10s>".to_string());
            }
            parse_seek_command(value)
        },
        "next" | "nxt" => Ok(Command::Next),
        "prev" | "previous" => Ok(Command::Prev),
        "queue" => parse_queue_command(parts.collect()),
        "playlist" | "pl" => parse_playlist_command(parts.collect()),
        "plugin" => parse_plugin_command(parts.collect(), raw),
        _ => Err(format!("unknown command: {head}")),
    }
}

pub fn build_command_suggestions(
    prefix: char,
    input: &str,
    last_search_query: &str,
) -> Vec<CommandSuggestion> {
    match prefix {
        ':' => build_colon_suggestions(input),
        '/' | '?' => build_search_suggestions(input, last_search_query),
        _ => Vec::new(),
    }
}

fn build_colon_suggestions(input: &str) -> Vec<CommandSuggestion> {
    let normalized = input.trim_start().to_ascii_lowercase();
    let mut out = COMMAND_HINTS
        .iter()
        .filter(|(insert, display)| {
            if normalized.is_empty() {
                return true;
            }
            let insert_norm = insert.to_ascii_lowercase();
            let display_norm = display.to_ascii_lowercase();
            insert_norm.starts_with(&normalized)
                || display_norm.starts_with(&normalized)
                || display_norm.contains(&normalized)
        })
        .map(|(insert, display)| CommandSuggestion {
            insert: (*insert).to_string(),
            display: (*display).to_string(),
        })
        .collect::<Vec<_>>();
    out.truncate(8);
    out
}

fn build_search_suggestions(input: &str, last_search_query: &str) -> Vec<CommandSuggestion> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        let mut out = Vec::new();
        if !last_search_query.trim().is_empty() {
            out.push(CommandSuggestion {
                insert: last_search_query.trim().to_string(),
                display: format!("repeat last search: {}", last_search_query.trim()),
            });
        }
        out.push(CommandSuggestion {
            insert: String::new(),
            display: "type keyword and press Enter".to_string(),
        });
        return out;
    }

    vec![CommandSuggestion {
        insert: trimmed.to_string(),
        display: format!("search `{trimmed}`"),
    }]
}

fn parse_root_command(args: Vec<&str>) -> Result<Command, String> {
    if args.is_empty() {
        return Err("usage: root <add|rm> <path>".to_string());
    }
    let op = args[0].to_ascii_lowercase();
    let path = args[1..].join(" ").trim().to_string();
    if path.is_empty() {
        return Err("usage: root <add|rm> <path>".to_string());
    }
    match op.as_str() {
        "add" => Ok(Command::RootAdd { path }),
        "rm" | "remove" => Ok(Command::RootRemove { path }),
        _ => Err("usage: root <add|rm> <path>".to_string()),
    }
}

fn parse_seek_command(raw: &str) -> Result<Command, String> {
    let parsed = parse_ms_value(raw)?;
    if raw.starts_with('+') || raw.starts_with('-') {
        Ok(Command::SeekBy { delta_ms: parsed })
    } else {
        Ok(Command::SeekTo {
            position_ms: parsed.max(0),
        })
    }
}

fn parse_queue_command(args: Vec<&str>) -> Result<Command, String> {
    if args.is_empty() {
        return Err("usage: queue <add|add-current|show|clear> ...".to_string());
    }
    let op = args[0].to_ascii_lowercase();
    match op.as_str() {
        "add-current" | "ac" => Ok(Command::QueueAddCurrent),
        "show" | "ls" => Ok(Command::QueueShow),
        "clear" | "clr" => Ok(Command::QueueClear),
        "add" => {
            let path = args[1..].join(" ").trim().to_string();
            if path.is_empty() {
                return Err("usage: queue add <path>".to_string());
            }
            Ok(Command::QueueAdd { path })
        },
        _ => Err("usage: queue <add|add-current|show|clear> ...".to_string()),
    }
}

fn parse_playlist_command(args: Vec<&str>) -> Result<Command, String> {
    if args.is_empty() {
        return Err("usage: playlist <create|rename|delete|add|rm> ...".to_string());
    }
    let op = args[0].to_ascii_lowercase();
    match op.as_str() {
        "create" => {
            let name = args[1..].join(" ").trim().to_string();
            if name.is_empty() {
                return Err("usage: playlist create <name>".to_string());
            }
            Ok(Command::PlaylistCreate { name })
        },
        "rename" => {
            if args.len() < 3 {
                return Err("usage: playlist rename <id> <name>".to_string());
            }
            let id = parse_i64(args[1], "playlist id")?;
            let name = args[2..].join(" ").trim().to_string();
            if name.is_empty() {
                return Err("usage: playlist rename <id> <name>".to_string());
            }
            Ok(Command::PlaylistRename { id, name })
        },
        "delete" | "del" => {
            if args.len() != 2 {
                return Err("usage: playlist delete <id>".to_string());
            }
            let id = parse_i64(args[1], "playlist id")?;
            Ok(Command::PlaylistDelete { id })
        },
        "add" => {
            if args.len() != 3 {
                return Err("usage: playlist add <playlist_id> <track_id>".to_string());
            }
            Ok(Command::PlaylistAddTrack {
                playlist_id: parse_i64(args[1], "playlist id")?,
                track_id: parse_i64(args[2], "track id")?,
            })
        },
        "rm" | "remove" => {
            if args.len() != 3 {
                return Err("usage: playlist rm <playlist_id> <track_id>".to_string());
            }
            Ok(Command::PlaylistRemoveTrack {
                playlist_id: parse_i64(args[1], "playlist id")?,
                track_id: parse_i64(args[2], "track id")?,
            })
        },
        _ => Err("usage: playlist <create|rename|delete|add|rm> ...".to_string()),
    }
}

fn parse_plugin_command(args: Vec<&str>, raw: &str) -> Result<Command, String> {
    if args.is_empty() {
        return Err("usage: plugin <install|uninstall|enable|disable|apply> ...".to_string());
    }
    let op = args[0].to_ascii_lowercase();
    match op.as_str() {
        "install" => {
            let artifact_path = take_tail(raw, "plugin install").trim().to_string();
            if artifact_path.is_empty() {
                return Err("usage: plugin install <artifact_path>".to_string());
            }
            Ok(Command::PluginInstall { artifact_path })
        },
        "uninstall" | "rm" => {
            if args.len() != 2 {
                return Err("usage: plugin uninstall <plugin_id>".to_string());
            }
            Ok(Command::PluginUninstall {
                plugin_id: args[1].to_string(),
            })
        },
        "enable" => {
            if args.len() != 2 {
                return Err("usage: plugin enable <plugin_id>".to_string());
            }
            Ok(Command::PluginEnable {
                plugin_id: args[1].to_string(),
            })
        },
        "disable" => {
            if args.len() != 2 {
                return Err("usage: plugin disable <plugin_id>".to_string());
            }
            Ok(Command::PluginDisable {
                plugin_id: args[1].to_string(),
            })
        },
        "apply" => Ok(Command::PluginApply),
        _ => Err("usage: plugin <install|uninstall|enable|disable|apply> ...".to_string()),
    }
}

fn parse_i64(raw: &str, label: &str) -> Result<i64, String> {
    raw.parse::<i64>()
        .map_err(|_| format!("invalid {label}: {raw}"))
}

fn parse_ms_value(raw: &str) -> Result<i64, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("empty time value".to_string());
    }

    let (numeric, multiplier) = if let Some(value) = trimmed.strip_suffix('s') {
        (value, 1000)
    } else if let Some(value) = trimmed.strip_suffix("ms") {
        (value, 1)
    } else {
        (trimmed, 1)
    };

    let parsed = numeric
        .parse::<i64>()
        .map_err(|_| format!("invalid time value: {raw}"))?;
    Ok(parsed.saturating_mul(multiplier))
}

fn take_tail<'a>(raw: &'a str, prefix: &str) -> &'a str {
    raw.strip_prefix(prefix).unwrap_or("")
}
