use anyhow::{anyhow, Context, Result};
use aws_config::{identity::IdentityCache, BehaviorVersion, SdkConfig};
use aws_sdk_sqs::Client;
use aws_types::region::Region;
use clap::Parser;
use futures::{future::join_all, prelude::*};
use std::{collections::HashMap, time::Duration};
use tracing::{debug, error};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

/// List the status of AWS SQS queues.
///
/// You can set the environment variable `RUST_LOG` to
/// adjust logging, for example `RUST_LOG=trace available-enis`.
#[derive(Clone, Debug, Parser)]
#[command(about, author, version)]
struct MyArgs {
    /// AWS profile to use.
    ///
    /// This overrides the standard (and complex!) AWS profile handling.
    #[arg(long, short)]
    profile: Option<String>,

    /// AWS region to target.
    ///
    /// This override the standard (and complex!) AWS region handling.
    #[arg(long, short)]
    region: Option<String>,
}

async fn aws_sdk_config(args: &MyArgs) -> SdkConfig {
    let base = aws_config::defaults(BehaviorVersion::latest()).identity_cache(
        IdentityCache::lazy()
            .load_timeout(Duration::from_secs(90))
            .build(),
    );
    let with_profile = match &args.profile {
        None => base,
        Some(profile_name) => base.profile_name(profile_name),
    };
    let with_overrides = match &args.region {
        None => with_profile,
        Some(region_name) => with_profile.region(Region::new(region_name.clone())),
    };
    with_overrides.load().await
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env())
        .init();
    let args = MyArgs::parse();
    let config = aws_sdk_config(&args).await;
    debug!("Config: {:#?}", config);
    let client = Client::new(&config);
    Ok(())
}
