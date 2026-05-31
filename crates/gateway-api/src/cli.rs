//! CLI argument parsing for the gateway binary.

use std::env;

#[derive(Debug, Clone)]
pub enum Command {
    /// Run the API server (default).
    Serve,
    /// Run the interactive config wizard.
    Config,
    /// Print the active routing profile.
    Profile,
    /// Open the TUI dashboard.
    Dashboard,
    /// Print help.
    Help,
}

impl Command {
    pub fn from_args() -> Self {
        let args: Vec<String> = env::args().collect();
        match args.get(1).map(|s| s.as_str()) {
            Some("config") => Command::Config,
            Some("profile") => Command::Profile,
            Some("dashboard") => Command::Dashboard,
            Some("help") | Some("--help") | Some("-h") => Command::Help,
            Some("serve") | None => Command::Serve,
            Some(unknown) => {
                eprintln!("Unknown command: {}. Use 'help' for usage.", unknown);
                std::process::exit(1);
            }
        }
    }

    pub fn help_text() -> &'static str {
        r#"AI Gateway

USAGE:
    gateway [COMMAND]

COMMANDS:
    serve       Run the API server (default)
    config      Run the interactive setup wizard
    profile     Show the currently configured routing profile
    dashboard   Open the TUI monitoring dashboard
    help        Print this message

EXAMPLES:
    gateway                    # Start the server
    gateway config             # Run setup wizard
    gateway profile            # Show active profile
    gateway dashboard          # Open TUI dashboard
"#
    }
}
