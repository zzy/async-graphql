use once_cell::sync::Lazy;
use regex::Regex;
use serde::de::{Deserializer, Unexpected};

use super::InputValueValidator;

static EMAIL_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new("^(([0-9A-Za-z!#$%&'*+-/=?^_`{|}~&&[^@]]+)|(\"([0-9A-Za-z!#$%&'*+-/=?^_`{|}~ \"(),:;<>@\\[\\\\\\]]+)\"))@").unwrap()
});

/// Email validator
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Email;

impl<'de> InputValueValidator<'de> for Email {
    fn validate<D: Deserializer<'de> + Clone>(&self, deserializer: D) -> Result<(), D::Error> {
        let value = str::deserialize(deserializer)?;

        if !EMAIL_RE.is_match(str::deserialize(deserializer)?) {
            Err(D::Error::invalid_value(Unexpected::Str(value), &"a valid email"))
        } else {
            Ok(())
        }
    }
}

static MAC_ADDRESS_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new("^([0-9a-fA-F]{2}:){5}[0-9a-fA-F]{2}$").unwrap());
static MAC_ADDRESS_NO_COLON_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new("^[0-9a-fA-F]{12}$").unwrap());

/// MAC address validator.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MAC {
    /// Whether the MAC address must include a colon.
    pub colon: bool,
}

impl MAC {
    /// Set whether the MAC address requires a colon.
    pub fn colon(self, colon: bool) -> Self {
        Self {
            colon,
        }
    }
}

impl<'de> InputValueValidator<'de> for MAC {
    fn validate<D: Deserializer<'de> + Clone>(&self, deserializer: D) -> Result<(), D::Error> {
        let value = str::deserialize(deserializer)?;

        let re = if self.colon { &MAC_ADDRESS_RE } else { &MAC_ADDRESS_NO_COLON_RE };

        if !re.is_match(str::deserialize(deserializer)?) {
            Err(D::Error::invalid_value(Unexpected::Str(value), &"a valid MAC address"))
        } else {
            Ok(())
        }
    }
}
