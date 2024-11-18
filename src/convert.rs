use std::sync::{Arc, LazyLock};

use minijinja::{context, Environment};
use typst::{
    diag::{FileError, FileResult, Warned},
    foundations::{Bytes, Datetime},
    syntax::{FileId, Source},
    text::{Font, FontBook},
    utils::LazyHash,
    Library,
};
use typst_kit::fonts::{FontSlot, Fonts};

use crate::{error::Error, spec::CrateName};

/// Identifier for the Open Graph template in the
/// [`minijinja::Environment`]
const OG_TEMPLATE_NAME: &str = "og-typst";

/// Set up the [`minijinja::Environment`] for rendering the
/// Jinja2 template to Typst source.
static TEMPLATE_ENV: LazyLock<minijinja::Environment> = LazyLock::new(|| {
    const OG_TEMPLATE_J2: &str = include_str!("../template.typ.j2");
    let mut env = Environment::new();
    env.add_template(OG_TEMPLATE_NAME, OG_TEMPLATE_J2).unwrap();
    env
});

/// Set up a reusable HTTP client with a User Agent
/// that allows for identifying this application.
static HTTP_CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    const CARGO_PKG_NAME: &str = env!("CARGO_PKG_NAME");
    const CARGO_PKG_VERSION: &str = env!("CARGO_PKG_VERSION");
    const CARGO_PKG_REPOSITORY: &str = env!("CARGO_PKG_REPOSITORY");

    reqwest::ClientBuilder::new()
        .user_agent(format!(
            "{CARGO_PKG_NAME}/{CARGO_PKG_VERSION} ({CARGO_PKG_REPOSITORY})"
        ))
        .build()
        .unwrap()
});

#[derive(Debug, serde::Serialize)]
/// Crate data used for rendering the Jinja2 template
/// to Typst source.
pub struct CrateData {
    /// The name of the crate
    pub name: CrateName,
    /// The crate's description
    pub description: String,
}

impl CrateData {
    /// Augment a [`CrateVersionSpec`] to produce a [`CrateData`].
    /// This function performs a HTTP request to the crates.io API,
    /// in order to fetch details such as the crate's description
    /// or the number of downloads for the specified version.
    pub async fn augment_crate_version_spec(name: CrateName) -> Result<Self, Error> {
        // A buch of structs to deserialize
        // the API response into.

        #[derive(Debug, serde::Deserialize)]
        struct CrateDataResponse {
            #[serde(rename = "crate")]
            krate: CrateDef,
        }

        #[derive(Debug, serde::Deserialize)]
        struct CrateDef {
            description: String,
        }

        let url = format!("https://crates.io/api/v1/crates/{}", name);
        let res: CrateDataResponse = HTTP_CLIENT
            .get(url)
            .header(reqwest::header::ACCEPT, "application/json")
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        Ok(CrateData {
            name,
            description: res.krate.description,
        })
    }

    fn render_as_typst_source(&self) -> String {
        let template = TEMPLATE_ENV.get_template(OG_TEMPLATE_NAME).unwrap();
        template
            .render(context! {
                krate => self
            })
            .expect("Error rendering template")
    }

    /// Render a PNG for this [`CrateData`] using [`typst`].
    pub async fn render_as_png(self) -> Vec<u8> {
        tokio::task::spawn_blocking(move || {
            let typ = self.render_as_typst_source();
            let world = OgTypstWorld::new(typ);
            let Warned { output, .. } = typst::compile(&world);
            let output = output.unwrap();
            let page = &output.pages[0];
            let pixmap = typst_render::render(page, 1.);
            pixmap.encode_png().unwrap()
        })
        .await
        .unwrap()
    }
}

/// Simple [`typst::World`] implementation that
/// supports nothing more than what's needed to
/// render the Open Grapth image template.
/// Creating new [`OgTypstWorld`]s is cheap,
/// as any shared resources are kept in a singleton.
struct OgTypstWorld {
    shared: Arc<OgTypstWorldShared>,
    source: Source,
}

struct OgTypstWorldShared {
    library: LazyHash<Library>,
    book: LazyHash<FontBook>,
    fonts: Vec<FontSlot>,
}

impl OgTypstWorld {
    fn new(source: String) -> Self {
        static SHARED: LazyLock<Arc<OgTypstWorldShared>> = LazyLock::new(|| {
            let fonts = Fonts::searcher().search();
            let shared = OgTypstWorldShared {
                library: LazyHash::new(Library::default()),
                book: LazyHash::new(fonts.book),

                fonts: fonts.fonts,
            };
            Arc::new(shared)
        });

        Self {
            source: Source::detached(source),
            shared: SHARED.clone(),
        }
    }
}

impl typst::World for OgTypstWorld {
    fn library(&self) -> &LazyHash<Library> {
        &self.shared.library
    }

    fn book(&self) -> &LazyHash<FontBook> {
        &self.shared.book
    }

    fn main(&self) -> FileId {
        self.source.id()
    }

    fn source(&self, _id: FileId) -> FileResult<Source> {
        Ok(self.source.clone())
    }

    fn file(&self, _id: FileId) -> FileResult<Bytes> {
        Err(FileError::Other(None))
    }

    fn font(&self, index: usize) -> Option<Font> {
        self.shared.fonts[index].get()
    }

    fn today(&self, _offset: Option<i64>) -> Option<Datetime> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::CrateData;

    #[test]
    fn render_typst_source() {
        let data = CrateData {
            name: "knien".parse().unwrap(),
            description: "Typed RabbitMQ interfacing for async Rust".to_string(),
        };

        let rendered = data.render_as_typst_source();
        insta::assert_snapshot!(rendered);
    }

    #[tokio::test]
    async fn render_png() {
        let data = CrateData {
            name: "knien".parse().unwrap(),
            description: "Typed RabbitMQ interfacing for async Rust".to_string(),
        };
        let rendered = data.render_as_png().await;
        insta::assert_binary_snapshot!(".png", rendered);
    }

    #[tokio::test]
    async fn augment_crate_data() {
        let data = CrateData::augment_crate_version_spec("knien".parse().unwrap())
            .await
            .unwrap();

        assert_eq!(data.name, "knien".parse().unwrap());
        assert_eq!(
            data.description,
            "Typed RabbitMQ interfacing for async Rust"
        );
    }
}
