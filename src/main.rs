use anyhow::Result;
use aws_config::{identity::IdentityCache, BehaviorVersion, SdkConfig};
use aws_sdk_sqs::{types::QueueAttributeName, Client};
use aws_types::region::Region;
use clap::Parser;
use colored::Colorize;
use futures::prelude::*;
use serde::Deserialize;
use serde_json::from_str;
use std::{collections::HashSet, time::Duration};
use tracing::debug;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

/// List the status of AWS SQS queues.
///
/// You can set the environment variable `RUST_LOG` to
/// adjust logging, for example `RUST_LOG=trace sqs-status`.
#[derive(Clone, Debug, Parser)]
#[command(about, author, version)]
struct MyArgs {
    /// Print all SQS queues instead of only non-empty queues.
    #[arg(long, short)]
    all: bool,

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

#[derive(Deserialize)]
struct RedrivePolicy {
    #[serde(alias = "deadLetterTargetArn")]
    dead_letter_target_arn: String,
}

#[derive(Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
struct QueueInfo {
    name: String,
    available: String,
    delayed: String,
    not_visible: String,
    dead_letter_queue_name: String,
}

async fn queue_info(client: &Client, queue_url: String) -> Result<QueueInfo> {
    let empty = "".to_string();
    let zero = "0".to_string();
    let attributes = client
        .get_queue_attributes()
        .queue_url(&queue_url)
        .set_attribute_names(
            vec![
                QueueAttributeName::ApproximateNumberOfMessages,
                QueueAttributeName::ApproximateNumberOfMessagesDelayed,
                QueueAttributeName::ApproximateNumberOfMessagesNotVisible,
                QueueAttributeName::RedriveAllowPolicy,
                QueueAttributeName::RedrivePolicy,
            ]
            .into(),
        )
        .send()
        .await?
        .attributes
        .unwrap_or_default();
    Ok(QueueInfo {
        name: queue_url,
        available: attributes
            .get(&QueueAttributeName::ApproximateNumberOfMessages)
            .unwrap_or(&zero)
            .to_string(),
        delayed: attributes
            .get(&QueueAttributeName::ApproximateNumberOfMessagesDelayed)
            .unwrap_or(&zero)
            .to_string(),
        not_visible: attributes
            .get(&QueueAttributeName::ApproximateNumberOfMessagesNotVisible)
            .unwrap_or(&zero)
            .to_string(),
        dead_letter_queue_name: match attributes.get(&QueueAttributeName::RedrivePolicy) {
            Some(redrive) => match from_str::<RedrivePolicy>(redrive) {
                Ok(redrive_policy) => redrive_policy
                    .dead_letter_target_arn
                    .split(':')
                    .last()
                    .unwrap_or(&empty)
                    .to_string(),
                Err(_) => empty,
            },
            None => empty,
        },
    })
}

/*
    let results: Vec<Result<Vec<FunctionVersions>>> = stream::iter(function_names)
        .map(|name| function_versions(&client, name))
        .buffer_unordered(CONCURRENT_REQUESTS)
        .collect()
        .await;
*/

async fn gather_status(client: &Client, queue_urls: Vec<String>) -> Result<Vec<QueueInfo>> {
    let vec_result: Vec<Result<QueueInfo>> = stream::iter(queue_urls)
        .map(|queue_url| queue_info(client, queue_url))
        .buffer_unordered(256)
        .collect()
        .await;
    vec_result.into_iter().collect()
}

fn report_status(results: &Vec<QueueInfo>, all: bool) {
    let dead_letter_queue_names: HashSet<String> = results
        .iter()
        .map(|item| item.dead_letter_queue_name.clone())
        .collect();
    let mut first = true;
    for item in results {
        if all || item.available != "0" || item.delayed != "0" || item.not_visible != "0" {
            if first {
                first = false;
                println!(
                    "{:>9}  {:>7}  {:>11}  {}  (each {} can be redriven)",
                    "available".bold(),
                    "delayed".bold(),
                    "not_visible".bold(),
                    "name".bold(),
                    "name in italic".italic(),
                );
            }
            let queue_name = item.name.split('/').last().unwrap_or_default();
            let formatted_queue_name = if dead_letter_queue_names.contains(queue_name) {
                queue_name.italic()
            } else {
                queue_name.normal()
            };
            println!(
                "{:>9}  {:>7}  {:>11}  {} ",
                item.available, item.delayed, item.not_visible, formatted_queue_name,
            );
        }
    }
    if first {
        println!("No in flight messages found in any queues.");
    }
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
    let queue_urls = client
        .list_queues()
        .max_results(1000) // This is required to retrieve multiple pages!!
        .into_paginator()
        .items()
        .send()
        .collect::<Result<Vec<_>, _>>()
        .await?;
    if queue_urls.is_empty() {
        println!("No SQS queues found");
    } else {
        println!("Found {} SQS queues", queue_urls.len());
        let mut results = gather_status(&client, queue_urls).await?;
        results.sort_unstable();
        report_status(&results, args.all);
    }
    Ok(())
}
