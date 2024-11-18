use std::{fmt, path::PathBuf, pin::pin, str::FromStr, vec};

use futures_lite::{stream, FutureExt, Stream, StreamExt};
use tokio::{
    fs::File,
    io::{self, stdin, AsyncBufReadExt, AsyncWriteExt, BufReader, Lines, Stdin},
};

use crate::{
    convert::CrateData,
    error::Error,
    spec::{CrateName, InvalidCrateName},
    CommonArgs,
};

#[derive(Debug, clap::Args)]
pub struct Bulk {
    /// Force overwrite the output.
    #[arg(env, long, short)]
    pub force: bool,
    /// Input specifier. Either a comma-separated list of crate names, a path to a file containing a newline-separated list of crate names, or `-`, indicating stdin.
    /// Will first attempt to match input with `-`, then parse it as a comma-separated list of crate names, and then fall back to a path, only failing if an empty
    /// value is passed.
    #[arg(env, long = "input", short)]
    pub input: BulkInput,
    /// The path of the folder to which the PNGs should be written
    #[arg(env, long = "out", short)]
    pub out_folder: PathBuf,
}

impl Bulk {
    pub async fn run(self, _common: CommonArgs) -> Result<(), Error> {
        let mut stream = self.input.as_stream().await?;
        tokio::fs::create_dir_all(&self.out_folder).await?;

        while let Some(krate) = stream.next().await {
            let crate_name = krate?;
            let image_file_name = format!("{crate_name}.png");
            let path = self.out_folder.join(image_file_name);

            tokio::spawn(async move {
                let data = CrateData::augment_crate_version_spec(crate_name).await?;
                let png = data.render_as_png();

                let mut file = if self.force {
                    tokio::fs::File::create(path).await?
                } else {
                    tokio::fs::File::create_new(path).await?
                };

                file.write_all(&png).await?;
                Ok::<_, Error>(())
            });
        }
        Ok(())
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
    pub async fn as_stream(
        self,
    ) -> Result<impl Stream<Item = Result<CrateName, BulkInputError>>, io::Error> {
        enum InputStream {
            Path(Lines<BufReader<File>>),
            List(stream::Iter<vec::IntoIter<CrateName>>),
            StdIn(Lines<BufReader<Stdin>>),
        }

        impl Stream for InputStream {
            type Item = Result<CrateName, BulkInputError>;

            fn poll_next(
                self: std::pin::Pin<&mut Self>,
                cx: &mut std::task::Context<'_>,
            ) -> std::task::Poll<Option<Self::Item>> {
                match self.get_mut() {
                    InputStream::Path(lines) => pin!(lines.next_line()).poll(cx).map(|line| {
                        line.transpose().map(|l| {
                            l.map_err(Into::into)
                                .and_then(|l| CrateName::from_str(&l).map_err(Into::into))
                        })
                    }),
                    InputStream::List(it) => {
                        let it = pin!(it);
                        it.poll_next(cx).map(|n| n.map(Ok))
                    }
                    InputStream::StdIn(lines) => pin!(lines.next_line()).poll(cx).map(|line| {
                        line.transpose().map(|l| {
                            l.map_err(Into::into)
                                .and_then(|l| CrateName::from_str(&l).map_err(Into::into))
                        })
                    }),
                }
            }
        }

        let stream = match self {
            BulkInput::Path(path_buf) => {
                InputStream::Path(BufReader::new(File::open(path_buf).await?).lines())
            }
            BulkInput::List(list) => InputStream::List(stream::iter(list.into_iter())),
            BulkInput::StdIn => InputStream::StdIn(BufReader::new(stdin()).lines()),
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
