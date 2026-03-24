mod auth;
mod cli;
mod commands;
mod config;
mod crypto;
mod errors;
mod output;
mod sanitize;

use clap::Parser;
use cli::{Cli, Command};

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let home = config::resolve_home();

    match cli.command {
        Command::Init {
            home: home_override,
        } => {
            let target = home_override.map(std::path::PathBuf::from).unwrap_or(home);
            commands::init::run(&target).await;
        }

        Command::Add { name } => {
            commands::identity::add(&home, &name).await;
        }

        Command::Ls => {
            commands::identity::list(&home).await;
        }

        Command::Show { name } => {
            commands::identity::show(&home, &name).await;
        }

        Command::Rm { name } => {
            commands::identity::remove(&home, &name).await;
        }

        Command::Send {
            identity,
            to,
            subject,
            body,
            attach,
        } => {
            commands::send::run(&home, &identity, &to, &subject, &body, &attach).await;
        }

        Command::Read {
            identity,
            latest,
            from,
            subject,
            count,
            verbose,
            download_dir,
        } => {
            commands::read::run(
                &home,
                &identity,
                latest,
                from.as_deref(),
                subject.as_deref(),
                count,
                verbose,
                download_dir.as_deref(),
            )
            .await;
        }

        Command::Wait {
            identity,
            from,
            subject,
            timeout,
            download_dir,
        } => {
            commands::wait::run(
                &home,
                &identity,
                from.as_deref(),
                subject.as_deref(),
                timeout,
                download_dir.as_deref(),
            )
            .await;
        }

        Command::Download {
            identity,
            message_id,
            dir,
        } => {
            commands::download::run(&home, &identity, message_id.as_deref(), &dir).await;
        }

        Command::Extract {
            identity,
            message_id,
            codes,
            links,
            first_code,
            first_link,
        } => {
            commands::extract::run(
                &home,
                &identity,
                message_id.as_deref(),
                codes,
                links,
                first_code,
                first_link,
            )
            .await;
        }
    }
}
