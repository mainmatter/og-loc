use std::{fmt, str::FromStr};

/// A valid crate name.
#[derive(Clone, Debug, Hash, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(try_from = "&str")]
pub struct CrateName(String);

impl CrateName {
    pub const MAX_LEN: usize = 64;
}

impl fmt::Display for CrateName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// Error caused by attempting to parse an invalid crate
/// name as a [`CrateName`.]
#[derive(Debug, PartialEq, Eq)]
pub struct InvalidCrateName(String);

impl InvalidCrateName {
    fn err_with_msg<T>(msg: impl ToString) -> Result<T, Self> {
        Err(Self(msg.to_string()))
    }
}

impl fmt::Display for InvalidCrateName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Invalid crate name: {}", self.0)
    }
}

impl std::error::Error for InvalidCrateName {}

impl TryFrom<&str> for CrateName {
    type Error = <CrateName as FromStr>::Err;

    fn try_from(name: &str) -> Result<Self, Self::Error> {
        name.parse()
    }
}

impl FromStr for CrateName {
    type Err = InvalidCrateName;

    fn from_str(name: &str) -> Result<Self, Self::Err> {
        // Implements the rules from https://doc.rust-lang.org/cargo/reference/manifest.html#the-name-field
        // TODO:
        // - 'Note that cargo new and cargo init impose some additional restrictions on the package name, such as enforcing that it is a valid Rust identifier and not a keyword.'
        // - 'Do not use reserved names.' => Figure out what 'reserved names' are and complete the checl
        // - 'Do not use special Windows names such as “nul”.' => Complete the check
        if name.is_empty() {
            return InvalidCrateName::err_with_msg("Crate names cannot be empty");
        }
        if name.len() > Self::MAX_LEN {
            return InvalidCrateName::err_with_msg(
                "Crate names can not be longer than 64 characters",
            );
        }

        if !name.is_ascii() {
            return InvalidCrateName::err_with_msg("Crate names must be ASCII");
        }
        if !name
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, '-' | '_'))
        {
            return InvalidCrateName::err_with_msg(
                "Crate names must use only alphanumeric characters or `-` or `_`",
            );
        }
        if name.contains("nul") {
            return InvalidCrateName::err_with_msg("Crate names cannot use special Windows names");
        }
        Ok(CrateName(name.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use test_case::case;

    use super::{CrateName, InvalidCrateName};

    #[case("" => InvalidCrateName::err_with_msg("Crate names cannot be empty"))]
    #[case("aksajdkajhdskjashdkjahdkajshdkajshdklajhdlkjashdkjadkjadkashdakdkajshda" => InvalidCrateName::err_with_msg("Crate names can not be longer than 64 characters"))]
    #[case("🤡_test" => InvalidCrateName::err_with_msg("Crate names must be ASCII"))]
    #[case("test@123" => InvalidCrateName::err_with_msg(
        "Crate names must use only alphanumeric characters or `-` or `_`",
    ))]
    #[case("onbenullig" => InvalidCrateName::err_with_msg("Crate names cannot use special Windows names"))]
    #[case("og-loc" => Ok(CrateName("og-loc".to_string())))]
    fn test_crate_name_validation(name: &str) -> Result<CrateName, InvalidCrateName> {
        name.parse()
    }
}
