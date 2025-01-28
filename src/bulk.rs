use std::{fmt, path::PathBuf, pin::pin, str::FromStr, sync::Arc, task::Poll, time::Duration, vec};

use futures_lite::{stream, FutureExt, Stream, StreamExt};
use tokio::{
    fs::File,
    io::{self, stdin, AsyncBufReadExt, AsyncRead, AsyncWriteExt, BufReader, Lines, Stdin},
    sync::Semaphore,
};

use crate::{
    augment::CrateDb,
    error::Error,
    spec::{CrateName, InvalidCrateName},
    CommonArgs,
};

#[derive(Debug, clap::Args)]
pub struct Bulk {
    /// Force overwrite the output.
    #[arg(env, long, short)]
    pub force: bool,
    /// The number of images to render per second.
    #[arg(env, long, short, default_value_t = 1)]
    pub rate: u64,
    /// Input specifier. Either a comma-separated list of crate names, a path to a file containing a newline-separated list of crate names, or `-`, indicating stdin.
    /// Will first attempt to match input with `-`, then parse it as a comma-separated list of crate names, and then fall back to a path, only failing if an empty
    /// value is passed.
    #[arg(env, long = "in", short)]
    pub input: BulkInput,
    /// The path of the folder to which the PNGs should be written
    #[arg(env, long = "out", short)]
    pub out_folder: PathBuf,
}

impl Bulk {
    pub async fn run(self, common: CommonArgs) -> Result<(), Error> {
        let stream = self.input.into_stream().await?;
        let items = stream
            .map(|r| r.map(CrateName::into_inner))
            .try_collect()
            .await
            .unwrap();
        tokio::fs::create_dir_all(&self.out_folder).await?;

        // Add backpressure so we don't open too many files at once.
        // 1000 should be on the safe side
        let semaphore = Arc::new(Semaphore::new(1000));
        // Rate limiter so we don't go fetch images from GitHub too often.
        let mut rate_limit_ticker =
            tokio::time::interval(Duration::from_micros(1000000 / self.rate));

        let db = Arc::new(CrateDb::preload_many(common.db_dump_path, items).await?);

        let mut tasks = tokio::task::JoinSet::new();
        for data in db.augment_preloaded() {
            rate_limit_ticker.tick().await;
            let permit = semaphore.clone().acquire_owned().await.unwrap();
            let image_file_name = format!("{}.png", data.name);
            let path = self.out_folder.join(image_file_name);
            tasks.spawn(async move {
                println!("🖼️  Generating image for crate '{}'", data.name);
                // Move the permit to this task, so it only gets dropped
                // once the task ends
                let _permit = permit;
                let png = data.render_as_png().await;
                let mut file = if self.force {
                    tokio::fs::File::create(path).await?
                } else {
                    tokio::fs::File::create_new(path).await?
                };

                file.write_all(&png).await?;
                Ok::<_, Error>(())
            });
        }

        tasks.join_all().await.into_iter().collect()
    }
}

#[derive(Clone, Default, Debug, Hash, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(try_from = "&str")]
pub enum BulkInput {
    Path(PathBuf),
    List(Vec<CrateName>),
    #[default]
    StdIn,
}

#[derive(thiserror::Error, Debug)]
pub enum BulkInputError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
    #[error("Invalid Crate name: {0}")]
    InvalidCrateName(#[from] InvalidCrateName),
}

impl BulkInput {
    pub async fn into_stream(
        self,
    ) -> Result<impl Stream<Item = Result<CrateName, BulkInputError>>, io::Error> {
        enum BulkInputStream {
            Path(Lines<BufReader<File>>),
            List(stream::Iter<vec::IntoIter<CrateName>>),
            StdIn(Lines<BufReader<Stdin>>),
        }

        impl Stream for BulkInputStream {
            type Item = Result<CrateName, BulkInputError>;

            fn poll_next(
                self: std::pin::Pin<&mut Self>,
                cx: &mut std::task::Context<'_>,
            ) -> std::task::Poll<Option<Self::Item>> {
                fn poll_next_name<R: AsyncRead + Unpin>(
                    cx: &mut std::task::Context<'_>,
                    lines: &mut Lines<BufReader<R>>,
                ) -> Poll<Option<<BulkInputStream as Stream>::Item>> {
                    pin!(lines.next_line()).poll(cx).map(|line| {
                        line.transpose().map(|l| {
                            l.map_err(Into::into)
                                .and_then(|l| CrateName::from_str(&l).map_err(Into::into))
                        })
                    })
                }

                match self.get_mut() {
                    BulkInputStream::Path(lines) => poll_next_name(cx, lines),
                    BulkInputStream::List(it) => {
                        let it = pin!(it);
                        it.poll_next(cx).map(|n| n.map(Ok))
                    }
                    BulkInputStream::StdIn(lines) => poll_next_name(cx, lines),
                }
            }
        }

        let stream = match self {
            BulkInput::Path(path_buf) => {
                BulkInputStream::Path(BufReader::new(File::open(path_buf).await?).lines())
            }
            BulkInput::List(list) => BulkInputStream::List(stream::iter(list.into_iter())),
            BulkInput::StdIn => BulkInputStream::StdIn(BufReader::new(stdin()).lines()),
        };

        Ok(stream)
    }
}

#[derive(Debug)]
pub struct ParseBulkInputError;

impl std::error::Error for ParseBulkInputError {}

impl fmt::Display for ParseBulkInputError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        "Error parsing bulk input spec. Expecting either a path, a comma-separated list, or '-' (indicating stdin)".fmt(f)
    }
}

impl TryFrom<&str> for BulkInput {
    type Error = <Self as FromStr>::Err;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        s.parse()
    }
}

impl FromStr for BulkInput {
    type Err = ParseBulkInputError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.is_empty() {
            return Err(ParseBulkInputError);
        }

        if let "-" = s {
            return Ok(Self::StdIn);
        };

        if let Ok(list) = s.split(',').try_fold(vec![], |mut res, name| {
            res.push(name.parse()?);
            Ok::<_, <CrateName as FromStr>::Err>(res)
        }) {
            return Ok(Self::List(list));
        }

        Ok(Self::Path(s.into()))
    }
}
