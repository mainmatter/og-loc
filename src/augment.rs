use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    path::Path,
};

use db_dump::{crate_owners::OwnerId, crates::CrateId, teams::TeamId, users::UserId};

use crate::{
    convert::{CrateData, TeamCrateOwner, UserCrateOwner},
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
    user_crate_owners: HashMap<UserId, Option<DbDumpCrateOwnerData>>,
    team_crate_owners: HashMap<TeamId, Option<DbDumpCrateOwnerData>>,
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
                // Cut off description if it's too long.
                // Sadly typst doesn't seem to provide a nice
                // way to do this.
                let mut description = c.description;
                if let Some((idx, _)) = description.char_indices().nth(110) {
                    let idx = description[..idx]
                        .rfind([' ', ',', '.', ';', '!', '?'])
                        .unwrap_or(idx);
                    description.truncate(idx);
                    description.push('…');
                };
                let description = description.trim().replace(|c: char| c.is_whitespace(), " ");

                let data = DbDumpCrateData {
                    description,
                    owners: vec![],
                };
                crates.borrow_mut().insert(c.id, data);
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
        }

        let crates = crates.into_inner();
        let crate_owners = crate_owners.into_inner();

        let (user_crate_owners, team_crate_owners) = crate_owners
            .into_iter()
            .partition::<HashMap<_, _>, _>(|(k, _)| matches!(k, OwnerId::User(_)));
        let user_crate_owners = user_crate_owners
            .into_iter()
            .map(|(k, v)| {
                let OwnerId::User(k) = k else {
                    unreachable!();
                };
                (k, v)
            })
            .collect();

        let team_crate_owners = team_crate_owners
            .into_iter()
            .map(|(k, v)| {
                let OwnerId::Team(k) = k else {
                    unreachable!();
                };
                (k, v)
            })
            .collect();

        Ok(Self {
            crates,
            crate_names,
            // crate_owners,
            user_crate_owners,
            team_crate_owners,
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

        let user_owners = data
            .owners
            .iter()
            .filter_map(|oid| match oid {
                OwnerId::User(uid) => Some(uid),
                OwnerId::Team(_) => None,
            })
            .flat_map(|uid| self.user_crate_owners[uid].iter())
            .map(|DbDumpCrateOwnerData { avatar }| UserCrateOwner {
                avatar: format!("{avatar}&s=70").into(),
            })
            .take(3)
            .collect();

        let team_owners = data
            .owners
            .iter()
            .filter_map(|oid| match oid {
                OwnerId::Team(tid) => Some(tid),
                OwnerId::User(_) => None,
            })
            .flat_map(|tid| self.team_crate_owners[tid].iter())
            .map(|DbDumpCrateOwnerData { avatar }| TeamCrateOwner {
                avatar: format!("{avatar}&s=70").into(),
            })
            .take(3)
            .collect();

        Ok(CrateData {
            name,
            description: data.description.clone().into(),
            user_owners,
            team_owners,
        })
    }

    /// Returns an iterator over all preloaded crates, augmented
    pub fn augment_preloaded(&self) -> impl Iterator<Item = CrateData> + '_ {
        self.crate_names
            .keys()
            .map(|k| self.augment_crate_spec(k.parse().unwrap()).unwrap())
    }
}
