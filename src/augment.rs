use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    path::Path,
};

use db_dump::{crate_owners::OwnerId, crates::CrateId, versions::VersionId};

use crate::{
    convert::{CrateData, CrateOwner},
    error::Error,
    spec::CrateName,
};

#[derive(Debug, Hash)]
struct DbDumpCrateData {
    description: String,
    owners: Vec<OwnerId>,
}

#[derive(Debug, Hash)]
struct DbDumpCrateOwnerData {
    avatar: String,
}

#[derive(Debug)]
pub struct CrateDb {
    crates: HashMap<CrateId, DbDumpCrateData>,
    crate_names: HashMap<String, CrateId>,
    crate_owners: HashMap<OwnerId, Option<DbDumpCrateOwnerData>>,
    crate_default_versions: HashMap<CrateId, Option<VersionId>>,
    version_licenses: HashMap<(CrateId, VersionId), String>,
}

pub enum LoadFilter {
    Select(HashSet<String>),
    Single(String),
    All,
}

impl LoadFilter {
    fn matches(&self, name: &str) -> bool {
        match self {
            LoadFilter::All => true,
            LoadFilter::Single(item) => item == name,
            LoadFilter::Select(items) => items.contains(name),
        }
    }
}

impl CrateDb {
    fn load_with_filter_blocking(
        dump_path: impl AsRef<Path>,
        filter: LoadFilter,
    ) -> Result<Self, Error> {
        let crates = RefCell::new(HashMap::new());
        let crate_owners = RefCell::new(HashMap::new());
        let crate_default_versions = RefCell::new(HashMap::new());
        let mut version_licenses = HashMap::new();
        let mut crate_names = HashMap::new();
        {
            // Sadly, the order in which the CSVs are loaded is non-deterministic,
            // but in order to save memory, we'll only want to load data that
            // matches the filter. Luckily, `db_dump::Loader` will skip
            // CSVs that are not requested, and thus won't iterate over
            // a CSV more than once, but the archive does need to be inflated
            // multiple times.

            let mut loader = db_dump::Loader::new();
            loader.crates(|c| {
                if !filter.matches(&c.name) {
                    return;
                }
                let data = DbDumpCrateData {
                    description: c.description,
                    owners: vec![],
                };
                crates.borrow_mut().insert(c.id, data);
                crate_default_versions.borrow_mut().insert(c.id, None);
                crate_names.insert(c.name, c.id);
            });
            loader.load(&dump_path)?;

            let mut loader = db_dump::Loader::new();
            loader.crate_owners(|co| {
                crates.borrow_mut().entry(co.crate_id).and_modify(|c| {
                    crate_owners.borrow_mut().insert(co.owner_id, None);
                    c.owners.push(co.owner_id);
                });
            });
            loader.load(&dump_path)?;

            let mut loader = db_dump::Loader::new();
            loader.teams(|t| {
                crate_owners
                    .borrow_mut()
                    .entry(OwnerId::Team(t.id))
                    .and_modify(|co| *co = Some(DbDumpCrateOwnerData { avatar: t.avatar }));
            });
            loader.load(&dump_path)?;

            let mut loader = db_dump::Loader::new();
            loader.users(|u| {
                crate_owners
                    .borrow_mut()
                    .entry(OwnerId::User(u.id))
                    .and_modify(|co| {
                        *co = Some(DbDumpCrateOwnerData {
                            avatar: u.gh_avatar,
                        })
                    });
            });
            loader.load(&dump_path)?;

            let mut loader = db_dump::Loader::new();
            loader.default_versions(|dv| {
                crate_default_versions
                    .borrow_mut()
                    .entry(dv.crate_id)
                    .and_modify(|v| *v = Some(dv.version_id));
            });
            loader.load(&dump_path)?;

            let mut loader = db_dump::Loader::new();
            loader.versions(|v| {
                if let Some(cid) = crate_default_versions.borrow().get(&v.crate_id) {
                    let (cid, vid) = match cid.as_ref() {
                        Some(vid) if *vid == v.id => (v.crate_id, v.id),
                        _ => return,
                    };
                    version_licenses.insert((cid, vid), v.license);
                }
            });
            loader.load(&dump_path)?;
        }

        let crates = crates.into_inner();
        let crate_owners = crate_owners.into_inner();
        let crate_default_versions = crate_default_versions.into_inner();

        Ok(Self {
            crates,
            crate_names,
            crate_owners,
            crate_default_versions,
            version_licenses,
        })
    }

    pub async fn preload_all(dump_path: impl AsRef<Path> + Send + 'static) -> Result<Self, Error> {
        tokio::task::spawn_blocking(|| Self::load_with_filter_blocking(dump_path, LoadFilter::All))
            .await
            .unwrap()
    }

    pub async fn preload_many(
        dump_path: impl AsRef<Path> + Send + 'static,
        items: HashSet<String>,
    ) -> Result<Self, Error> {
        tokio::task::spawn_blocking(|| {
            Self::load_with_filter_blocking(dump_path, LoadFilter::Select(items))
        })
        .await
        .unwrap()
    }

    pub async fn preload_one(
        dump_path: impl AsRef<Path> + Send + 'static,
        item: String,
    ) -> Result<Self, Error> {
        tokio::task::spawn_blocking(|| {
            Self::load_with_filter_blocking(dump_path, LoadFilter::Single(item))
        })
        .await
        .unwrap()
    }

    pub fn augment_crate_spec(&self, name: CrateName) -> Result<CrateData, Error> {
        let id = self.crate_names.get(name.as_ref()).ok_or(Error::NotFound)?;
        let data = &self.crates[id];

        let default_version =
            &self.crate_default_versions[id].expect("Every crate has a default version");
        let license = self.version_licenses[&(*id, *default_version)]
            .as_str()
            .into();

        let owners = data
            .owners
            .iter()
            .flat_map(|o| self.crate_owners[o].iter())
            .map(|DbDumpCrateOwnerData { avatar }| CrateOwner {
                avatar: avatar.clone().into(),
            })
            .collect();

        Ok(CrateData {
            name,
            description: data.description.clone().into(),
            owners,
            license,
        })
    }

    /// Returns an iterator over all preloaded crates, augmented
    pub fn augment_preloaded(&self) -> impl Iterator<Item = CrateData> + '_ {
        self.crate_names
            .keys()
            .map(|k| self.augment_crate_spec(k.parse().unwrap()).unwrap())
    }
}
