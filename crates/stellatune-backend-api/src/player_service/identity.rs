use std::num::NonZeroU64;

use super::error::PlayerServiceError;

pub(super) const MAX_PROVIDER_ID_BYTES: usize = 255;
const MAX_PROVIDER_TEXT_KEY_BYTES: usize = 512;
macro_rules! persistent_id {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(NonZeroU64);

        impl $name {
            pub fn new(value: u64) -> Result<Self, PlayerServiceError> {
                if value == 0 || value > i64::MAX as u64 {
                    return Err(PlayerServiceError::InvalidIdentity {
                        identity: stringify!($name),
                        value,
                    });
                }
                Ok(Self(NonZeroU64::new(value).expect("non-zero validated")))
            }

            pub const fn get(self) -> u64 {
                self.0.get()
            }

            pub(super) fn as_i64(self) -> i64 {
                self.get() as i64
            }
        }
    };
}

persistent_id!(SourceInstanceId);
persistent_id!(TrackId);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceKind {
    LocalLibrary,
    Plugin,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProviderId(String);

impl ProviderId {
    pub fn new(value: impl Into<String>) -> Result<Self, PlayerServiceError> {
        let value = value.into();
        let normalized = value.trim();
        if normalized.is_empty() || normalized.len() > MAX_PROVIDER_ID_BYTES {
            return Err(PlayerServiceError::InvalidProviderId);
        }
        Ok(Self(normalized.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceBinding {
    LocalLibrary,
    Plugin { provider_id: ProviderId },
}

impl SourceBinding {
    pub fn kind(&self) -> SourceKind {
        match self {
            Self::LocalLibrary => SourceKind::LocalLibrary,
            Self::Plugin { .. } => SourceKind::Plugin,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ProviderTrackKey {
    Numeric(u64),
    Text(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderTrackKeyInput {
    Numeric(u64),
    Text(String),
}

impl TryFrom<ProviderTrackKeyInput> for ProviderTrackKey {
    type Error = PlayerServiceError;

    fn try_from(value: ProviderTrackKeyInput) -> Result<Self, Self::Error> {
        match value {
            ProviderTrackKeyInput::Numeric(value) if value > 0 && value <= i64::MAX as u64 => {
                Ok(Self::Numeric(value))
            },
            ProviderTrackKeyInput::Numeric(value) => Err(PlayerServiceError::InvalidIdentity {
                identity: "ProviderTrackKey::Numeric",
                value,
            }),
            ProviderTrackKeyInput::Text(value) => {
                if value.is_empty()
                    || value.trim() != value
                    || value.len() > MAX_PROVIDER_TEXT_KEY_BYTES
                {
                    return Err(PlayerServiceError::InvalidProviderTrackKey);
                }
                Ok(Self::Text(value))
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderTrackIdentityInput {
    pub source_instance_id: u64,
    pub provider_key: ProviderTrackKeyInput,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderTrackIdentity {
    pub source_instance_id: SourceInstanceId,
    pub provider_key: ProviderTrackKey,
}

impl TryFrom<ProviderTrackIdentityInput> for ProviderTrackIdentity {
    type Error = PlayerServiceError;

    fn try_from(value: ProviderTrackIdentityInput) -> Result<Self, Self::Error> {
        Ok(Self {
            source_instance_id: SourceInstanceId::new(value.source_instance_id)?,
            provider_key: value.provider_key.try_into()?,
        })
    }
}
