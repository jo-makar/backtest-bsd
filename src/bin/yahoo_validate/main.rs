use backtest_bsd::yahoo;

use log::LevelFilter;
use simple_logger::SimpleLogger;

use std::error::Error;
use std::fs;
use std::panic;
use std::path::PathBuf;
use std::thread::{self, ScopedJoinHandle};

fn main() -> Result<(), Box<dyn Error>> {
    SimpleLogger::new()
        .with_level(LevelFilter::Info)
        .with_threads(true)
        .init()?;

    let num_cores = thread::available_parallelism()?.get();
    log::info!("found {} cores", num_cores);

    let files: Vec<PathBuf> = {
        let mut files = Vec::new();
        for entry in fs::read_dir("data/yahoo/")? {
            let path = entry?.path();
            if path.is_file() {
                files.push(path);
            }
        }
        files
    };
    log::info!("found {} yahoo files", files.len());

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
                    log::info!("will process {} files", files.len());

                    for file in files {
                        log::info!("processing {:?}", file);
                        yahoo::parse_chart(&fs::read_to_string(file)?)?;
                    }

                    Ok(())
                },
            )?);
        }

        for handle in handles {
            match handle.join() {
                Ok(Ok(())) => {}
                Ok(Err(e)) => return Err(e),
                Err(e) => panic::resume_unwind(e),
            }
        }

        Ok(())
    })
}
