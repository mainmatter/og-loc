use std::sync::{Arc, LazyLock};

use aho_corasick::AhoCorasick;
use dashmap::DashMap;
use minijinja::{context, Environment};
use typst::{
    diag::{FileError, FileResult, Warned},
    foundations::{Bytes, Datetime},
    syntax::{FileId, Source, VirtualPath},
    text::{Font, FontBook},
    utils::LazyHash,
    Library,
};
use typst_kit::fonts::{FontSlot, Fonts};

use crate::{spec::CrateName, HTTP_CLIENT};

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

#[derive(Debug, PartialEq, Eq, Clone, serde::Serialize)]
/// Crate data used for rendering the Jinja2 template
/// to Typst source.
pub struct CrateData {
    /// The name of the crate
    pub name: CrateName,
    /// The crate's description
    pub description: TypstString,
    /// The team owners of the crate
    pub team_owners: Vec<TeamCrateOwner>,
    /// The user owners of the crate
    pub user_owners: Vec<UserCrateOwner>,
}

#[derive(Debug, PartialEq, Eq, Clone, serde::Serialize)]
/// A team crate owner
pub struct TeamCrateOwner {
    /// URL of the owner's avatar image
    pub avatar: TypstString,
}

#[derive(Debug, PartialEq, Eq, Clone, serde::Serialize)]
/// A user crate owner
pub struct UserCrateOwner {
    /// URL of the owner's avatar image
    pub avatar: TypstString,
}

#[derive(Debug, Clone, serde::Serialize, PartialEq, Eq, Hash)]
/// A string that is safe to use directly in typst source code.
pub struct TypstString(String);

impl From<&str> for TypstString {
    fn from(s: &str) -> Self {
        const NUM_REPLACE_ITEMS: usize = 5;
        const REPLACE: [(&str, &str); NUM_REPLACE_ITEMS] = [
            // TODO figure out whether this list is exhaustive and correct
            ("#", r"\#"),
            (r"\", r"\\"),
            ("^", r"\^"),
            ("$", r"\$"),
            (r#"""#, r#"\""#),
        ];

        // Alas, <[N;T]>::map is not const, so instead we have to do this
        static PATTERNS: LazyLock<[&str; NUM_REPLACE_ITEMS]> =
            LazyLock::new(|| REPLACE.map(|(p, _)| p));
        static ESCAPED: LazyLock<[&str; NUM_REPLACE_ITEMS]> =
            LazyLock::new(|| REPLACE.map(|(_, e)| e));
        static MATCHER: LazyLock<AhoCorasick> =
            LazyLock::new(|| AhoCorasick::new(PATTERNS.iter()).expect("Error setting up matcher"));

        Self(MATCHER.replace_all(s, &*ESCAPED))
    }
}

impl From<String> for TypstString {
    fn from(s: String) -> Self {
        s.as_str().into()
    }
}

impl CrateData {
    fn render_as_typst_source(&self) -> String {
        let template = TEMPLATE_ENV.get_template(OG_TEMPLATE_NAME).unwrap();
        template
            .render(context! {
                krate => self
            })
            .expect("Error rendering Jinja2 template")
    }

    /// Render a PNG for this [`CrateData`] using [`typst`].
    pub async fn render_as_png(self) -> Vec<u8> {
        tokio::task::spawn_blocking(move || {
            let typ = self.render_as_typst_source();
            let world = OgTypstWorld::new(typ.clone());
            let Warned { output, warnings } = typst::compile(&world);
            if !warnings.is_empty() {
                panic!("{warnings:?}");
            }
            let output = output.unwrap_or_else(|e| {
                e.into_iter().for_each(|e| {
                    eprintln!("Error rendering image for crate {}: {e:?}", self.name)
                });
                eprintln!("Source:");
                eprintln!("================");
                eprintln!("{typ}");
                eprintln!("================");
                std::process::exit(-1);
            });

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
/// *To be used only in `tokio` context*
struct OgTypstWorld {
    shared: Arc<OgTypstWorldShared>,
    source: Source,
}

struct OgTypstWorldShared {
    library: LazyHash<Library>,
    book: LazyHash<FontBook>,
    fonts: Vec<FontSlot>,
    // TODO replace this with a moka cache
    avatars: DashMap<FileId, Option<Bytes>>,
}

impl OgTypstWorld {
    fn new(source: String) -> Self {
        static SHARED: LazyLock<Arc<OgTypstWorldShared>> = LazyLock::new(|| {
            let fonts = Fonts::searcher().search();
            let shared = OgTypstWorldShared {
                library: LazyHash::new(Library::default()),
                book: LazyHash::new(fonts.book),
                avatars: DashMap::new(),
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

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        if id.vpath() == &VirtualPath::new("/cargo.png") {
            return Ok(Bytes::from_static(include_bytes!("../cargo.png")));
        }

        self.shared
            .avatars
            .entry(id)
            .or_insert_with(|| {
                tokio::runtime::Handle::current().block_on(async {
                    // TODO parse and validate URL
                    let url = id.vpath().as_rootless_path().to_str()?;
                    let body = HTTP_CLIENT
                        .get(url)
                        .send()
                        .await
                        .ok()?
                        .error_for_status()
                        .ok()?
                        .bytes()
                        .await
                        .ok()?;
                    Some(Bytes::from(body.to_vec()))
                })
            })
            .clone()
            .ok_or(FileError::Other(None))
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
    use std::sync::LazyLock;

    use crate::{augment::CrateDb, convert::UserCrateOwner};

    use super::CrateData;

    static KNIEN_CRATE_DATA: LazyLock<CrateData> = LazyLock::new(|| CrateData {
        name: "knien".parse().unwrap(),
        description: "Typed RabbitMQ interfacing for async Rust".into(),
        user_owners: vec![
            UserCrateOwner {
                avatar: "https://avatars.githubusercontent.com/u/17907879?v=4&s=70".into(),
            },
            UserCrateOwner {
                avatar: "https://avatars.githubusercontent.com/u/8545127?v=4&s=70".into(),
            },
        ],
        team_owners: vec![],
    });

    #[test]
    fn render_typst_source() {
        let rendered = KNIEN_CRATE_DATA.render_as_typst_source();
        insta::assert_snapshot!(rendered);
    }

    #[tokio::test]
    async fn render_png() {
        let rendered = KNIEN_CRATE_DATA.clone().render_as_png().await;
        insta::assert_binary_snapshot!(".png", rendered);
    }

    #[tokio::test]
    async fn augment_crate_data() {
        let db = CrateDb::preload_one("./db-dump.tar.gz", "knien".into())
            .await
            .unwrap();
        let data = db.augment_crate_spec("knien".parse().unwrap()).unwrap();

        assert_eq!(&data, &*KNIEN_CRATE_DATA);
    }
}
