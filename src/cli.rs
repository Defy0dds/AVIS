use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "avis",
    about = "Multi-identity email operations for AI agents",
    version = "1.0.0",
    arg_required_else_help = true
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Add a new identity via OAuth2 PKCE
    Add {
        /// Short name for this identity (e.g. ops, personal, work)
        name: String,
    },

    /// List all identities
    #[command(alias = "list")]
    Ls,

    /// Show identity details
    Show {
        /// Identity name
        name: String,
    },

    /// Remove an identity
    #[command(alias = "remove", alias = "delete")]
    Rm {
        /// Identity name
        name: String,
    },

    /// Send an email
    Send {
        /// Identity name to send as
        identity: String,

        /// Recipient email address
        #[arg(short = 't', long = "to")]
        to: String,

        /// Subject line
        #[arg(short = 's', long = "subject")]
        subject: String,

        /// Message body (plain text)
        #[arg(short = 'b', long = "body")]
        body: String,

        /// Attach file(s) to the email (can be repeated)
        #[arg(short = 'a', long = "attach")]
        attach: Vec<String>,
    },

    /// Read inbox messages
    Read {
        /// Identity name
        identity: String,

        /// Return only the latest message
        #[arg(long)]
        latest: bool,

        /// Filter by sender (case-insensitive substring)
        #[arg(short = 'f', long = "from")]
        from: Option<String>,

        /// Filter by subject (case-insensitive substring)
        #[arg(short = 's', long = "subject")]
        subject: Option<String>,

        /// Number of messages to return (default: 10)
        #[arg(short = 'n', long = "count", default_value = "10")]
        count: usize,

        /// Full output including headers and metadata
        #[arg(long)]
        verbose: bool,

        /// Auto-download attachments to this directory
        #[arg(long = "download-dir")]
        download_dir: Option<String>,
    },

    /// Wait for a matching email to arrive
    Wait {
        /// Identity name
        identity: String,

        /// Match on sender (case-insensitive substring)
        #[arg(short = 'f', long = "from")]
        from: Option<String>,

        /// Match on subject (case-insensitive substring)
        #[arg(short = 's', long = "subject")]
        subject: Option<String>,

        /// Seconds to wait before timeout (default: 60)
        #[arg(short = 't', long = "timeout", default_value = "60")]
        timeout: u64,

        /// Auto-download attachments to this directory
        #[arg(long = "download-dir")]
        download_dir: Option<String>,
    },

    /// Extract OTP codes or links from an email
    Extract {
        /// Identity name
        identity: String,

        /// Target a specific message by ID (default: latest)
        #[arg(long = "id")]
        message_id: Option<String>,

        /// Extract all numeric codes (4-8 digits)
        #[arg(long, conflicts_with_all = ["links", "first_code", "first_link"])]
        codes: bool,

        /// Extract all URLs
        #[arg(long, conflicts_with_all = ["codes", "first_code", "first_link"])]
        links: bool,

        /// Extract first numeric code found
        #[arg(long, conflicts_with_all = ["codes", "links", "first_link"])]
        first_code: bool,

        /// Extract first URL found
        #[arg(long, conflicts_with_all = ["codes", "links", "first_code"])]
        first_link: bool,
    },

    /// Download attachments from an email
    Download {
        /// Identity name
        identity: String,

        /// Message ID to download attachments from (default: latest)
        #[arg(long = "id")]
        message_id: Option<String>,

        /// Directory to save attachments to (default: system temp dir / avis / identity)
        #[arg(short = 'd', long = "dir")]
        dir: Option<String>,
    },
}
