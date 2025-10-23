mod scheduler;

use crate::scheduler::plan::SchedulerPlan;
use actix_web::{App, HttpServer, Responder, web};
use clap::Parser;
use dotenv::dotenv;
use lazy_static::lazy_static;
use log::{error, info};
use prometheus::{Gauge, register_gauge};
use reqwest::Client;
use std::net::IpAddr;
use std::{env, sync::Arc, time::Duration};
use tokio::time::interval;
use web3::types::{Address, U256};
use web3::{Web3, transports::Http};

lazy_static! {
    static ref BATCHER_BALANCE_GAUGE: Gauge =
        register_gauge!("batcher_balance_eth", "ETH balance of the L1 batcher").unwrap();
    static ref SCHEDULER_HEARTBEAT_GAUGE: Gauge = register_gauge!(
        "scheduler_last_heartbeat",
        "Last timestamp at which the scheduler was active"
    )
    .unwrap();
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(long, env = "ALEPH_MONITOR_HOST", default_value = "0.0.0.0", value_parser = clap::value_parser!(IpAddr))]
    #[doc = "Bind to this IP address."]
    host: IpAddr,

    #[arg(long, env = "ALEPH_MONITOR_PORT", default_value_t = 8000)]
    #[doc = "Bind to this port."]
    port: u16,

    #[arg(long, env = "ALEPH_MONITOR_UPDATE_INTERVAL", default_value_t = 60)]
    #[doc = "Update interval, in seconds."]
    update_interval: u64,
}

async fn metrics() -> impl Responder {
    use prometheus::Encoder;
    let encoder = prometheus::TextEncoder::new();
    let mut buffer = Vec::new();
    encoder.encode(&prometheus::gather(), &mut buffer).unwrap();
    String::from_utf8(buffer).unwrap()
}

/// Converts a wei balance to a floating-point ETH balance while avoiding precision loss.
fn wei_to_eth(balance: U256) -> f64 {
    let wei_per_eth = web3::types::U256::from(1_000_000_000_000_000_000u64); // 10^18
    let eth_whole = balance / wei_per_eth;
    let wei_remainder = balance % wei_per_eth;

    // Convert to f64 for Prometheus (accepts some precision loss for the fractional part)
    eth_whole.as_u64() as f64 + (wei_remainder.as_u128() as f64 / 1e18)
}

async fn update_metrics(
    web3: Arc<Web3<Http>>,
    eth_address: Address,
    http_client: Client,
    scheduler_url: &str,
    refresh_interval: Duration,
) {
    let mut interval = interval(refresh_interval);
    loop {
        interval.tick().await;

        match web3.eth().balance(eth_address, None).await {
            Ok(balance) => {
                let balance_eth = wei_to_eth(balance);
                BATCHER_BALANCE_GAUGE.set(balance_eth);
                info!("Updated batcher balance: {} ETH", balance_eth);
            }
            Err(e) => error!("Failed to fetch batcher balance: {}", e),
        }

        match http_client
            .get(format!("{scheduler_url}/api/v0/plan"))
            .send()
            .await
        {
            Ok(resp) => match resp.json::<SchedulerPlan>().await {
                Ok(plan) => {
                    info!("Last scheduler heartbeat: {}", plan.period.start_timestamp);
                    SCHEDULER_HEARTBEAT_GAUGE.set(plan.period.start_timestamp.timestamp() as f64);
                }
                Err(e) => error!("Failed to parse scheduler response: {}", e),
            },
            Err(e) => error!("Failed to fetch scheduler status: {}", e),
        }
    }
}

fn read_env_var(name: &str) -> Result<String, anyhow::Error> {
    env::var(name).map_err(|_| anyhow::anyhow!("{} env var not set", name))
}

#[actix_web::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();
    env_logger::init();

    let args = Args::parse();
    info!("Starting Aleph monitor service");

    let eth_address: Address = read_env_var("ALEPH_BATCHER_ADDRESS")?
        .parse()
        .map_err(|e| anyhow::anyhow!("Invalid ALEPH_BATCHER_ADDRESS: {}", e))?;
    let eth_rpc_url = read_env_var("ETHEREUM_RPC_URL")?;
    let scheduler_url = read_env_var("ALEPH_SCHEDULER_API_URL")?;

    let transport = Http::new(&eth_rpc_url)?;
    let web3 = Arc::new(Web3::new(transport));
    let http_client = Client::new();

    tokio::spawn(async move {
        update_metrics(
            web3,
            eth_address,
            http_client,
            &scheduler_url,
            Duration::from_secs(args.update_interval),
        )
        .await;
    });

    HttpServer::new(move || App::new().route("/metrics", web::get().to(metrics)))
        .bind((args.host.to_string(), args.port))?
        .run()
        .await?;

    Ok(())
}
