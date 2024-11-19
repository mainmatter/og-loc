use std::{path::PathBuf, time::Duration};

use async_observable::Observable;
use tokio::{
    io::{stdout, AsyncWrite, AsyncWriteExt},
    sync::mpsc,
};

use crate::{convert::HTTP_CLIENT, error::Error, CommonArgs};

#[derive(Debug, clap::Args)]
pub struct Prepare {
    /// The number of crate names to fetch
    #[arg(short = 'n', long, env)]
    num_pages: usize,

    /// The number of concurrent jobs
    #[arg(short, long, env, default_value_t = 10)]
    jobs: usize,

    /// The number of requests per second
    #[arg(short, long, env, default_value_t = 10)]
    rate_limit: u64,

    /// The path to the output file. Defaults to stdout.
    #[arg(env, long = "out", short)]
    pub out_path: Option<PathBuf>,
}

impl Prepare {
    pub async fn run(self, _common: CommonArgs) -> Result<(), Error> {
        #[derive(Debug, serde::Deserialize)]
        struct ApiResponse {
            #[serde(rename = "crates")]
            krates: Vec<Krate>,
        }

        #[derive(Debug, serde::Deserialize)]
        struct Krate {
            name: String,
        }

        let (page_tx, mut page_rx) = tokio::sync::mpsc::channel(self.jobs);
        let (crate_name_tx, mut crate_name_rx) = mpsc::unbounded_channel();

        for page in 0..self.jobs {
            page_tx.send(page + 1).await.unwrap();
        }

        let mut active_jobs = Observable::new(self.jobs);

        let write_task = tokio::spawn(async move {
            let mut writer: Box<dyn AsyncWrite + Unpin + Send> =
                if let Some(out_path) = self.out_path {
                    Box::new(tokio::fs::File::create(out_path).await.unwrap())
                } else {
                    Box::new(stdout())
                };

            while let Some(name) = crate_name_rx.recv().await {
                writer.write_all(String::as_bytes(&name)).await.unwrap();
                writer.write_u8(b'\n').await.unwrap();
            }
            writer.flush().await.unwrap();
        });

        let mut fetch_tasks = tokio::task::JoinSet::new();
        let mut rate_limit_interval =
            tokio::time::interval(Duration::from_micros(1_000_000 / self.rate_limit));

        loop {
            tokio::select! {
                active_jobs = active_jobs.next() => {
                    if active_jobs == 0 {
                        break;
                    }
                }
                page = page_rx.recv() => {
                    rate_limit_interval.tick().await;
                    let Some(page) = page else {
                        break;
                    };
                    let jobs = self.jobs;
                    if page > self.num_pages {
                        active_jobs.modify(|j| *j -= 1);
                        continue;
                    }

                    fetch_tasks.spawn({
                        let crate_name_tx = crate_name_tx.clone();
                        let page_tx = page_tx.clone();
                        let mut active_jobs = active_jobs.clone();

                        async move {
                            let url = format!("https://crates.io/api/v1/crates?page={page}&per_page=100&sort=recent-downloads");
                            let krates: ApiResponse = HTTP_CLIENT.get(url).send().await.unwrap().error_for_status().unwrap().json().await.unwrap();

                            let mut count = 0;
                            krates.krates.into_iter()
                                .for_each(|name| {
                                    count += 1;
                                    crate_name_tx.send(name.name).unwrap();
                                });
                            let next_page = page + jobs;

                            if count >= 100 {
                                page_tx.send(next_page).await.unwrap();
                            } else {
                                active_jobs.modify(|j| *j -= 1);
                            }
                    }});
                }
            }
        }

        drop(page_tx);
        drop(crate_name_tx);
        fetch_tasks.join_all().await;
        write_task.await.unwrap();

        Ok(())
    }
}
