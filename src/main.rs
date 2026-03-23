mod auth;
mod cli;
mod commands;
mod config;
mod crypto;
mod errors;
mod output;

use clap::Parser;
use cli::{AddTarget, Cli, Command};

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

        Command::Add { target } => match target {
            AddTarget::Id { name, email } => {
                commands::identity::add(&home, &name, &email).await;
            }
        },

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
        } => {
            commands::send::run(&home, &identity, &to, &subject, &body).await;
        }

        Command::Read {
            identity,
            latest,
            from,
            subject,
            count,
            verbose,
        } => {
            commands::read::run(
                &home,
                &identity,
                latest,
                from.as_deref(),
                subject.as_deref(),
                count,
                verbose,
            )
            .await;
        }

        Command::Wait {
            identity,
            from,
            subject,
            timeout,
        } => {
            commands::wait::run(
                &home,
                &identity,
                from.as_deref(),
                subject.as_deref(),
                timeout,
            )
            .await;
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
