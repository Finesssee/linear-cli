mod api;
mod cache;
mod commands;
mod config;
mod dates;
mod error;
mod output;
mod pagination;
mod priority;
mod json_path;
mod vcs;
mod retry;
mod text;

use anyhow::Result;
use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use clap_complete::{generate, Shell};
use commands::{
    auth, bulk, comments, cycles, doctor, documents, export, favorites, git, history,
    initiatives, interactive, issues, labels, metrics, notifications, projects, relations, roadmaps, search,
    statuses, sync, teams, templates, time, triage, uploads, users, watch,
};
use error::CliError;
use output::print_json;
use output::{parse_filters, JsonOutputOptions, OutputOptions, SortOrder};
use pagination::PaginationOptions;
use std::sync::OnceLock;

/// Output format for command results
#[derive(Debug, Clone, Copy, Default, ValueEnum, PartialEq)]
pub enum OutputFormat {
    /// Display results as formatted tables (default)
    #[default]
    Table,
    /// Display results as raw JSON
    Json,
    /// Display results as NDJSON (one JSON object per line)
    Ndjson,
}

#[derive(Debug, Clone, Copy, Default, ValueEnum, PartialEq)]
pub enum ColorChoice {
    #[default]
    Auto,
    Always,
    Never,
}

/// Global options for agentic/scripting use
#[derive(Debug, Clone, Copy, Default)]
pub struct AgentOptions {
    /// Suppress decorative output (headers, separators, tips)
    pub quiet: bool,
    /// Only output IDs of created/updated resources
    pub id_only: bool,
    /// Preview without making changes (where supported)
    pub dry_run: bool,
}

#[derive(Parser)]
#[command(name = "linear")]
#[command(
    about = "A powerful CLI for Linear.app - manage issues, projects, and more from your terminal"
)]
#[command(version)]
#[command(after_help = r#"QUICK START:
    1. Get your API key from https://linear.app/settings/api
    2. Configure the CLI:
       linear config set-key YOUR_API_KEY
    3. List your issues:
       linear issues list
    4. Create an issue:
       linear issues create "Fix bug" --team ENG --priority 2

COMMON FLAGS:
    --output table|json|ndjson    Output format (default: table)
    --color auto|always|never     Color output control
    --no-color                    Disable color output
    --width N                     Max table column width
    --no-truncate                 Disable table truncation
    --quiet                       Reduce decorative output
    --format TEMPLATE             Template output (e.g. '{{identifier}} {{title}}')
    --filter field=value          Filter results (repeatable)
    --limit N                     Limit list/search results
    --page-size N                 Page size for list/search
    --after CURSOR                Pagination cursor (after)
    --before CURSOR               Pagination cursor (before)
    --all                         Fetch all pages
    --profile NAME                Use named profile
    --schema                      Print JSON schema version and exit
    --cache-ttl N                 Cache TTL in seconds
    --no-cache                    Disable cache usage

For more info on a command, run: linear <command> --help"#)]
struct Cli {
    /// Output format (table or json)
    #[arg(
        short,
        long,
        global = true,
        env = "LINEAR_CLI_OUTPUT",
        default_value = "table"
    )]
    output: OutputFormat,

    /// Suppress decorative output (headers, separators, tips) - for scripting
    #[arg(short, long, global = true)]
    quiet: bool,

    /// Only output IDs of created/updated resources - for chaining commands
    #[arg(long, global = true)]
    id_only: bool,

    /// Color output: auto, always, or never
    #[arg(
        long,
        global = true,
        value_enum,
        default_value = "auto",
        conflicts_with = "no_color"
    )]
    color: ColorChoice,

    /// Disable color output
    #[arg(long, global = true)]
    no_color: bool,

    /// Max column width for table output (default: 50)
    #[arg(long, global = true)]
    width: Option<usize>,

    /// Disable truncation for table output
    #[arg(long, global = true)]
    no_truncate: bool,

    /// Emit compact JSON without pretty formatting
    #[arg(long, global = true)]
    compact: bool,

    /// Limit JSON output to specific fields (comma-separated, supports dot paths)
    #[arg(long, global = true, value_delimiter = ',')]
    fields: Vec<String>,

    /// Sort JSON array output by a field (default: identifier/id when available)
    #[arg(long, global = true)]
    sort: Option<String>,

    /// Sort order for JSON array output
    #[arg(long, global = true, value_enum, default_value = "asc")]
    order: SortOrder,

    /// Override API key for this invocation
    #[arg(long, global = true, env = "LINEAR_API_KEY")]
    api_key: Option<String>,

    /// Override workspace profile for this invocation
    #[arg(long, global = true, env = "LINEAR_CLI_PROFILE")]
    profile: Option<String>,

    /// Output using a template (e.g. '{{identifier}} {{title}}')
    #[arg(long, global = true)]
    format: Option<String>,

    /// Filter results (field=value, field!=value, field~=value)
    #[arg(long, global = true)]
    filter: Vec<String>,

    /// Exit with non-zero status when a list is empty
    #[arg(long, global = true)]
    fail_on_empty: bool,

    /// Max results to return for list/search commands
    #[arg(long, global = true)]
    limit: Option<usize>,

    /// Pagination cursor to start after
    #[arg(long, global = true)]
    after: Option<String>,

    /// Pagination cursor to end before
    #[arg(long, global = true)]
    before: Option<String>,

    /// Page size per request for list/search commands
    #[arg(long, global = true)]
    page_size: Option<usize>,

    /// Fetch all pages for list/search commands
    #[arg(long, global = true)]
    all: bool,

    /// Override cache TTL in seconds
    #[arg(long, global = true, env = "LINEAR_CLI_CACHE_TTL")]
    cache_ttl: Option<u64>,

    /// Disable cache usage for this invocation
    #[arg(long, global = true, env = "LINEAR_CLI_NO_CACHE")]
    no_cache: bool,

    /// Preview without making changes where supported
    #[arg(long, global = true)]
    dry_run: bool,

    /// Number of retries for failed API requests (with exponential backoff)
    #[arg(long, global = true, default_value = "0")]
    retry: u32,

    /// Print JSON schema version info and exit
    #[arg(long, global = true)]
    schema: bool,

    /// Show common tasks and examples
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct DisplayOptions {
    pub width: Option<usize>,
    pub no_truncate: bool,
}

impl DisplayOptions {
    pub fn max_width(&self, default: usize) -> Option<usize> {
        if self.no_truncate {
            None
        } else {
            Some(self.width.unwrap_or(default))
        }
    }
}

static DISPLAY_OPTIONS: OnceLock<DisplayOptions> = OnceLock::new();

fn set_cli_state(display: DisplayOptions) {
    let _ = DISPLAY_OPTIONS.set(display);
}

pub fn display_options() -> DisplayOptions {
    DISPLAY_OPTIONS.get().copied().unwrap_or_default()
}

#[derive(Subcommand)]
enum Commands {
    /// Show common tasks and examples
    #[command(alias = "tasks")]
    Common,
    /// Show agent-focused capabilities and examples
    Agent,
    /// Authenticate and manage API keys
    #[command(after_help = r#"EXAMPLES:
    linear auth login                        # Store API key
    linear auth status                       # Show auth status
    linear auth logout                       # Remove current profile"#)]
    Auth {
        #[command(subcommand)]
        action: auth::AuthCommands,
    },
    /// Diagnose configuration and connectivity
    #[command(after_help = r#"EXAMPLES:
    linear doctor                            # Check config and auth
    linear doctor --check-api                # Validate API access"#)]
    Doctor {
        /// Validate API connectivity and auth
        #[arg(long)]
        check_api: bool,
    },
    /// Manage projects - list, create, update, delete projects
    #[command(alias = "p")]
    #[command(after_help = r#"EXAMPLES:
    linear projects list                    # List all projects
    linear p list --archived                # Include archived projects
    linear p get PROJECT_ID                 # View project details
    linear p create "Q1 Roadmap" -t ENG     # Create a project"#)]
    Projects {
        #[command(subcommand)]
        action: projects::ProjectCommands,
    },
    /// Manage issues - list, create, update, assign, track issues
    #[command(alias = "i")]
    #[command(after_help = r#"EXAMPLES:
    linear issues list                      # List all issues
    linear i list -t ENG -s "In Progress"   # Filter by team and status
    linear i get LIN-123                    # View issue details
    linear i create "Bug fix" -t ENG -p 2   # Create high priority issue
    linear i update LIN-123 -s Done         # Update issue status"#)]
    Issues {
        #[command(subcommand)]
        action: issues::IssueCommands,
    },
    /// Manage labels - create and organize project/issue labels
    #[command(alias = "l")]
    #[command(after_help = r##"EXAMPLES:
    linear labels list                      # List project labels
    linear l list --type issue              # List issue labels
    linear l create "Feature" --color "#10B981"
    linear l delete LABEL_ID --force"##)]
    Labels {
        #[command(subcommand)]
        action: labels::LabelCommands,
    },
    /// Manage teams - list and view team details
    #[command(alias = "t")]
    #[command(after_help = r#"EXAMPLES:
    linear teams list                       # List all teams
    linear t get ENG                        # View team details"#)]
    Teams {
        #[command(subcommand)]
        action: teams::TeamCommands,
    },
    /// Manage users - list workspace users and view profiles
    #[command(alias = "u")]
    #[command(after_help = r#"EXAMPLES:
    linear users list                       # List all users
    linear u list --team ENG                # List team members
    linear u me                             # View your profile"#)]
    Users {
        #[command(subcommand)]
        action: users::UserCommands,
    },
    /// Manage cycles - view sprint cycles and current cycle
    #[command(alias = "c")]
    #[command(after_help = r#"EXAMPLES:
    linear cycles list -t ENG               # List team cycles
    linear c current -t ENG                 # Show current cycle"#)]
    Cycles {
        #[command(subcommand)]
        action: cycles::CycleCommands,
    },
    /// Manage comments - add and view issue comments
    #[command(alias = "cm")]
    #[command(after_help = r#"EXAMPLES:
    linear comments list ISSUE_ID           # List comments on issue
    linear cm create ISSUE_ID -b "LGTM!"    # Add a comment"#)]
    Comments {
        #[command(subcommand)]
        action: comments::CommentCommands,
    },
    /// Manage documents - create and organize documentation
    #[command(alias = "d")]
    #[command(after_help = r#"EXAMPLES:
    linear documents list                   # List all documents
    linear d get DOC_ID                     # View document
    linear d create "Design Doc" -p PROJ_ID # Create document"#)]
    Documents {
        #[command(subcommand)]
        action: documents::DocumentCommands,
    },
    /// Search across Linear - find issues and projects
    #[command(alias = "s")]
    #[command(after_help = r#"EXAMPLES:
    linear search issues "auth bug"         # Search issues
    linear s projects "backend"             # Search projects"#)]
    Search {
        #[command(subcommand)]
        action: search::SearchCommands,
    },
    /// Sync operations - compare local folders with Linear
    #[command(alias = "sy")]
    #[command(after_help = r#"EXAMPLES:
    linear sync status                      # Compare local vs Linear
    linear sy push -t ENG                   # Create projects for folders
    linear sy push -t ENG --dry-run         # Preview without creating"#)]
    Sync {
        #[command(subcommand)]
        action: sync::SyncCommands,
    },
    /// Manage issue statuses - view workflow states
    #[command(alias = "st")]
    #[command(after_help = r#"EXAMPLES:
    linear statuses list -t ENG             # List team statuses
    linear st get "In Progress" -t ENG      # View status details"#)]
    Statuses {
        #[command(subcommand)]
        action: statuses::StatusCommands,
    },
    /// Git branch operations - checkout branches, create PRs
    #[command(alias = "g")]
    #[command(after_help = r#"EXAMPLES:
    linear git checkout LIN-123             # Checkout issue branch
    linear g branch LIN-123                 # Show branch name
    linear g pr LIN-123                     # Create GitHub PR
    linear g pr LIN-123 --draft             # Create draft PR"#)]
    Git {
        #[command(subcommand)]
        action: git::GitCommands,
    },
    /// Bulk operations - update multiple issues at once
    #[command(alias = "b")]
    #[command(after_help = r#"EXAMPLES:
    linear bulk update -s Done LIN-1 LIN-2  # Update multiple issues
    linear b assign --user me LIN-1 LIN-2   # Assign multiple issues
    linear b label --add bug LIN-1 LIN-2    # Add label to issues"#)]
    Bulk {
        #[command(subcommand)]
        action: bulk::BulkCommands,
    },
    /// Manage cache - clear cached data or view status
    #[command(alias = "ca")]
    #[command(after_help = r#"EXAMPLES:
    linear cache status                     # Show cache status
    linear ca clear                         # Clear all cache
    linear ca clear --type teams            # Clear only teams cache"#)]
    Cache {
        #[command(subcommand)]
        action: commands::cache::CacheCommands,
    },
    /// Manage notifications - view and mark as read
    #[command(alias = "n")]
    #[command(after_help = r#"EXAMPLES:
    linear notifications list               # List unread notifications
    linear n count                          # Show unread count
    linear n read-all                       # Mark all as read"#)]
    Notifications {
        #[command(subcommand)]
        action: notifications::NotificationCommands,
    },
    /// Manage issue templates - create and use templates
    #[command(alias = "tpl")]
    #[command(after_help = r#"EXAMPLES:
    linear templates list                   # List all templates
    linear tpl create bug                   # Create a new template
    linear tpl show bug                     # View template details"#)]
    Templates {
        #[command(subcommand)]
        action: templates::TemplateCommands,
    },
    /// Time tracking - log and view time entries
    #[command(alias = "tm")]
    #[command(after_help = r#"EXAMPLES:
    linear time log LIN-123 2h              # Log 2 hours on issue
    linear tm list --issue LIN-123          # List time entries"#)]
    Time {
        #[command(subcommand)]
        action: time::TimeCommands,
    },
    /// Fetch uploads from Linear with authentication
    #[command(alias = "up")]
    #[command(after_help = r#"EXAMPLES:
    linear uploads fetch URL                # Output to stdout (for piping)
    linear up fetch URL -f file.png         # Save to file
    linear up fetch URL | base64            # Pipe to another tool"#)]
    Uploads {
        #[command(subcommand)]
        action: uploads::UploadCommands,
    },
    /// Interactive mode - TUI for browsing and managing issues
    #[command(alias = "int")]
    #[command(after_help = r#"EXAMPLES:
    linear interactive                      # Launch interactive mode
    linear interactive --team ENG           # Preselect team

Use arrow keys to navigate, Enter to select, q to quit."#)]
    Interactive {
        /// Preselect team by key, name, or ID
        #[arg(short, long)]
        team: Option<String>,
    },
    /// Detect current Linear issue from git branch - for AI agents
    #[command(alias = "ctx")]
    #[command(after_help = r#"EXAMPLES:
    linear context                          # Show current issue from branch
    linear ctx --output json                # Get as JSON for parsing

Detects issue ID from branch names like:
  - lin-123-fix-bug
  - feature/LIN-456-new-feature
  - scw-789-some-task"#)]
    Context,
    /// Manage favorites - quick access to issues/projects
    #[command(alias = "fav")]
    #[command(after_help = r#"EXAMPLES:
    linear favorites list                   # List favorites
    linear fav add LIN-123                  # Add issue to favorites
    linear fav remove LIN-123               # Remove from favorites"#)]
    Favorites {
        #[command(subcommand)]
        action: favorites::FavoriteCommands,
    },
    /// Manage roadmaps - view roadmap planning
    #[command(alias = "rm")]
    #[command(after_help = r#"EXAMPLES:
    linear roadmaps list                    # List all roadmaps
    linear rm get ROADMAP_ID                # View roadmap details"#)]
    Roadmaps {
        #[command(subcommand)]
        action: roadmaps::RoadmapCommands,
    },
    /// Manage initiatives - high-level tracking
    #[command(alias = "init")]
    #[command(after_help = r#"EXAMPLES:
    linear initiatives list                 # List all initiatives
    linear init get INITIATIVE_ID           # View initiative details"#)]
    Initiatives {
        #[command(subcommand)]
        action: initiatives::InitiativeCommands,
    },
    /// Triage inbox - manage unassigned issues
    #[command(alias = "tr")]
    #[command(after_help = r#"EXAMPLES:
    linear triage list                      # List triage issues
    linear tr claim LIN-123                 # Claim an issue
    linear tr snooze LIN-123 --duration 1w  # Snooze for a week"#)]
    Triage {
        #[command(subcommand)]
        action: triage::TriageCommands,
    },
    /// View metrics - velocity, burndown, progress
    #[command(alias = "mt")]
    #[command(after_help = r#"EXAMPLES:
    linear metrics cycle CYCLE_ID           # Cycle metrics
    linear mt project PROJECT_ID            # Project progress
    linear mt velocity TEAM --cycles 5      # Team velocity"#)]
    Metrics {
        #[command(subcommand)]
        action: metrics::MetricsCommands,
    },
    /// Export issues to CSV or Markdown
    #[command(alias = "exp")]
    #[command(after_help = r#"EXAMPLES:
    linear export csv --team ENG            # Export team issues to CSV
    linear exp csv -f issues.csv            # Export to file
    linear exp markdown --team ENG          # Export as Markdown"#)]
    Export {
        #[command(subcommand)]
        action: export::ExportCommands,
    },
    /// View issue history and activity
    #[command(alias = "hist")]
    #[command(after_help = r#"EXAMPLES:
    linear history issue LIN-123            # View issue activity
    linear hist issue LIN-123 --limit 50    # More entries"#)]
    History {
        #[command(subcommand)]
        action: history::HistoryCommands,
    },
    /// Watch for issue updates (polling)
    #[command(after_help = r#"EXAMPLES:
    linear watch LIN-123                    # Watch single issue
    linear watch LIN-123 --interval 30      # Poll every 30 seconds"#)]
    Watch {
        /// Issue identifier to watch
        id: String,
        /// Polling interval in seconds
        #[arg(short, long, default_value = "10")]
        interval: u64,
    },
    /// Manage issue relationships - parent/child, blocking, related
    #[command(alias = "rel")]
    #[command(after_help = r#"EXAMPLES:
    linear relations list LIN-123           # List issue relationships
    linear rel add LIN-1 -r blocks LIN-2    # LIN-1 blocks LIN-2
    linear rel parent LIN-2 LIN-1           # Set LIN-1 as parent of LIN-2
    linear rel unparent LIN-2               # Remove parent"#)]
    Relations {
        #[command(subcommand)]
        action: relations::RelationCommands,
    },
    /// Configure CLI settings - API keys and workspaces
    #[command(after_help = r#"EXAMPLES:
    linear config set-key YOUR_API_KEY      # Set API key
    linear config set api-key YOUR_API_KEY  # Set API key (alt)
    linear config get api-key               # Get API key (masked)
    linear config set profile work          # Switch profile
    linear config show                      # Show configuration
    linear config workspace-add work KEY    # Add workspace
    linear config workspace-switch work     # Switch workspace"#)]
    Config {
        #[command(subcommand)]
        action: ConfigCommands,
    },
}

#[derive(Subcommand)]
enum ConfigCommands {
    /// Set API key
    #[command(after_help = r#"EXAMPLE:
    linear config set-key lin_api_xxxxxxxxxxxxx"#)]
    SetKey {
        /// Your Linear API key
        key: String,
    },
    /// Get a configuration value
    Get {
        /// Config key to retrieve (api-key, profile)
        key: String,
        /// Output raw value without masking
        #[arg(long)]
        raw: bool,
    },
    /// Set a configuration value
    Set {
        /// Config key to set (api-key, profile)
        key: String,
        /// Value to set
        value: String,
    },
    /// Show current configuration
    Show,
    /// Generate shell completions
    #[command(after_help = r#"EXAMPLES:
    linear config completions bash > ~/.bash_completion.d/linear
    linear config completions zsh > ~/.zfunc/_linear
    linear config completions fish > ~/.config/fish/completions/linear.fish
    linear config completions powershell > linear.ps1"#)]
    Completions {
        /// Shell to generate completions for
        #[arg(value_enum)]
        shell: Shell,
    },
    /// Add a new workspace
    #[command(alias = "add")]
    #[command(after_help = r#"EXAMPLE:
    linear config workspace-add personal lin_api_xxxxxxxxxxxxx"#)]
    WorkspaceAdd {
        /// Workspace name
        name: String,
        /// API key for this workspace
        api_key: String,
    },
    /// List all workspaces
    #[command(alias = "list")]
    WorkspaceList,
    /// Switch to a different workspace
    #[command(alias = "use")]
    #[command(after_help = r#"EXAMPLE:
    linear config workspace-switch personal"#)]
    WorkspaceSwitch {
        /// Workspace name to switch to
        name: String,
    },
    /// Show current workspace
    #[command(alias = "current")]
    WorkspaceCurrent,
    /// Remove a workspace
    #[command(alias = "rm")]
    WorkspaceRemove {
        /// Workspace name to remove
        name: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    if cli.no_color || cli.color == ColorChoice::Never {
        colored::control::set_override(false);
    } else if cli.color == ColorChoice::Always {
        colored::control::set_override(true);
    }
    set_cli_state(DisplayOptions {
        width: cli.width,
        no_truncate: cli.no_truncate,
    });
    if let Some(key) = cli.api_key.as_deref() {
        std::env::set_var("LINEAR_API_KEY", key);
    }
    if let Some(profile) = cli.profile.as_deref() {
        std::env::set_var("LINEAR_CLI_PROFILE", profile);
    }
    api::set_default_retry(cli.retry);
    let filters = parse_filters(&cli.filter)?;
    let pagination = PaginationOptions {
        limit: cli.limit,
        after: cli.after.clone(),
        before: cli.before.clone(),
        page_size: cli.page_size,
        all: cli.all,
    };
    let json_opts = JsonOutputOptions::new(
        cli.compact,
        if cli.fields.is_empty() {
            None
        } else {
            Some(cli.fields.clone())
        },
        cli.sort.clone(),
        cli.order,
        true,
    );
    let output = OutputOptions {
        format: cli.output,
        json: json_opts,
        format_template: cli.format.clone(),
        filters,
        fail_on_empty: cli.fail_on_empty,
        pagination,
        cache: cache::CacheOptions {
            ttl_seconds: cli.cache_ttl,
            no_cache: cli.no_cache,
        },
        dry_run: cli.dry_run,
    };
    let agent_opts = AgentOptions {
        quiet: cli.quiet,
        id_only: cli.id_only,
        dry_run: cli.dry_run,
    };

    if cli.schema {
        let schema = serde_json::json!({
            "schema_version": "1.0",
            "schema_file": "docs/json/schema.json",
        });
        if matches!(cli.output, OutputFormat::Ndjson) || cli.compact {
            println!("{}", serde_json::to_string(&schema)?);
        } else {
            println!("{}", serde_json::to_string_pretty(&schema)?);
        }
        std::process::exit(0);
    }

    let result = run_command(cli.command, &output, agent_opts, cli.retry).await;

    match result {
        Ok(()) => std::process::exit(0),
        Err(e) => {
            // Check if JSON output requested for structured errors
            if output.is_json() {
                if let Some(cli_error) = e.downcast_ref::<CliError>() {
                    let error_json = serde_json::json!({
                        "error": true,
                        "message": cli_error.message,
                        "code": cli_error.code,
                        "details": cli_error.details,
                        "retry_after": cli_error.retry_after,
                    });
                    eprintln!(
                        "{}",
                        serde_json::to_string(&error_json).unwrap_or_else(|_| e.to_string())
                    );
                } else {
                    let error_json = serde_json::json!({
                        "error": true,
                        "message": e.to_string(),
                        "code": categorize_error(&e),
                        "details": null,
                        "retry_after": null,
                    });
                    eprintln!(
                        "{}",
                        serde_json::to_string(&error_json).unwrap_or_else(|_| e.to_string())
                    );
                }
            } else {
                eprintln!("Error: {}", e);
            }
            std::process::exit(categorize_error(&e) as i32);
        }
    }

    #[allow(unreachable_code)]
    Ok(())
}

/// Categorize error for exit codes: 1=general error, 2=not found, 3=auth error
fn categorize_error(e: &anyhow::Error) -> u8 {
    if let Some(cli_error) = e.downcast_ref::<CliError>() {
        return cli_error.code;
    }
    let msg = e.to_string().to_lowercase();
    if msg.contains("not found") || msg.contains("does not exist") {
        2
    } else if msg.contains("unauthorized")
        || msg.contains("api key")
        || msg.contains("authentication")
    {
        3
    } else if msg.contains("rate limit") || msg.contains("too many requests") {
        4
    } else {
        1
    }
}

async fn run_command(
    command: Commands,
    output: &OutputOptions,
    agent_opts: AgentOptions,
    retry: u32,
) -> Result<()> {
    match command {
        Commands::Common => {
            println!("Common tasks:");
            println!("  linear issues list -t ENG");
            println!("  linear issues get LIN-123");
            println!("  linear issues create \"Title\" -t ENG");
            println!("  linear issues update LIN-123 -s Done");
            println!("  linear projects list");
            println!("  linear teams list");
            println!("  linear git checkout LIN-123");
            println!("  linear git pr LIN-123 --draft");
            println!("  linear interactive --team ENG");
            println!();
            println!("Tips:");
            println!("  Use --help after any command for more options.");
            println!("  Use --output json or --output ndjson for scripting/LLMs.");
            println!("  Use --no-color for logs/CI.");
            println!("  Use --limit/--page-size/--all for pagination.");
        }
        Commands::Agent => {
            println!("Agent harness:");
            println!("  Use --output json or --output ndjson for machine-readable output.");
            println!("  Use --compact and --fields to reduce tokens.");
            println!("  Use --sort/--order to stabilize list outputs.");
            println!("  Use --filter to reduce list results.");
            println!("  Use --id-only for chaining create/update commands.");
            println!("  Use --data - for JSON input on issue create/update.");
            println!();
            println!("Examples:");
            println!("  linear issues list --output json --compact --fields identifier,title");
            println!("  linear issues list --output ndjson --filter state.name=In\\ Progress");
            println!("  linear issues get LIN-123 --output json");
            println!("  linear issues update LIN-123 --data - --dry-run");
            println!("  linear context --output json --id-only");
            println!();
            println!("Schemas:");
            println!("  See docs/json/ for sample outputs.");
            println!("  Use --schema to print the current schema version.");
        }
        Commands::Projects { action } => projects::handle(action, output).await?,
        Commands::Issues { action } => issues::handle(action, output, agent_opts).await?,
        Commands::Labels { action } => labels::handle(action, output).await?,
        Commands::Teams { action } => teams::handle(action, output).await?,
        Commands::Users { action } => users::handle(action, output).await?,
        Commands::Cycles { action } => cycles::handle(action, output).await?,
        Commands::Comments { action } => comments::handle(action, output).await?,
        Commands::Documents { action } => documents::handle(action, output).await?,
        Commands::Search { action } => search::handle(action, output).await?,
        Commands::Sync { action } => sync::handle(action, output).await?,
        Commands::Statuses { action } => statuses::handle(action, output).await?,
        Commands::Git { action } => git::handle(action).await?,
        Commands::Bulk { action } => bulk::handle(action, output).await?,
        Commands::Cache { action } => commands::cache::handle(action).await?,
        Commands::Notifications { action } => notifications::handle(action, output).await?,
        Commands::Templates { action } => templates::handle(action, output).await?,
        Commands::Time { action } => time::handle(action, output).await?,
        Commands::Uploads { action } => uploads::handle(action).await?,
        Commands::Interactive { team } => interactive::run(team).await?,
        Commands::Context => handle_context(output, agent_opts, retry).await?,
        Commands::Favorites { action } => favorites::handle(action, output).await?,
        Commands::Roadmaps { action } => {
            let pagination = PaginationOptions::default();
            roadmaps::handle(action, output, &pagination).await?
        }
        Commands::Initiatives { action } => {
            let pagination = PaginationOptions::default();
            initiatives::handle(action, output, &pagination).await?
        }
        Commands::Triage { action } => triage::handle(action, output).await?,
        Commands::Metrics { action } => metrics::handle(action, output).await?,
        Commands::Export { action } => export::handle(action, output).await?,
        Commands::History { action } => history::handle(action, output).await?,
        Commands::Watch { id, interval } => watch::watch_issue(&id, interval, output).await?,
        Commands::Relations { action } => relations::handle(action, output).await?,
        Commands::Auth { action } => auth::handle(action, output).await?,
        Commands::Doctor { check_api } => doctor::run(output, check_api).await?,
        Commands::Config { action } => match action {
            ConfigCommands::SetKey { key } => {
                config::set_api_key(&key)?;
                if !agent_opts.quiet {
                    println!("API key saved successfully!");
                }
            }
            ConfigCommands::Get { key, raw } => {
                config::config_get(&key, raw)?;
            }
            ConfigCommands::Set { key, value } => {
                config::config_set(&key, &value)?;
            }
            ConfigCommands::Show => {
                config::show_config()?;
            }
            ConfigCommands::Completions { shell } => {
                let mut cmd = Cli::command();
                generate(shell, &mut cmd, "linear-cli", &mut std::io::stdout());
            }
            ConfigCommands::WorkspaceAdd { name, api_key } => {
                config::workspace_add(&name, &api_key)?;
            }
            ConfigCommands::WorkspaceList => {
                config::workspace_list()?;
            }
            ConfigCommands::WorkspaceSwitch { name } => {
                config::workspace_switch(&name)?;
            }
            ConfigCommands::WorkspaceCurrent => {
                config::workspace_current()?;
            }
            ConfigCommands::WorkspaceRemove { name } => {
                config::workspace_remove(&name)?;
            }
        },
    }

    Ok(())
}

/// Handle the context command - detect current Linear issue from git branch
async fn handle_context(output: &OutputOptions, agent_opts: AgentOptions, retry: u32) -> Result<()> {
    // Get current git branch
    let branch_output = std::process::Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output();

    let branch = match branch_output {
        Ok(output) if output.status.success() => {
            String::from_utf8_lossy(&output.stdout).trim().to_string()
        }
        _ => {
            anyhow::bail!("Not in a git repository or git not available");
        }
    };

    // Extract issue ID from branch name using regex
    let re = regex::Regex::new(r"(?i)([a-z]+-\d+)").unwrap();

    let issue_id = re
        .find(&branch)
        .map(|m| m.as_str().to_uppercase())
        .ok_or_else(|| anyhow::anyhow!("No Linear issue ID found in branch: {}", branch))?;

    if output.is_json() || output.has_template() {
        if agent_opts.id_only {
            print_json(&serde_json::json!(issue_id), output)?;
            return Ok(());
        }
        // Fetch issue details for JSON output
        let client = api::LinearClient::new_with_retry(retry)?;
        let query = r#"
            query($id: String!) {
                issue(id: $id) {
                    id
                    identifier
                    title
                    state { name }
                    assignee { name }
                    priority
                    url
                }
            }
        "#;

        let result = client
            .query(query, Some(serde_json::json!({ "id": issue_id })))
            .await;

        match result {
            Ok(data) => {
                let issue = &data["data"]["issue"];
                if issue.is_null() {
                    print_json(
                        &serde_json::json!({
                            "branch": branch,
                            "issue_id": issue_id,
                            "found": false,
                        }),
                        output,
                    )?;
                } else {
                    print_json(
                        &serde_json::json!({
                            "branch": branch,
                            "issue_id": issue_id,
                            "found": true,
                            "issue": issue,
                        }),
                        output,
                    )?;
                }
            }
            Err(_) => {
                print_json(
                    &serde_json::json!({
                        "branch": branch,
                        "issue_id": issue_id,
                        "found": false,
                    }),
                    output,
                )?;
            }
        }
    } else {
        println!("{}", issue_id);
    }

    Ok(())
}




