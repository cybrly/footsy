use std::net::{IpAddr, Ipv4Addr};
use std::sync::Arc;
use std::time::Duration;

use clap::Parser;
use colored::Colorize;
use futures::future::join_all;
use governor::{Quota, RateLimiter};
use hyper::{Client, Uri};
use hyper_rustls::HttpsConnectorBuilder;
use local_ipaddress::get;
use nonzero_ext::nonzero;
use tokio::net::TcpStream;
use tokio::sync::{Mutex, Semaphore};
use tokio::time::timeout;
use regex::Regex;


#[derive(Parser)]
struct Cli {
    /// Subnet size to scan (e.g., 24, 16, 8)
    #[clap(default_value = "24")]
    subnet_size: u8,
}

const PORTS: &[u16] = &[80, 443, 8008, 3000, 5000, 9080, 9443, 8000, 8001, 8080, 8443, 9000, 9001];
const CONCURRENT_REQUESTS: usize = 100;
const RATE_LIMIT: u32 = 300;
const PING_TIMEOUT: Duration = Duration::from_millis(700);
const TCP_TIMEOUT: Duration = Duration::from_millis(700);
const HTTP_TIMEOUT: Duration = Duration::from_secs(6);

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Cli::parse();
    let local_ip: IpAddr = get().unwrap().parse()?;

    let ip_ranges = calculate_ip_ranges(local_ip, args.subnet_size);
    let total_ips = ip_ranges.len();

    let rate_limiter = Arc::new(RateLimiter::direct(Quota::per_second(nonzero!(RATE_LIMIT))));
    let semaphore = Arc::new(Semaphore::new(CONCURRENT_REQUESTS));

    let scanned_ips = Arc::new(Mutex::new(0));

    // Shared channel for real-time results
    let (tx, mut rx) = tokio::sync::mpsc::channel(100);

    // Spawn tasks for scanning
    let tasks = ip_ranges.into_iter().map(|ip| {
        let rate_limiter = Arc::clone(&rate_limiter);
        let semaphore = Arc::clone(&semaphore);
        let scanned_ips = Arc::clone(&scanned_ips);
        let tx = tx.clone();
        tokio::spawn(async move {
            let _permit = semaphore.acquire_owned().await.unwrap();
            rate_limiter.until_ready().await;

            if is_ip_responsive(&ip).await {
                let scan_results = scan_ip(ip).await;
                for result in scan_results {
                    let _ = tx.send(result).await;
                }
            }

            let mut count = scanned_ips.lock().await;
            *count += 1;
        })
    });

    // Spawn a task to print results in real-time
    let print_handle = tokio::spawn(async move {
        while let Some(result) = rx.recv().await {
            println!("{}", result);
        }
    });

    // Wait for all scanning tasks to complete
    join_all(tasks).await;

    // Close the channel
    drop(tx);

    // Wait for the print task to finish
    print_handle.await?;

    Ok(())
}

async fn is_ip_responsive(ip: &IpAddr) -> bool {
    let payload = [0; 8];
    match timeout(PING_TIMEOUT, surge_ping::ping(*ip, &payload)).await {
        Ok(Ok(_)) => true,
        _ => false,
    }
}

fn calculate_ip_ranges(base_ip: IpAddr, subnet_size: u8) -> Vec<IpAddr> {
    if let IpAddr::V4(ipv4) = base_ip {
        let host_bits = 32 - subnet_size;
        let network = u32::from(ipv4) & !((1 << host_bits) - 1);
        let broadcast = network | ((1 << host_bits) - 1);

        (network..=broadcast)
            .map(|ip| IpAddr::V4(Ipv4Addr::from(ip)))
            .collect()
    } else {
        vec![]
    }
}

async fn scan_ip(ip: IpAddr) -> Vec<ScanResult> {
    let mut results = Vec::new();
    let scan_tasks = PORTS.iter().map(|&port| {
        let ip = ip;
        tokio::spawn(async move {
            if let Ok(Ok(_)) = timeout(TCP_TIMEOUT, TcpStream::connect((ip, port))).await {
                for scheme in &["http", "https"] {
                    let url = format!("{}://{}:{}", scheme, ip, port);
                    match check_web_server(&url).await {
                        Ok(result) => return Some(result),
                        Err(_) => {} // Suppress error messages
                    }
                }
            }
            None
        })
    });

    for task in join_all(scan_tasks).await {
        if let Ok(Some(result)) = task {
            results.push(result);
        }
    }

    results
}

async fn check_web_server(url: &str) -> Result<ScanResult, Box<dyn std::error::Error>> {
    let https = HttpsConnectorBuilder::new()
        .with_native_roots()
        .https_or_http()
        .enable_http1()
        .build();

    let client: Client<_, hyper::Body> = Client::builder().build(https);
    let uri: Uri = url.parse()?;

    let response = timeout(HTTP_TIMEOUT, client.get(uri.clone())).await??;
    let status = response.status().as_u16();
    let body = hyper::body::to_bytes(response.into_body()).await?;
    let title = extract_title(&body);

    Ok(ScanResult {
        ip_port: uri.to_string(),
        status,
        title,
    })
}

fn extract_title(body: &[u8]) -> String {
    let body_str = String::from_utf8_lossy(body);
    
    // Regular expression to match the title tag content
    let title_re = Regex::new(r"(?i)<title[^>]*>(.*?)</title>").unwrap();
    
    if let Some(captures) = title_re.captures(&body_str) {
        return captures.get(1).map_or("No Title Found".to_string(), |m| m.as_str().to_string());
    }

    "No Title Found".to_string()
}

#[derive(Debug)]
struct ScanResult {
    ip_port: String,
    status: u16,
    title: String,
}

impl std::fmt::Display for ScanResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let status_color = match self.status {
            200..=299 => "green",
            300..=399 => "blue",
            400..=499 => "yellow",
            500..=599 => "red",
            _ => "white",
        };

        write!(
            f,
            "{} {} {} {}",
            format!("[{}]", self.status).color(status_color).bold(),
            self.title.trim().bright_magenta(),
            "->".bright_black(),
            self.ip_port.bright_cyan()
        )
    }
}
