use std::path::PathBuf;

use tokio::io::AsyncWriteExt;

use crate::{augment::CrateDb, error::Error, spec::CrateName, CommonArgs};

#[derive(Debug, clap::Args)]
pub struct OneShot {
    /// The name of the crate
    #[arg(env, long, short)]
    pub name: CrateName,
    /// The path to the PNG output file
    #[arg(env, long = "out", short)]
    pub out_path: PathBuf,
}

impl OneShot {
    pub async fn run(self, common: CommonArgs) -> Result<(), Error> {
        let db = CrateDb::preload_one(common.db_dump_path, self.name.inner().clone()).await?;
        let data = db.augment_crate_spec(self.name)?;
        let png = data.render_as_png().await;
        let mut out_file = tokio::fs::File::create(self.out_path).await?;
        out_file.write_all(&png).await?;

        Ok(())
    }
}
