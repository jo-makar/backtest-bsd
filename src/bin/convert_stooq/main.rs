use chrono::NaiveDate;
use log::info;
use serde::Deserialize;
use simple_logger::SimpleLogger;

use std::error::Error;
use std::fs;
use std::path::PathBuf;
use std::{thread, thread::ScopedJoinHandle};

fn main() -> Result<(), Box<dyn Error>> {
    const DAILY_PATH: &str = "data/stooq/data-daily";
    const MONTHLY_PATH: &str = "data/stooq/data-monthly";
    const START_DATE: NaiveDate = NaiveDate::from_ymd_opt(2009, 1, 1).unwrap();

    SimpleLogger::new().with_threads(true).init()?;

    let num_cores = thread::available_parallelism()?.get();
    info!("found {} cores", num_cores);

    let files: Vec<PathBuf> = {
        let mut files = Vec::new();
        let mut dirs = vec![PathBuf::from(DAILY_PATH)];

        while let Some(curdir) = dirs.pop() {
            for entry in fs::read_dir(curdir)? {
                let entry: fs::DirEntry = entry?;
                let path = entry.path();

                if path.is_dir() {
                    dirs.push(path);
                } else if path.is_file() {
                    files.push(path);
                }
            }
        }

        files
    };
    info!("found {} daily stooq files", files.len());

    let file_slices: Vec<&[PathBuf]> = files.chunks(files.len().div_ceil(num_cores)).collect();
    assert!(file_slices.len() == num_cores);

    thread::scope(|scope| -> Result<(), Box<dyn Error>> {
        let mut handles: Vec<ScopedJoinHandle<Result<(), Box<dyn Error + Send + Sync>>>> =
            Vec::new();

        for i in 0..num_cores {
            let files: &[PathBuf] = file_slices[i];
            let builder = thread::Builder::new().name(format!("thread-{}", i));
            handles.push(builder.spawn_scoped(
                scope,
                move || -> Result<(), Box<dyn Error + Send + Sync>> {
                    info!("will process {} files", files.len());

                    for file in files {
                        info!("processing {:?}", file);
                        let mut reader = csv::Reader::from_path(&file)?;
                        for result in reader.deserialize() {
                            let record: DailyRecord = result?;
                            if record.date < START_DATE {
                                continue;
                            }

                            if record.open > record.high || record.open < record.low {
                                return Err(format!("{:?} {} invalid open", &file, record.date).into());
                            }
                            if record.close > record.high || record.close < record.low {
                                return Err(format!("{:?} {} invalid close", &file, record.date).into());
                            }

                            // FIXME STOPPED
                        }
                    }

                    Ok(())
                },
            )?);
        }

        for handle in handles {
            match handle.join() {
                Ok(Ok(())) => {}
                Ok(Err(e)) => return Err(e),
                Err(e) => std::panic::resume_unwind(e),
            }
        }

        Ok(())
    })
}

#[derive(Debug, Deserialize)]
struct DailyRecord {
    #[serde(rename = "<TICKER>", skip_deserializing, default)]
    _ticker: String,
    #[serde(rename = "<PER>", skip_deserializing, default)]
    _per: String,
    #[serde(rename = "<DATE>", with = "record_date_format")]
    date: NaiveDate,
    #[serde(rename = "<TIME>", skip_deserializing, default)]
    _time: String,
    #[serde(rename = "<OPEN>")]
    open: f64,
    #[serde(rename = "<HIGH>")]
    high: f64,
    #[serde(rename = "<LOW>")]
    low: f64,
    #[serde(rename = "<CLOSE>")]
    close: f64,
}

mod record_date_format {
    use chrono::NaiveDate;
    use serde::{Deserialize, Deserializer};

    pub fn deserialize<'de, D>(deserializer: D) -> Result<NaiveDate, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        NaiveDate::parse_from_str(&s, "%Y%m%d").map_err(serde::de::Error::custom)
    }
}
