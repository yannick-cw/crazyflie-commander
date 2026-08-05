use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use chrono::{DateTime, Utc};
use drone_control::{Command, Telemetry};
use serde::{Deserialize, Serialize};
use std::ops::Deref;
use tracing::{error, warn};

#[derive(Debug, Clone, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct MissionResponse {
    pub name: ValidName,
    pub mission: Vec<Command>,
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Hash, Deserialize, Serialize, sqlx::Type)]
#[sqlx(transparent)]
#[serde(try_from = "String")]
pub struct ValidName(String);

impl Deref for ValidName {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        self.as_ref()
    }
}

impl ValidName {
    pub fn parse(s: String) -> Result<ValidName, String> {
        let empty = s.trim().is_empty();
        let too_long = s.len() > 200;
        let contains_weirdness = s.chars().any(|c| ['\\', '%', '<', '>'].contains(&c));

        if empty || too_long || contains_weirdness {
            Err(format!("name `{}`", s))
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
    type Error = String;

    fn try_from(value: String) -> Result<Self, String> {
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
    #[error("{0}")]
    ValidationError(String),
    #[error("Did not find: {0}")]
    NotFound(String),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}
impl IntoResponse for Error {
    fn into_response(self) -> Response {
        match self {
            Error::ValidationError(validation) => {
                warn!("Failed validation {:#}", validation);
                (StatusCode::BAD_REQUEST, validation).into_response()
            }
            Error::NotFound(missing) => {
                (StatusCode::NOT_FOUND, format!("Did not find `{}`", missing)).into_response()
            }
            Error::UnexpectedError(_) => {
                error!("{:#}", self);
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
        }
    }
}

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
