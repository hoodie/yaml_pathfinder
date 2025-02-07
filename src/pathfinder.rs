#[cfg(feature = "date_parsing")]
use chrono::prelude::*;

use yaml_rust::{yaml::Array as YamlArray, yaml::Hash as YamlHash, Yaml};

#[cfg(feature = "date_parsing")]
use crate::util::parse_dmy_date;

pub use crate::error::{FieldError, FieldResult};
pub use crate::path::*;

/// Enables access to structured data via a simple path
///
/// A path can be something like `users/clients/23/name`
/// but also  `users.clients.23.name`
pub trait PathFinder {
    /// You only need to implement this.
    fn data(&self) -> &Yaml;

    /// Wrapper around `get_path()`.
    ///
    /// Splits path string
    /// and replaces `Yaml::Null` and `Yaml::BadValue`.
    fn get<'a>(&'a self, paths: &YPaths) -> Option<&'a Yaml> {
        paths
            .alternatives()
            .filter_map(|path| self.get_direct(self.data(), &path))
            .nth(0)
    }

    /// Wrapper around `get_path()`.
    ///
    /// Splits path string
    /// and replaces `Yaml::Null` and `Yaml::BadValue`.
    fn get_direct<'a>(&'a self, data: &'a Yaml, path: &YPath) -> Option<&'a Yaml> {
        // TODO: this can be without copying
        // debug_assert!(
        //     !path.chars().any(char::is_whitespace),
        //     "paths shouldn't contain whitespaces {:?}",
        //     path
        // );
        let elements: Vec<&str> = path.elements().collect();
        match self.get_path(data, &elements) {
            Some(&Yaml::BadValue) | Some(&Yaml::Null) => None,
            content => content,
        }
    }

    /// Returns content at `path` in the yaml document.
    /// TODO: make this generic over the type of data to support more than just `Yaml`.
    fn get_path<'a>(&'a self, data: &'a Yaml, path: &[&str]) -> Option<&'a Yaml> {
        if let Some((&path, remainder)) = path.split_first() {
            match *data {
                // go further into the rabbit hole
                Yaml::Hash(ref hash) => {
                    if remainder.is_empty() {
                        hash.get(&Yaml::String(path.to_owned()))
                    } else {
                        hash.get(&Yaml::String(path.to_owned()))
                            .and_then(|c| self.get_path(c, remainder))
                    }
                }
                // interpret component as index
                Yaml::Array(ref vec) => {
                    if let Ok(index) = path.parse::<usize>() {
                        if remainder.is_empty() {
                            vec.get(index)
                        } else {
                            vec.get(index).and_then(|c| self.get_path(c, remainder))
                        }
                    } else {
                        None
                    }
                }
                // return none, because the path is longer than the data structure
                _ => None,
            }
        } else {
            None
        }
    }

    /// Gets the field for a given path.
    fn field<'a, T, F, I: Into<YPaths<'a>>>(
        &'a self,
        path: I,
        err: &str,
        parser: F,
    ) -> FieldResult<T>
    where
        F: FnOnce(&'a Yaml) -> Option<T>,
    {
        let res = self.get(&path.into());
        match res {
            None => Err(FieldError::Missing),
            Some(ref node) => match parser(node) {
                None => Err(FieldError::Invalid(format!("{} ({:?})", err, node))),
                Some(parsed) => FieldResult::Ok(parsed),
            },
        }
    }

    /// Gets a `&str` value.
    ///
    /// Same mentality as `yaml_rust`, only returns `Some`, if it's a `Yaml::String`.
    fn get_str<'a, I: Into<YPaths<'a>>>(&'a self, path: I) -> FieldResult<&'a str> {
        self.field(path, "not a string", Yaml::as_str)
    }

    /// Gets a `&str` value.
    ///
    /// Same mentality as `yaml_rust`, only returns `Some`, if it's a `Yaml::String`.
    fn get_string<'a, I: Into<YPaths<'a>>>(&'a self, path: I) -> FieldResult<String> {
        self.field(path, "not a string", Yaml::as_str)
            .map(Into::into)
    }

    /// Gets an `Int` value.
    ///
    /// Same mentality as `yaml_rust`, only returns `Some`, if it's a `Yaml::Int`.
    fn get_int<'a, I: Into<YPaths<'a>>>(&'a self, path: I) -> FieldResult<i64> {
        self.field(path, "not an integer", Yaml::as_i64)
    }

    /// Gets a Date in `dd.mm.YYYY` format.
    #[cfg(feature = "date_parsing")]
    fn get_dmy<'a, I: Into<YPaths<'a>>>(&'a self, path: I) -> FieldResult<Date<Utc>> {
        self.field(path, "not a date", |x| x.as_str().and_then(parse_dmy_date))
    }

    /// Gets a `Bool` value.
    ///
    /// **Careful** this is a bit sweeter then ordinary `YAML1.2`,
    /// this will interpret `"yes"` and `"no"` as booleans, similar to `YAML1.1`.
    /// Actually it will interpret any string but `"yes"` als `false`.
    fn get_bool<'a, I: Into<YPaths<'a>>>(&'a self, path: I) -> FieldResult<bool> {
        self.field(path, "not a boolean", |y| {
            y.as_bool()
                // allowing it to be a str: "yes" or "no"
                .or_else(|| {
                    y.as_str()
                        .map(|yes_or_no| match yes_or_no.to_lowercase().as_ref() {
                            "yes" => true,
                            _ => false,
                        })
                })
        })
    }

    /// Get as `Bool` value.
    fn get_bool_strict<'a, I: Into<YPaths<'a>>>(&'a self, path: I) -> FieldResult<bool> {
        self.field(path, "not a boolean", |y| y.as_bool())
    }

    /// Get as `Yaml::Hash`
    fn get_hash<'a, I: Into<YPaths<'a>>>(&'a self, path: I) -> FieldResult<&'a YamlHash> {
        self.field(path, "not a hash", Yaml::as_hash)
    }

    /// Get as `Yaml::Array`
    fn get_vec<'a, I: Into<YPaths<'a>>>(&'a self, path: I) -> FieldResult<&'a YamlArray> {
        self.field(path, "not a vector", Yaml::as_vec)
    }

    /// Gets a `Float` value.
    ///
    /// Also takes a `Yaml::I64` and reinterprets it.
    fn get_f64<'a, I: Into<YPaths<'a>>>(&'a self, path: I) -> FieldResult<f64> {
        self.field(path, "not a float", |y| {
            y.as_f64().or_else(|| y.as_i64().map(|y| y as f64))
        })
    }
}

impl PathFinder for yaml_rust::Yaml {
    fn data(&self) -> &yaml_rust::Yaml {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::parse;

    struct TestProvider {
        yaml: Yaml,
    }

    impl TestProvider {
        pub fn parse(src: &str) -> Self {
            Self {
                yaml: parse(src).unwrap(),
            }
        }
    }

    impl PathFinder for TestProvider {
        fn data(&self) -> &Yaml {
            &self.yaml
        }
    }

    static NO_FALLBACK_PATH: &'static str = r#"
    offer:
        date: 07.11.2019
    "#;

    static FALLBACK_PATH: &'static str = r#"
    offer_date: 08.11.2019
    "#;

    #[test]
    fn find_fallback_paths() {
        let no_fallback = TestProvider::parse(NO_FALLBACK_PATH);
        let fallback = TestProvider::parse(FALLBACK_PATH);

        assert_eq!(
            no_fallback.get_str("offer.date|offer_date"),
            FieldResult::Ok("07.11.2019")
        );

        assert_eq!(
            fallback.get_str("offer.date|offer_date"),
            FieldResult::Ok("08.11.2019")
        );

        assert_eq!(
            no_fallback.get_str("offer.date"),
            FieldResult::Ok("07.11.2019")
        );

        assert_eq!(
            fallback.get_str("offer_date"),
            FieldResult::Ok("08.11.2019")
        );

        assert_eq!(
            no_fallback.get_str("offer_date"),
            FieldResult::Err(FieldError::Missing)
        );

        assert_eq!(
            fallback.get_str("offer.date"),
            FieldResult::Err(FieldError::Missing)
        );
    }

    #[test]
    #[should_panic]
    fn paths_forbid_whitespaces() {
        let fallback = TestProvider::parse(FALLBACK_PATH);
        assert_eq!(
            fallback.get_str("offer.date | offer_date"),
            FieldResult::Ok("08.11.2019")
        );
    }
}
