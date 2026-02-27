//! dmcp - MCP Manager CLI

use clap::{Parser, Subcommand};
use dmcp::config;
use dmcp::elevation::{is_elevated, is_system_scope, re_exec_with_pkexec};
use dmcp::{add_source, call, connect, discovery, fetch_server_from_registry, get_server, install, list_registry_servers, list_registry_servers_from_url, list_servers, list_sources, remove_source, run, run_setup, scope_from_registry_server, set_config_value, uninstall, Paths};

#[derive(Parser)]
#[command(name = "dmcp")]
#[command(about = "MCP Manager - discover, manage, and invoke MCP servers")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Enable debug output
    #[arg(short, long, global = true)]
    debug: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// List installed MCP servers (default: both user and system)
    List {
        /// Include user-scope servers only
        #[arg(long)]
        user: bool,

        /// Include system-scope servers only
        #[arg(long)]
        system: bool,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Show detailed info for a server
    Info {
        /// Server ID (e.g. com.example.calculator)
        id: String,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Get or set server configuration
    Config {
        /// Server ID
        id: String,

        #[command(subcommand)]
        action: ConfigAction,
    },

    /// Manage registry sources
    Sources {
        #[command(subcommand)]
        action: SourcesAction,
    },

    /// Install an MCP server from registry (by ID) or from manifest/endpoint URL
    Install {
        /// Server ID (from registry) or URL (manifest.json or SSE/WebSocket endpoint)
        id_or_url: String,

        /// Install to system scope (requires elevation)
        #[arg(long)]
        system: bool,

        /// Skip running the setup script (if defined)
        #[arg(long)]
        no_setup: bool,
    },

    /// Uninstall an MCP server
    Uninstall {
        /// Server ID to uninstall
        id: String,
    },

    /// Run an MCP server (stdio: spawn and relay; SSE/WebSocket: print connection URL)
    Run {
        /// Server ID to run
        id: String,

        /// Enable verbose output (reserved for future debug mode)
        #[arg(long)]
        verbose: bool,
    },

    /// Connect to a remote server. Fetches manifest from URL if valid JSON; otherwise treats URL as raw endpoint.
    Connect {
        /// URL to manifest.json (fetched and used) or raw SSE/WebSocket endpoint (fallback)
        url: String,

        /// Override server ID
        #[arg(long)]
        id: Option<String>,

        /// Override display name
        #[arg(long)]
        name: Option<String>,

        /// Override short description
        #[arg(long)]
        summary: Option<String>,

        /// Override version string
        #[arg(long)]
        version: Option<String>,

        /// Config key=value (repeatable, overrides manifest config)
        #[arg(short, long, value_parser = parse_config)]
        config: Vec<(String, String)>,

        /// Install to system scope (requires elevation)
        #[arg(long)]
        system: bool,

        /// Skip running the setup script (if defined)
        #[arg(long)]
        no_setup: bool,
    },

    /// Run the setup script for an installed server (e.g. after config changes)
    Setup {
        /// Server ID
        id: String,
    },

    /// List tools available on an MCP server
    Tools {
        /// Server ID
        id: String,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Call a tool on an MCP server
    Call {
        /// Server ID
        id: String,

        /// Tool name to call
        tool: String,

        /// Tool arguments as JSON (e.g. '{"key":"value"}')
        #[arg(long)]
        args: Option<String>,
    },

    /// Run dmcp as an MCP server (for LLM integration)
    Serve,

    /// Browse servers available in registry sources (or a specific registry URL)
    Browse {
        /// Registry URL to browse (omit to use configured sources)
        url: Option<String>,

        /// Show user-scope sources only (ignored when URL is given)
        #[arg(long)]
        user: bool,

        /// Show system-scope sources only (ignored when URL is given)
        #[arg(long)]
        system: bool,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Show resolved paths (for debugging)
    Paths,
}

#[derive(Subcommand)]
enum ConfigAction {
    /// Get config value(s)
    Get {
        /// Specific key (omit for all)
        key: Option<String>,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Set a config value
    Set {
        /// Config key
        key: String,

        /// Config value
        value: String,
    },
}

#[derive(Subcommand)]
enum SourcesAction {
    /// List registry source URLs
    List {
        /// Show user-scope sources only
        #[arg(long)]
        user: bool,

        /// Show system-scope sources only
        #[arg(long)]
        system: bool,
    },

    /// Add a registry source URL
    Add {
        /// URL of the registry JSON file
        url: String,

        /// Add to user scope (default)
        #[arg(long)]
        user: bool,

        /// Add to system scope (requires elevation)
        #[arg(long)]
        system: bool,
    },

    /// Remove a registry source URL
    Remove {
        /// URL to remove
        url: String,

        /// Remove from user scope
        #[arg(long)]
        user: bool,

        /// Remove from system scope (requires elevation)
        #[arg(long)]
        system: bool,
    },
}

fn main() {
    dotenvy::dotenv().ok();
    let cli = Cli::parse();
    let paths = Paths::resolve();
    let debug = cli.debug;

    match cli.command {
        Commands::Paths => {
            println!("User install dir:  {}", paths.user_install_dir().display());
            println!("System install dir: {}", paths.system_install_dir().display());
            let user_index = paths.user_install_dir().join("index.json");
            let system_index = paths.system_install_dir().join("index.json");
            println!("User index exists:  {}", user_index.exists());
            println!("System index exists: {}", system_index.exists());
        }
        Commands::List { user, system, json } => {
            let include_user = user || (!user && !system);
            let include_system = system || (!user && !system);
            let servers = list_servers(&paths, include_user, include_system, debug);

            if json {
                let output = serde_json::to_string_pretty(&servers).unwrap();
                println!("{output}");
            } else {
                if servers.is_empty() {
                    println!("No MCP servers installed.");
                    return;
                }
                print_list_table(&servers);
            }
        }
        Commands::Info { id, json } => {
            match get_server(&paths, &id) {
                Some((manifest, scope)) => {
                    let scope_str = match scope {
                        dmcp::discovery::Scope::User => "user",
                        dmcp::discovery::Scope::System => "system",
                    };
                    if json {
                        let output = serde_json::to_string_pretty(&manifest).unwrap();
                        println!("{output}");
                    } else {
                        print_info_output(&manifest, scope_str);
                    }
                }
                None => {
                    eprintln!("Server not found: {}", id);
                    std::process::exit(1);
                }
            }
        }
        Commands::Config { id, action } => match action {
            ConfigAction::Set { key, value } => {
                match set_config_value(&paths, &id, &key, &value) {
                    Ok(()) => println!("Set {} = {}", key, value),
                    Err(config::SetConfigError::WriteFailed(_, manifest_path)) if !is_elevated() => {
                        if is_system_scope(&manifest_path, paths.system_install_dir()) {
                            re_exec_with_pkexec();
                        } else {
                            eprintln!("Error: Failed to write manifest (permission denied)");
                            std::process::exit(1);
                        }
                    }
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
                }
            }
            ConfigAction::Get { key, json } => {
                match get_server(&paths, &id) {
                    Some((manifest, _)) => {
                        if let Some(k) = key {
                            match manifest.config.get(&k) {
                                Some(v) => {
                                    if json {
                                        println!("{}", serde_json::to_string_pretty(v).unwrap());
                                    } else {
                                        let val: String = v.as_str().map(String::from).unwrap_or_else(|| v.to_string());
                                        println!("{}", val);
                                    }
                                }
                                None => {
                                    eprintln!("Config key not found: {}", k);
                                    std::process::exit(1);
                                }
                            }
                        } else {
                            if json {
                                let output = serde_json::to_string_pretty(&manifest.config).unwrap();
                                println!("{output}");
                            } else {
                                if manifest.config.is_empty() {
                                    println!("No config set.");
                                } else {
                                    for (k, v) in &manifest.config {
                                        let val: String = v.as_str().map(String::from).unwrap_or_else(|| v.to_string());
                                        println!("{} = {}", k, val);
                                    }
                                }
                            }
                        }
                    }
                    None => {
                        eprintln!("Server not found: {}", id);
                        std::process::exit(1);
                    }
                }
            }
        },
        Commands::Sources { action } => match action {
            SourcesAction::List { user, system } => {
                let include_user = user || (!user && !system);
                let include_system = system || (!user && !system);
                let sources = list_sources(&paths, include_user, include_system);
                if sources.is_empty() {
                    println!("No registry sources configured.");
                    println!("Add URLs to ~/.config/mcp/sources.list or /etc/mcp/sources.list");
                    return;
                }
                println!("{:<8} {}", "SCOPE", "URL");
                println!("{}", "-".repeat(80));
                for (url, scope) in sources {
                    let scope_str = match scope {
                        dmcp::SourceScope::User => "user",
                        dmcp::SourceScope::System => "system",
                    };
                    println!("{:<8} {}", scope_str, url);
                }
            }
            SourcesAction::Add { url, system, .. } => {
                let scope = if system {
                    dmcp::SourceScope::System
                } else {
                    dmcp::SourceScope::User
                };
                // System scope needs root for create_dir + write; re-exec upfront
                if scope == dmcp::SourceScope::System && !is_elevated() {
                    re_exec_with_pkexec();
                }
                match add_source(&paths, &url, scope) {
                    Ok(()) => println!("Added {}", url),
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
                }
            }
            SourcesAction::Remove { url, system, .. } => {
                let scope = if system {
                    dmcp::SourceScope::System
                } else {
                    dmcp::SourceScope::User
                };
                // System scope needs root; re-exec upfront
                if scope == dmcp::SourceScope::System && !is_elevated() {
                    re_exec_with_pkexec();
                }
                match remove_source(&paths, &url, scope) {
                    Ok(()) => println!("Removed {}", url),
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
                }
            }
        },
        Commands::Install { id_or_url, system, no_setup } => {
            let run_setup = !no_setup;
            let is_url = id_or_url.starts_with("http://") || id_or_url.starts_with("https://");
            if is_url {
                    // URL: use connect flow
                    let scope = if system {
                        dmcp::discovery::Scope::System
                    } else {
                        dmcp::discovery::Scope::User
                    };
                    if scope == dmcp::discovery::Scope::System && !is_elevated() {
                        re_exec_with_pkexec();
                    }
                    match connect(
                        &paths,
                        &id_or_url,
                        None,
                        None,
                        None,
                        None,
                        &[],
                        scope,
                        run_setup,
                    ) {
                        Ok(id) => println!("Installed {}", id),
                        Err(e) => {
                            eprintln!("Error: {}", e);
                            std::process::exit(1);
                        }
                    }
            } else {
                    // ID: use registry install flow
                    let id = id_or_url;
                    let server = match fetch_server_from_registry(&paths, &id) {
                        Ok(s) => s,
                        Err(e) => {
                            eprintln!("Error: {}", e);
                            std::process::exit(1);
                        }
                    };
                    let scope = if system {
                        dmcp::discovery::Scope::System
                    } else {
                        scope_from_registry_server(&server)
                    };
                    if scope == dmcp::discovery::Scope::System && !is_elevated() {
                        re_exec_with_pkexec();
                    }
                    match install(&paths, &id, scope, Some(server), run_setup) {
                        Ok(()) => println!("Installed {}", id),
                        Err(e) => {
                            eprintln!("Error: {}", e);
                            std::process::exit(1);
                        }
                    }
            }
        }
        Commands::Run { id, verbose } => {
            match run(&paths, &id, verbose) {
                Ok(()) => {}
                Err(dmcp::run::RunError::ProcessExited(code)) => std::process::exit(code),
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Commands::Uninstall { id } => {
            if let Some((_, _, scope)) = discovery::get_uninstall_info(&paths, &id) {
                if scope == dmcp::discovery::Scope::System && !is_elevated() {
                    re_exec_with_pkexec();
                }
            }
            match uninstall(&paths, &id) {
                Ok(()) => println!("Uninstalled {}", id),
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Commands::Connect {
            url,
            id,
            name,
            summary,
            version,
            config,
            system,
            no_setup,
        } => {
            let scope = if system {
                dmcp::discovery::Scope::System
            } else {
                dmcp::discovery::Scope::User
            };
            if scope == dmcp::discovery::Scope::System && !is_elevated() {
                re_exec_with_pkexec();
            }
            let config_ref: Vec<(String, String)> = config.iter().cloned().collect();
            let run_setup = !no_setup;
            match connect(
                &paths,
                &url,
                id.as_deref(),
                name.as_deref(),
                summary.as_deref(),
                version.as_deref(),
                &config_ref,
                scope,
                run_setup,
            ) {
                Ok(id) => println!("Connected {}", id),
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Commands::Setup { id } => {
            match get_server(&paths, &id) {
                Some((manifest, _)) => {
                    let setup_script = manifest
                        .setup_script
                        .as_deref()
                        .filter(|s| !s.is_empty());
                    match setup_script {
                        Some(script) => {
                            let install_dir = manifest
                                .install_dir
                                .as_deref()
                                .map(std::path::Path::new)
                                .filter(|p| p.is_absolute())
                                .map(|p| p.to_path_buf())
                                .or_else(|| {
                                    discovery::get_manifest_path(&paths, &id)
                                        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
                                });
                            match install_dir {
                                Some(dir) => {
                                    if let Err(e) = run_setup(script, &dir, &manifest.config) {
                                        eprintln!("Error: {}", e);
                                        std::process::exit(1);
                                    }
                                    // Update manifest with setup run timestamp
                                    if let Some(manifest_path) = discovery::get_manifest_path(&paths, &id) {
                                        if let Ok(content) = std::fs::read_to_string(&manifest_path) {
                                            if let Ok(mut m) = serde_json::from_str::<serde_json::Value>(&content) {
                                                m["setupScriptPath"] = serde_json::json!(dir.join(script).to_string_lossy());
                                                m["setupScriptRunAt"] = serde_json::Value::String(dmcp::install::rfc3339_now());
                                                m["setupScriptVersion"] = manifest
                                                    .setup_script_version
                                                    .as_ref()
                                                    .map(|s| serde_json::Value::String(s.clone()))
                                                    .unwrap_or(serde_json::json!("1.0.0"));
                                                let _ = std::fs::write(&manifest_path, serde_json::to_string_pretty(&m).unwrap_or_default());
                                            }
                                        }
                                    }
                                    println!("Setup complete for {}", id);
                                }
                                None => {
                                    eprintln!("Error: Could not determine install directory");
                                    std::process::exit(1);
                                }
                            }
                        }
                        None => {
                            eprintln!("Server {} has no setup script defined", id);
                            std::process::exit(1);
                        }
                    }
                }
                None => {
                    eprintln!("Server not found: {}", id);
                    std::process::exit(1);
                }
            }
        }
        Commands::Tools { id, json } => {
            let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
            match rt.block_on(call::list_tools(&paths, &id)) {
                Ok(tools) => {
                    if json {
                        println!("{}", serde_json::to_string_pretty(&tools).unwrap());
                    } else {
                        for t in &tools {
                            println!("{} - {}", t.name, t.description.as_deref().unwrap_or(""));
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Commands::Call { id, tool, args } => {
            let args_val = args.as_deref().and_then(|s| serde_json::from_str(s).ok());
            let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
            match rt.block_on(call::call_tool(&paths, &id, &tool, args_val)) {
                Ok(result) => println!("{}", call::format_call_result(&result)),
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Commands::Serve => {
            if let Err(e) = dmcp::serve::run(&paths) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Commands::Browse { url, user, system, json } => {
            let (servers, errors): (Vec<_>, Vec<_>) = if let Some(ref u) = url {
                match list_registry_servers_from_url(u) {
                    Ok(s) => (s, vec![]),
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
                }
            } else {
                let include_user = user || (!user && !system);
                let include_system = system || (!user && !system);
                list_registry_servers(&paths, include_user, include_system)
            };

            for e in &errors {
                eprintln!("Warning: {}", e);
            }

            if json {
                let output = serde_json::to_string_pretty(&servers).unwrap();
                println!("{output}");
            } else {
                if servers.is_empty() && errors.is_empty() && url.is_none() {
                    println!("No registry sources configured. Add one with: dmcp sources add <url>");
                    return;
                }
                if servers.is_empty() {
                    println!("No servers found in registries.");
                    return;
                }
                print_browse_table(&servers);
            }
        }
    }
}

fn parse_config(s: &str) -> Result<(String, String), String> {
    let s = s.trim();
    if let Some(eq) = s.find('=') {
        let k = s[..eq].trim().to_string();
        let v = s[eq + 1..].trim().to_string();
        if k.is_empty() {
            Err("config key cannot be empty".to_string())
        } else {
            Ok((k, v))
        }
    } else {
        Err("config must be key=value".to_string())
    }
}

fn format_tools(tools: &[serde_json::Value]) -> String {
    tools
        .iter()
        .map(|t| {
            if let Some(obj) = t.as_object() {
                obj.get("name")
                    .and_then(|n| n.as_str())
                    .unwrap_or("?")
            } else {
                t.as_str().unwrap_or("?")
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn format_transports(transports: &[dmcp::models::Transport]) -> String {
    transports
        .iter()
        .map(|t| match t {
            dmcp::models::Transport::Stdio { command, args, .. } => {
                let args_str = args
                    .as_ref()
                    .map(|a| a.join(" "))
                    .unwrap_or_default();
                format!("stdio ({command} {args_str})")
            }
            dmcp::models::Transport::Sse { url, .. } => format!("sse ({url})"),
            dmcp::models::Transport::WebSocket { ws_url, .. } => format!("websocket ({ws_url})"),
        })
        .collect::<Vec<_>>()
        .join("; ")
}

fn print_info_output(manifest: &dmcp::Manifest, scope_str: &str) {
    const INDENT: &str = "        ";

    println!("{}", manifest.id.as_deref().unwrap_or("?"));
    println!("{}Name:        {}", INDENT, manifest.name.as_deref().unwrap_or("?"));
    println!("{}Version:     {}", INDENT, manifest.version.as_deref().unwrap_or("?"));
    println!("{}Scope:       {}", INDENT, scope_str);
    if let Some(s) = manifest.summary.as_deref().filter(|x| !x.is_empty()) {
        println!("{}Summary:     {}", INDENT, s);
    }
    if let Some(d) = manifest.description.as_deref().filter(|x| !x.is_empty()) {
        println!("{}Description:", INDENT);
        for line in d.lines() {
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                println!("{}{}{}", INDENT, INDENT, trimmed);
            }
        }
    }
    if let Some(a) = manifest.author.as_deref().filter(|x| !x.is_empty()) {
        println!("{}Author:      {}", INDENT, a);
    }
    if let Some(h) = manifest.homepage.as_deref().filter(|x| !x.is_empty()) {
        println!("{}Homepage:    {}", INDENT, h);
    }
    if !manifest.categories.is_empty() {
        println!("{}Categories:  {}", INDENT, manifest.categories.join(", "));
    }
    if !manifest.capabilities.is_empty() {
        println!("{}Capabilities: {}", INDENT, manifest.capabilities.join(", "));
    }
    if !manifest.tools.is_empty() {
        println!("{}Tools:       {}", INDENT, format_tools(&manifest.tools));
    }
    if let Some(ref t) = manifest.transports {
        println!("{}Transports:  {}", INDENT, format_transports(t));
    }
    if let Some(ref dir) = manifest.install_dir {
        println!("{}Install:     {}", INDENT, dir);
    }
    if !manifest.config.is_empty() {
        for (k, v) in &manifest.config {
            let val: String = v.as_str().map(String::from).unwrap_or_else(|| v.to_string());
            println!("{}Config.{}:   {}", INDENT, k, val);
        }
    }
    if let Some(ref s) = manifest.setup_script {
        println!("{}Setup:      {}", INDENT, s);
    }
}

fn print_list_table(servers: &[dmcp::ServerInfo]) {
    const INDENT: &str = "        ";

    for s in servers {
        let scope = match s.scope {
            dmcp::discovery::Scope::User => "user",
            dmcp::discovery::Scope::System => "system",
        };
        println!("{}", s.id);
        println!("{}Name:      {}", INDENT, s.name);
        println!("{}Version:   {}", INDENT, s.version);
        println!("{}Transport: {}", INDENT, s.transport_type);
        println!("{}Scope:     {}", INDENT, scope);
        println!("{}Manifest: {}", INDENT, s.manifest_path);
        println!();
    }
}

fn print_browse_table(servers: &[dmcp::RegistryServer]) {
    const INDENT: &str = "        ";

    for s in servers {
        println!("{}", s.id);
        println!("{}Name:      {}", INDENT, s.name);
        println!("{}Version:   {}", INDENT, s.version);
        println!("{}Transport: {}", INDENT, s.transport);
        if !s.summary.is_empty() {
            println!("{}Summary:   {}", INDENT, s.summary.lines().next().unwrap_or("").trim());
        }
        println!("{}Source:    {}", INDENT, s.source);
        println!();
    }
}
