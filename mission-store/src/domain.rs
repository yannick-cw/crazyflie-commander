use crate::domain::Error::ParseError;
use chrono::{DateTime, Utc};
use drone_control::Telemetry;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, PartialOrd, Hash, Deserialize, Serialize, sqlx::Type)]
#[sqlx(transparent)]
#[serde(try_from = "String")]
pub struct ValidName(String);

impl ValidName {
    pub fn parse(s: String) -> Res<ValidName> {
        let empty = s.trim().is_empty();
        let too_long = s.len() > 200;
        let contains_weirdness = s.chars().any(|c| ['\\', '%', '<', '>'].contains(&c));

        if empty || too_long || contains_weirdness {
            Err(ParseError(format!("name `{}`", s)))
        } else {
            Ok(Self(s))
        }
    }
    pub fn inner(self) -> String {
        self.0
    }
}

impl AsRef<String> for ValidName {
    fn as_ref(&self) -> &String {
        &self.0
    }
}

impl TryFrom<String> for ValidName {
    type Error = Error;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        ValidName::parse(value)
    }
}

#[derive(Debug, Default, Clone, PartialEq, PartialOrd, Deserialize, Serialize)]
pub struct Flight {
    pub date: DateTime<Utc>,
    pub telemetry: Vec<Telemetry>,
    pub mission: Option<ValidName>,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Failed parsing: ")]
    ParseError(String),
}
/// Result type for interacting with this crate.
pub type Res<A> = Result<A, Error>;

#[cfg(test)]
mod tests {
    use super::*;
    use fake::faker::name::raw::Name;
    use fake::{Dummy, Fake, Faker, locales};
    use quickcheck::{Arbitrary, Gen};
    use quickcheck_macros::quickcheck;
    use rand::SeedableRng;
    use rand::rngs::StdRng;
    use std::ops::Deref;

    // small custom helper to lift faked values into `Arbitrary`
    #[derive(Clone, Debug)]
    struct Faked<T>(T);
    impl<T> Deref for Faked<T> {
        type Target = T;
        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }

    impl<T: Dummy<Faker> + Clone + 'static> Arbitrary for Faked<T> {
        fn arbitrary(g: &mut Gen) -> Self {
            Faked(Faker.fake_with_rng(&mut StdRng::seed_from_u64(u64::arbitrary(g))))
        }
    }

    #[derive(Clone, Debug, Dummy)]
    struct FakeName(#[dummy(faker = "Name(locales::EN)")] String);

    #[quickcheck]
    fn prop(name: Faked<FakeName>) -> bool {
        ValidName::parse(name.0.0).is_ok()
    }
}
