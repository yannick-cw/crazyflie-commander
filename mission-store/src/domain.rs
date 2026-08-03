use crate::domain::Error::ParseError;
use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, PartialOrd, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct MissionName(String);

impl MissionName {
    pub fn parse(s: String) -> Res<MissionName> {
        let empty = s.trim().is_empty();
        let too_long = s.len() > 200;
        let contains_weirdness = s.chars().any(|c| ['\\', '%', '<', '>'].contains(&c));

        if empty || too_long || contains_weirdness {
            Err(ParseError(format!("name `{}`", s)))
        } else {
            Ok(Self(s))
        }
    }
}

impl AsRef<String> for MissionName {
    fn as_ref(&self) -> &String {
        &self.0
    }
}

impl TryFrom<String> for MissionName {
    type Error = Error;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        MissionName::parse(value)
    }
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
    struct ValidName(#[dummy(faker = "Name(locales::EN)")] String);

    #[quickcheck]
    fn prop(name: Faked<ValidName>) -> bool {
        MissionName::parse(name.0.0).is_ok()
    }
}
