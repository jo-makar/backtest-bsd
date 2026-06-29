use chrono::{NaiveDate, TimeZone, Utc};
use log::LevelFilter;
use rand::RngExt;
use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, HeaderValue, USER_AGENT};
use simple_logger::SimpleLogger;

use std::error::Error;
use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

fn main() -> Result<(), Box<dyn Error>> {
    SimpleLogger::new().with_level(LevelFilter::Info).init()?;

    let tickers: Vec<String> = {
        // 1. Download the Stooq daily historical archive from https://stooq.com/db/h/
        // 2. `find stooq/data/daily/ -type f | awk -F/ '{ sub(/\.us\.txt$/, "", $NF); print toupper($NF) }' | sort`
        let reader = BufReader::new(File::open(Path::new("data/stooq-tickers.txt"))?);
        reader.lines().collect::<Result<Vec<String>, io::Error>>()?
    };
    log::info!("{} stooq tickers found", tickers.len());

    let client: Client = {
        let mut headers = HeaderMap::new();
        headers.insert(
            USER_AGENT,
            HeaderValue::from_static(
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
            ),
        );
        Client::builder().default_headers(headers).build()?
    };

    const DOMAINS: [&str; 2] = ["query1.finance.yahoo.com", "query2.finance.yahoo.com"];

    #[allow(non_snake_case)]
    let START_DATE_TS: i64 = {
        let date = NaiveDate::from_ymd_opt(2008, 1, 1).unwrap();
        let datetime = date.and_hms_opt(0, 0, 0).unwrap();
        Utc.from_utc_datetime(&datetime).timestamp()
    };
    let end_date_ts: i64 = {
        let date = Utc::now().date_naive();
        let datetime = date.and_hms_opt(0, 0, 0).unwrap();
        Utc.from_utc_datetime(&datetime).timestamp()
    };

    let mut rng = rand::rng();

    for (idx, ticker) in tickers.iter().enumerate() {
        let path = PathBuf::from(format!("data/yahoo/{}.json", ticker.to_lowercase()));
        if path.exists() {
            log::info!("skipping {}, file exists", ticker);
        } else {
            if idx > 0 {
                let delay_ms = rng.random_range(1500..=3500);
                thread::sleep(Duration::from_millis(delay_ms));
            }

            let url: String = {
                let domain = DOMAINS[rng.random_range(0..DOMAINS.len())];
                format!(
                    "https://{}/v8/finance/chart/{}?period1={}&period2={}&interval=1mo&events=div%7Csplit",
                    domain,
                    ticker.to_uppercase(),
                    START_DATE_TS,
                    end_date_ts,
                )
            };

            let resp = client.get(url).send()?;
            if resp.status().is_success() {
                let mut file = File::create(path)?;
                let mut resp = resp;
                io::copy(&mut resp, &mut file)?;
                log::info!("downloaded {} data successfully", ticker);
            } else {
                if resp.status().as_u16() == 404 {
                    log::warn!("{} url returned 404, skipping", ticker);
                } else {
                    return Err(format!("{} url returned {}", ticker, resp.status()).into());
                }
            }
        }
    }

    Ok(())
}
