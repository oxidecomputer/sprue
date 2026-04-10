// The contents of this file are generated; do not modify them.

#[allow(unused_imports)]
use progenitor_client::{encode_path, ClientHooks, OperationInfo, RequestBuilderExt};
#[allow(unused_imports)]
pub use progenitor_client::{ByteStream, ClientInfo, Error, ResponseValue};
/// Types used as operation parameters and responses.
#[allow(clippy::all)]
pub mod types {
    /// Error types.
    pub mod error {
        /// Error from a `TryFrom` or `FromStr` implementation.
        pub struct ConversionError(::std::borrow::Cow<'static, str>);
        impl ::std::error::Error for ConversionError {}

        impl ::std::fmt::Display for ConversionError {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> Result<(), ::std::fmt::Error> {
                ::std::fmt::Display::fmt(&self.0, f)
            }
        }

        impl ::std::fmt::Debug for ConversionError {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> Result<(), ::std::fmt::Error> {
                ::std::fmt::Debug::fmt(&self.0, f)
            }
        }

        impl From<&'static str> for ConversionError {
            fn from(value: &'static str) -> Self {
                Self(value.into())
            }
        }

        impl From<String> for ConversionError {
            fn from(value: String) -> Self {
                Self(value.into())
            }
        }
    }

    /// `Blob`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    /// {
    ///  "type": "object",
    ///  "required": [
    ///    "created_at",
    ///    "id",
    ///    "service_id",
    ///    "size",
    ///    "state",
    ///    "total_size",
    ///    "updated_at"
    ///  ],
    ///  "properties": {
    ///    "created_at": {
    ///      "type": "string",
    ///      "format": "date-time"
    ///    },
    ///    "id": {
    ///      "$ref": "#/components/schemas/TypedUuidForBlobId"
    ///    },
    ///    "service_id": {
    ///      "$ref": "#/components/schemas/TypedUuidForServiceId"
    ///    },
    ///    "size": {
    ///      "type": "integer",
    ///      "format": "int64"
    ///    },
    ///    "state": {
    ///      "$ref": "#/components/schemas/BlobState"
    ///    },
    ///    "total_size": {
    ///      "type": "integer",
    ///      "format": "int64"
    ///    },
    ///    "updated_at": {
    ///      "type": "string",
    ///      "format": "date-time"
    ///    }

    ///  }

    /// }

    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug, schemars :: JsonSchema)]
    pub struct Blob {
        pub created_at: ::chrono::DateTime<::chrono::offset::Utc>,
        pub id: TypedUuidForBlobId,
        pub service_id: TypedUuidForServiceId,
        pub size: i64,
        pub state: BlobState,
        pub total_size: i64,
        pub updated_at: ::chrono::DateTime<::chrono::offset::Utc>,
    }

    impl Blob {
        pub fn builder() -> builder::Blob {
            Default::default()
        }
    }

    /// `BlobId`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    /// false
    /// ```
    /// </details>
    #[derive(
        :: serde :: Deserialize,
        :: serde :: Serialize,
        Clone,
        Copy,
        Debug,
        Eq,
        Hash,
        Ord,
        PartialEq,
        PartialOrd,
        schemars :: JsonSchema,
    )]
    #[serde(deny_unknown_fields)]
    pub enum BlobId {}

    /// `BlobState`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    /// {
    ///  "oneOf": [
    ///    {
    ///      "type": "string",
    ///      "enum": [
    ///        "Pending",
    ///        "Cancelled",
    ///        "Failed"
    ///      ]
    ///    },
    ///    {
    ///      "type": "object",
    ///      "required": [
    ///        "Uploading"
    ///      ],
    ///      "properties": {
    ///        "Uploading": {
    ///          "$ref": "#/components/schemas/BlobUploadState"
    ///        }

    ///      },
    ///      "additionalProperties": false
    ///    },
    ///    {
    ///      "type": "object",
    ///      "required": [
    ///        "Transferring"
    ///      ],
    ///      "properties": {
    ///        "Transferring": {
    ///          "$ref": "#/components/schemas/BlobTransferState"
    ///        }

    ///      },
    ///      "additionalProperties": false
    ///    }

    ///  ]
    /// }

    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug, schemars :: JsonSchema)]
    pub enum BlobState {
        Pending,
        Cancelled,
        Failed,
        Uploading(BlobUploadState),
        Transferring(BlobTransferState),
    }

    impl ::std::convert::From<BlobUploadState> for BlobState {
        fn from(value: BlobUploadState) -> Self {
            Self::Uploading(value)
        }
    }

    impl ::std::convert::From<BlobTransferState> for BlobState {
        fn from(value: BlobTransferState) -> Self {
            Self::Transferring(value)
        }
    }

    /// `BlobTransferState`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    /// {
    ///  "type": "string",
    ///  "enum": [
    ///    "Started",
    ///    "Complete"
    ///  ]
    /// }

    /// ```
    /// </details>
    #[derive(
        :: serde :: Deserialize,
        :: serde :: Serialize,
        Clone,
        Copy,
        Debug,
        Eq,
        Hash,
        Ord,
        PartialEq,
        PartialOrd,
        schemars :: JsonSchema,
    )]
    pub enum BlobTransferState {
        Started,
        Complete,
    }

    impl ::std::fmt::Display for BlobTransferState {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Started => f.write_str("Started"),
                Self::Complete => f.write_str("Complete"),
            }
        }
    }

    impl ::std::str::FromStr for BlobTransferState {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "Started" => Ok(Self::Started),
                "Complete" => Ok(Self::Complete),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for BlobTransferState {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for BlobTransferState {
        type Error = self::error::ConversionError;
        fn try_from(value: &::std::string::String) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for BlobTransferState {
        type Error = self::error::ConversionError;
        fn try_from(value: ::std::string::String) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    /// `BlobUploadState`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    /// {
    ///  "type": "string",
    ///  "enum": [
    ///    "Started",
    ///    "Complete"
    ///  ]
    /// }

    /// ```
    /// </details>
    #[derive(
        :: serde :: Deserialize,
        :: serde :: Serialize,
        Clone,
        Copy,
        Debug,
        Eq,
        Hash,
        Ord,
        PartialEq,
        PartialOrd,
        schemars :: JsonSchema,
    )]
    pub enum BlobUploadState {
        Started,
        Complete,
    }

    impl ::std::fmt::Display for BlobUploadState {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Started => f.write_str("Started"),
                Self::Complete => f.write_str("Complete"),
            }
        }
    }

    impl ::std::str::FromStr for BlobUploadState {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "Started" => Ok(Self::Started),
                "Complete" => Ok(Self::Complete),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for BlobUploadState {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for BlobUploadState {
        type Error = self::error::ConversionError;
        fn try_from(value: &::std::string::String) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for BlobUploadState {
        type Error = self::error::ConversionError;
        fn try_from(value: ::std::string::String) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    /// `CheckinBody`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    /// {
    ///  "type": "object",
    ///  "required": [
    ///    "checked_in_at"
    ///  ],
    ///  "properties": {
    ///    "checked_in_at": {
    ///      "type": "string",
    ///      "format": "date-time"
    ///    }

    ///  }

    /// }

    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug, schemars :: JsonSchema)]
    pub struct CheckinBody {
        pub checked_in_at: ::chrono::DateTime<::chrono::offset::Utc>,
    }

    impl CheckinBody {
        pub fn builder() -> builder::CheckinBody {
            Default::default()
        }
    }

    /// Error information from a response.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    /// {
    ///  "description": "Error information from a response.",
    ///  "type": "object",
    ///  "required": [
    ///    "message",
    ///    "request_id"
    ///  ],
    ///  "properties": {
    ///    "error_code": {
    ///      "type": "string"
    ///    },
    ///    "message": {
    ///      "type": "string"
    ///    },
    ///    "request_id": {
    ///      "type": "string"
    ///    }

    ///  }

    /// }

    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug, schemars :: JsonSchema)]
    pub struct Error {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub error_code: ::std::option::Option<::std::string::String>,
        pub message: ::std::string::String,
        pub request_id: ::std::string::String,
    }

    impl Error {
        pub fn builder() -> builder::Error {
            Default::default()
        }
    }

    /// `HealthCheck`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    /// {
    ///  "type": "object",
    ///  "required": [
    ///    "checked_in_at",
    ///    "created_at",
    ///    "id",
    ///    "server_registration_id"
    ///  ],
    ///  "properties": {
    ///    "checked_in_at": {
    ///      "type": "string",
    ///      "format": "date-time"
    ///    },
    ///    "created_at": {
    ///      "type": "string",
    ///      "format": "date-time"
    ///    },
    ///    "id": {
    ///      "$ref": "#/components/schemas/TypedUuidForHealthCheckId"
    ///    },
    ///    "server_registration_id": {
    ///      "$ref": "#/components/schemas/TypedUuidForServerRegistrationId"
    ///    }

    ///  }

    /// }

    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug, schemars :: JsonSchema)]
    pub struct HealthCheck {
        pub checked_in_at: ::chrono::DateTime<::chrono::offset::Utc>,
        pub created_at: ::chrono::DateTime<::chrono::offset::Utc>,
        pub id: TypedUuidForHealthCheckId,
        pub server_registration_id: TypedUuidForServerRegistrationId,
    }

    impl HealthCheck {
        pub fn builder() -> builder::HealthCheck {
            Default::default()
        }
    }

    /// `HealthCheckId`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    /// false
    /// ```
    /// </details>
    #[derive(
        :: serde :: Deserialize,
        :: serde :: Serialize,
        Clone,
        Copy,
        Debug,
        Eq,
        Hash,
        Ord,
        PartialEq,
        PartialOrd,
        schemars :: JsonSchema,
    )]
    #[serde(deny_unknown_fields)]
    pub enum HealthCheckId {}

    /// `Jwk`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    /// {
    ///  "type": "object",
    ///  "required": [
    ///    "e",
    ///    "kid",
    ///    "kty",
    ///    "n",
    ///    "use"
    ///  ],
    ///  "properties": {
    ///    "e": {
    ///      "type": "string"
    ///    },
    ///    "kid": {
    ///      "type": "string"
    ///    },
    ///    "kty": {
    ///      "type": "string"
    ///    },
    ///    "n": {
    ///      "type": "string"
    ///    },
    ///    "use": {
    ///      "type": "string"
    ///    }

    ///  }

    /// }

    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug, schemars :: JsonSchema)]
    pub struct Jwk {
        pub e: ::std::string::String,
        pub kid: ::std::string::String,
        pub kty: ::std::string::String,
        pub n: ::std::string::String,
        #[serde(rename = "use")]
        pub use_: ::std::string::String,
    }

    impl Jwk {
        pub fn builder() -> builder::Jwk {
            Default::default()
        }
    }

    /// `Jwks`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    /// {
    ///  "type": "object",
    ///  "required": [
    ///    "keys"
    ///  ],
    ///  "properties": {
    ///    "keys": {
    ///      "type": "array",
    ///      "items": {
    ///        "$ref": "#/components/schemas/Jwk"
    ///      }

    ///    }

    ///  }

    /// }

    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug, schemars :: JsonSchema)]
    pub struct Jwks {
        pub keys: ::std::vec::Vec<Jwk>,
    }

    impl Jwks {
        pub fn builder() -> builder::Jwks {
            Default::default()
        }
    }

    /// `OidcServerToken`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    /// {
    ///  "type": "object",
    ///  "required": [
    ///    "token"
    ///  ],
    ///  "properties": {
    ///    "token": {
    ///      "type": "string"
    ///    }

    ///  }

    /// }

    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug, schemars :: JsonSchema)]
    pub struct OidcServerToken {
        pub token: ::std::string::String,
    }

    impl OidcServerToken {
        pub fn builder() -> builder::OidcServerToken {
            Default::default()
        }
    }

    /// `OidcServerTokenNonce`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    /// {
    ///  "type": "object",
    ///  "required": [
    ///    "nonce"
    ///  ],
    ///  "properties": {
    ///    "nonce": {
    ///      "type": "string"
    ///    }

    ///  }

    /// }

    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug, schemars :: JsonSchema)]
    pub struct OidcServerTokenNonce {
        pub nonce: ::std::string::String,
    }

    impl OidcServerTokenNonce {
        pub fn builder() -> builder::OidcServerTokenNonce {
            Default::default()
        }
    }

    /// `OpenIdConfiguration`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    /// {
    ///  "type": "object",
    ///  "required": [
    ///    "jwks_uri"
    ///  ],
    ///  "properties": {
    ///    "jwks_uri": {
    ///      "type": "string"
    ///    }

    ///  }

    /// }

    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug, schemars :: JsonSchema)]
    pub struct OpenIdConfiguration {
        pub jwks_uri: ::std::string::String,
    }

    impl OpenIdConfiguration {
        pub fn builder() -> builder::OpenIdConfiguration {
            Default::default()
        }
    }

    /// `RegisterBlobBody`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    /// {
    ///  "type": "object",
    ///  "required": [
    ///    "size"
    ///  ],
    ///  "properties": {
    ///    "idempotency_key": {
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ]
    ///    },
    ///    "size": {
    ///      "type": "integer",
    ///      "format": "int64"
    ///    }

    ///  }

    /// }

    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug, schemars :: JsonSchema)]
    pub struct RegisterBlobBody {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub idempotency_key: ::std::option::Option<::std::string::String>,
        pub size: i64,
    }

    impl RegisterBlobBody {
        pub fn builder() -> builder::RegisterBlobBody {
            Default::default()
        }
    }

    /// `RegisterBlobResponse`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    /// {
    ///  "type": "object",
    ///  "required": [
    ///    "blob"
    ///  ],
    ///  "properties": {
    ///    "blob": {
    ///      "$ref": "#/components/schemas/Blob"
    ///    }

    ///  }

    /// }

    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug, schemars :: JsonSchema)]
    pub struct RegisterBlobResponse {
        pub blob: Blob,
    }

    impl RegisterBlobResponse {
        pub fn builder() -> builder::RegisterBlobResponse {
            Default::default()
        }
    }

    /// `RegisterServerBody`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    /// {
    ///  "type": "object",
    ///  "required": [
    ///    "instance"
    ///  ],
    ///  "properties": {
    ///    "instance": {
    ///      "$ref":
    /// "#/components/schemas/TypedUuidForServerRegistrationInstanceId"
    ///    }

    ///  }

    /// }

    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug, schemars :: JsonSchema)]
    pub struct RegisterServerBody {
        pub instance: TypedUuidForServerRegistrationInstanceId,
    }

    impl RegisterServerBody {
        pub fn builder() -> builder::RegisterServerBody {
            Default::default()
        }
    }

    /// `RegisterServerResponse`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    /// {
    ///  "type": "object",
    ///  "required": [
    ///    "registration"
    ///  ],
    ///  "properties": {
    ///    "registration": {
    ///      "$ref": "#/components/schemas/ServerRegistration"
    ///    }

    ///  }

    /// }

    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug, schemars :: JsonSchema)]
    pub struct RegisterServerResponse {
        pub registration: ServerRegistration,
    }

    impl RegisterServerResponse {
        pub fn builder() -> builder::RegisterServerResponse {
            Default::default()
        }
    }

    /// `ServerAttestation`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    /// {
    ///  "type": "object",
    ///  "required": [
    ///    "attestation"
    ///  ],
    ///  "properties": {
    ///    "attestation": {}

    ///  }

    /// }

    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug, schemars :: JsonSchema)]
    pub struct ServerAttestation {
        pub attestation: ::serde_json::Value,
    }

    impl ServerAttestation {
        pub fn builder() -> builder::ServerAttestation {
            Default::default()
        }
    }

    /// `ServerRegistration`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    /// {
    ///  "type": "object",
    ///  "required": [
    ///    "created_at",
    ///    "id",
    ///    "instance_id",
    ///    "service_id",
    ///    "state",
    ///    "updated_at"
    ///  ],
    ///  "properties": {
    ///    "created_at": {
    ///      "type": "string",
    ///      "format": "date-time"
    ///    },
    ///    "id": {
    ///      "$ref": "#/components/schemas/TypedUuidForServerRegistrationId"
    ///    },
    ///    "instance_id": {
    ///      "$ref":
    /// "#/components/schemas/TypedUuidForServerRegistrationInstanceId"
    ///    },
    ///    "service_id": {
    ///      "$ref": "#/components/schemas/TypedUuidForServiceId"
    ///    },
    ///    "state": {
    ///      "$ref": "#/components/schemas/ServerRegistrationState"
    ///    },
    ///    "updated_at": {
    ///      "type": "string",
    ///      "format": "date-time"
    ///    }

    ///  }

    /// }

    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug, schemars :: JsonSchema)]
    pub struct ServerRegistration {
        pub created_at: ::chrono::DateTime<::chrono::offset::Utc>,
        pub id: TypedUuidForServerRegistrationId,
        pub instance_id: TypedUuidForServerRegistrationInstanceId,
        pub service_id: TypedUuidForServiceId,
        pub state: ServerRegistrationState,
        pub updated_at: ::chrono::DateTime<::chrono::offset::Utc>,
    }

    impl ServerRegistration {
        pub fn builder() -> builder::ServerRegistration {
            Default::default()
        }
    }

    /// `ServerRegistrationId`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    /// false
    /// ```
    /// </details>
    #[derive(
        :: serde :: Deserialize,
        :: serde :: Serialize,
        Clone,
        Copy,
        Debug,
        Eq,
        Hash,
        Ord,
        PartialEq,
        PartialOrd,
        schemars :: JsonSchema,
    )]
    #[serde(deny_unknown_fields)]
    pub enum ServerRegistrationId {}

    /// `ServerRegistrationInstanceId`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    /// false
    /// ```
    /// </details>
    #[derive(
        :: serde :: Deserialize,
        :: serde :: Serialize,
        Clone,
        Copy,
        Debug,
        Eq,
        Hash,
        Ord,
        PartialEq,
        PartialOrd,
        schemars :: JsonSchema,
    )]
    #[serde(deny_unknown_fields)]
    pub enum ServerRegistrationInstanceId {}

    /// `ServerRegistrationState`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    /// {
    ///  "type": "string",
    ///  "enum": [
    ///    "Pending",
    ///    "Accepted",
    ///    "Rejected",
    ///    "Terminated"
    ///  ]
    /// }

    /// ```
    /// </details>
    #[derive(
        :: serde :: Deserialize,
        :: serde :: Serialize,
        Clone,
        Copy,
        Debug,
        Eq,
        Hash,
        Ord,
        PartialEq,
        PartialOrd,
        schemars :: JsonSchema,
    )]
    pub enum ServerRegistrationState {
        Pending,
        Accepted,
        Rejected,
        Terminated,
    }

    impl ::std::fmt::Display for ServerRegistrationState {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Pending => f.write_str("Pending"),
                Self::Accepted => f.write_str("Accepted"),
                Self::Rejected => f.write_str("Rejected"),
                Self::Terminated => f.write_str("Terminated"),
            }
        }
    }

    impl ::std::str::FromStr for ServerRegistrationState {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "Pending" => Ok(Self::Pending),
                "Accepted" => Ok(Self::Accepted),
                "Rejected" => Ok(Self::Rejected),
                "Terminated" => Ok(Self::Terminated),
                _ => Err("invalid value".into()),
            }
        }
    }

    impl ::std::convert::TryFrom<&str> for ServerRegistrationState {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<&::std::string::String> for ServerRegistrationState {
        type Error = self::error::ConversionError;
        fn try_from(value: &::std::string::String) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<::std::string::String> for ServerRegistrationState {
        type Error = self::error::ConversionError;
        fn try_from(value: ::std::string::String) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }

    /// `ServiceId`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    /// false
    /// ```
    /// </details>
    #[derive(
        :: serde :: Deserialize,
        :: serde :: Serialize,
        Clone,
        Copy,
        Debug,
        Eq,
        Hash,
        Ord,
        PartialEq,
        PartialOrd,
        schemars :: JsonSchema,
    )]
    #[serde(deny_unknown_fields)]
    pub enum ServiceId {}

    /// `TypedUuidForBlobId`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    /// {
    ///  "type": "string",
    ///  "format": "uuid",
    ///  "x-rust-type": {
    ///    "crate": "newtype-uuid",
    ///    "parameters": [
    ///      {
    ///        "$ref": "#/components/schemas/BlobId"
    ///      }

    ///    ],
    ///    "path": "newtype_uuid::TypedUuid",
    ///    "version": "1"
    ///  }

    /// }

    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug, schemars :: JsonSchema)]
    #[serde(transparent)]
    pub struct TypedUuidForBlobId(pub ::uuid::Uuid);
    impl ::std::ops::Deref for TypedUuidForBlobId {
        type Target = ::uuid::Uuid;
        fn deref(&self) -> &::uuid::Uuid {
            &self.0
        }
    }

    impl ::std::convert::From<TypedUuidForBlobId> for ::uuid::Uuid {
        fn from(value: TypedUuidForBlobId) -> Self {
            value.0
        }
    }

    impl ::std::convert::From<::uuid::Uuid> for TypedUuidForBlobId {
        fn from(value: ::uuid::Uuid) -> Self {
            Self(value)
        }
    }

    impl ::std::str::FromStr for TypedUuidForBlobId {
        type Err = <::uuid::Uuid as ::std::str::FromStr>::Err;
        fn from_str(value: &str) -> ::std::result::Result<Self, Self::Err> {
            Ok(Self(value.parse()?))
        }
    }

    impl ::std::convert::TryFrom<&str> for TypedUuidForBlobId {
        type Error = <::uuid::Uuid as ::std::str::FromStr>::Err;
        fn try_from(value: &str) -> ::std::result::Result<Self, Self::Error> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<String> for TypedUuidForBlobId {
        type Error = <::uuid::Uuid as ::std::str::FromStr>::Err;
        fn try_from(value: String) -> ::std::result::Result<Self, Self::Error> {
            value.parse()
        }
    }

    impl ::std::fmt::Display for TypedUuidForBlobId {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            self.0.fmt(f)
        }
    }

    /// `TypedUuidForHealthCheckId`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    /// {
    ///  "type": "string",
    ///  "format": "uuid",
    ///  "x-rust-type": {
    ///    "crate": "newtype-uuid",
    ///    "parameters": [
    ///      {
    ///        "$ref": "#/components/schemas/HealthCheckId"
    ///      }

    ///    ],
    ///    "path": "newtype_uuid::TypedUuid",
    ///    "version": "1"
    ///  }

    /// }

    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug, schemars :: JsonSchema)]
    #[serde(transparent)]
    pub struct TypedUuidForHealthCheckId(pub ::uuid::Uuid);
    impl ::std::ops::Deref for TypedUuidForHealthCheckId {
        type Target = ::uuid::Uuid;
        fn deref(&self) -> &::uuid::Uuid {
            &self.0
        }
    }

    impl ::std::convert::From<TypedUuidForHealthCheckId> for ::uuid::Uuid {
        fn from(value: TypedUuidForHealthCheckId) -> Self {
            value.0
        }
    }

    impl ::std::convert::From<::uuid::Uuid> for TypedUuidForHealthCheckId {
        fn from(value: ::uuid::Uuid) -> Self {
            Self(value)
        }
    }

    impl ::std::str::FromStr for TypedUuidForHealthCheckId {
        type Err = <::uuid::Uuid as ::std::str::FromStr>::Err;
        fn from_str(value: &str) -> ::std::result::Result<Self, Self::Err> {
            Ok(Self(value.parse()?))
        }
    }

    impl ::std::convert::TryFrom<&str> for TypedUuidForHealthCheckId {
        type Error = <::uuid::Uuid as ::std::str::FromStr>::Err;
        fn try_from(value: &str) -> ::std::result::Result<Self, Self::Error> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<String> for TypedUuidForHealthCheckId {
        type Error = <::uuid::Uuid as ::std::str::FromStr>::Err;
        fn try_from(value: String) -> ::std::result::Result<Self, Self::Error> {
            value.parse()
        }
    }

    impl ::std::fmt::Display for TypedUuidForHealthCheckId {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            self.0.fmt(f)
        }
    }

    /// `TypedUuidForServerRegistrationId`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    /// {
    ///  "type": "string",
    ///  "format": "uuid",
    ///  "x-rust-type": {
    ///    "crate": "newtype-uuid",
    ///    "parameters": [
    ///      {
    ///        "$ref": "#/components/schemas/ServerRegistrationId"
    ///      }

    ///    ],
    ///    "path": "newtype_uuid::TypedUuid",
    ///    "version": "1"
    ///  }

    /// }

    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug, schemars :: JsonSchema)]
    #[serde(transparent)]
    pub struct TypedUuidForServerRegistrationId(pub ::uuid::Uuid);
    impl ::std::ops::Deref for TypedUuidForServerRegistrationId {
        type Target = ::uuid::Uuid;
        fn deref(&self) -> &::uuid::Uuid {
            &self.0
        }
    }

    impl ::std::convert::From<TypedUuidForServerRegistrationId> for ::uuid::Uuid {
        fn from(value: TypedUuidForServerRegistrationId) -> Self {
            value.0
        }
    }

    impl ::std::convert::From<::uuid::Uuid> for TypedUuidForServerRegistrationId {
        fn from(value: ::uuid::Uuid) -> Self {
            Self(value)
        }
    }

    impl ::std::str::FromStr for TypedUuidForServerRegistrationId {
        type Err = <::uuid::Uuid as ::std::str::FromStr>::Err;
        fn from_str(value: &str) -> ::std::result::Result<Self, Self::Err> {
            Ok(Self(value.parse()?))
        }
    }

    impl ::std::convert::TryFrom<&str> for TypedUuidForServerRegistrationId {
        type Error = <::uuid::Uuid as ::std::str::FromStr>::Err;
        fn try_from(value: &str) -> ::std::result::Result<Self, Self::Error> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<String> for TypedUuidForServerRegistrationId {
        type Error = <::uuid::Uuid as ::std::str::FromStr>::Err;
        fn try_from(value: String) -> ::std::result::Result<Self, Self::Error> {
            value.parse()
        }
    }

    impl ::std::fmt::Display for TypedUuidForServerRegistrationId {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            self.0.fmt(f)
        }
    }

    /// `TypedUuidForServerRegistrationInstanceId`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    /// {
    ///  "type": "string",
    ///  "format": "uuid",
    ///  "x-rust-type": {
    ///    "crate": "newtype-uuid",
    ///    "parameters": [
    ///      {
    ///        "$ref": "#/components/schemas/ServerRegistrationInstanceId"
    ///      }

    ///    ],
    ///    "path": "newtype_uuid::TypedUuid",
    ///    "version": "1"
    ///  }

    /// }

    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug, schemars :: JsonSchema)]
    #[serde(transparent)]
    pub struct TypedUuidForServerRegistrationInstanceId(pub ::uuid::Uuid);
    impl ::std::ops::Deref for TypedUuidForServerRegistrationInstanceId {
        type Target = ::uuid::Uuid;
        fn deref(&self) -> &::uuid::Uuid {
            &self.0
        }
    }

    impl ::std::convert::From<TypedUuidForServerRegistrationInstanceId> for ::uuid::Uuid {
        fn from(value: TypedUuidForServerRegistrationInstanceId) -> Self {
            value.0
        }
    }

    impl ::std::convert::From<::uuid::Uuid> for TypedUuidForServerRegistrationInstanceId {
        fn from(value: ::uuid::Uuid) -> Self {
            Self(value)
        }
    }

    impl ::std::str::FromStr for TypedUuidForServerRegistrationInstanceId {
        type Err = <::uuid::Uuid as ::std::str::FromStr>::Err;
        fn from_str(value: &str) -> ::std::result::Result<Self, Self::Err> {
            Ok(Self(value.parse()?))
        }
    }

    impl ::std::convert::TryFrom<&str> for TypedUuidForServerRegistrationInstanceId {
        type Error = <::uuid::Uuid as ::std::str::FromStr>::Err;
        fn try_from(value: &str) -> ::std::result::Result<Self, Self::Error> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<String> for TypedUuidForServerRegistrationInstanceId {
        type Error = <::uuid::Uuid as ::std::str::FromStr>::Err;
        fn try_from(value: String) -> ::std::result::Result<Self, Self::Error> {
            value.parse()
        }
    }

    impl ::std::fmt::Display for TypedUuidForServerRegistrationInstanceId {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            self.0.fmt(f)
        }
    }

    /// `TypedUuidForServiceId`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    /// {
    ///  "type": "string",
    ///  "format": "uuid",
    ///  "x-rust-type": {
    ///    "crate": "newtype-uuid",
    ///    "parameters": [
    ///      {
    ///        "$ref": "#/components/schemas/ServiceId"
    ///      }

    ///    ],
    ///    "path": "newtype_uuid::TypedUuid",
    ///    "version": "1"
    ///  }

    /// }

    /// ```
    /// </details>
    #[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone, Debug, schemars :: JsonSchema)]
    #[serde(transparent)]
    pub struct TypedUuidForServiceId(pub ::uuid::Uuid);
    impl ::std::ops::Deref for TypedUuidForServiceId {
        type Target = ::uuid::Uuid;
        fn deref(&self) -> &::uuid::Uuid {
            &self.0
        }
    }

    impl ::std::convert::From<TypedUuidForServiceId> for ::uuid::Uuid {
        fn from(value: TypedUuidForServiceId) -> Self {
            value.0
        }
    }

    impl ::std::convert::From<::uuid::Uuid> for TypedUuidForServiceId {
        fn from(value: ::uuid::Uuid) -> Self {
            Self(value)
        }
    }

    impl ::std::str::FromStr for TypedUuidForServiceId {
        type Err = <::uuid::Uuid as ::std::str::FromStr>::Err;
        fn from_str(value: &str) -> ::std::result::Result<Self, Self::Err> {
            Ok(Self(value.parse()?))
        }
    }

    impl ::std::convert::TryFrom<&str> for TypedUuidForServiceId {
        type Error = <::uuid::Uuid as ::std::str::FromStr>::Err;
        fn try_from(value: &str) -> ::std::result::Result<Self, Self::Error> {
            value.parse()
        }
    }

    impl ::std::convert::TryFrom<String> for TypedUuidForServiceId {
        type Error = <::uuid::Uuid as ::std::str::FromStr>::Err;
        fn try_from(value: String) -> ::std::result::Result<Self, Self::Error> {
            value.parse()
        }
    }

    impl ::std::fmt::Display for TypedUuidForServiceId {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            self.0.fmt(f)
        }
    }

    /// Types for composing complex structures.
    pub mod builder {
        #[derive(Clone, Debug)]
        pub struct Blob {
            created_at: ::std::result::Result<::chrono::DateTime<::chrono::offset::Utc>, ::std::string::String>,
            id: ::std::result::Result<super::TypedUuidForBlobId, ::std::string::String>,
            service_id: ::std::result::Result<super::TypedUuidForServiceId, ::std::string::String>,
            size: ::std::result::Result<i64, ::std::string::String>,
            state: ::std::result::Result<super::BlobState, ::std::string::String>,
            total_size: ::std::result::Result<i64, ::std::string::String>,
            updated_at: ::std::result::Result<::chrono::DateTime<::chrono::offset::Utc>, ::std::string::String>,
        }

        impl ::std::default::Default for Blob {
            fn default() -> Self {
                Self {
                    created_at: Err("no value supplied for created_at".to_string()),
                    id: Err("no value supplied for id".to_string()),
                    service_id: Err("no value supplied for service_id".to_string()),
                    size: Err("no value supplied for size".to_string()),
                    state: Err("no value supplied for state".to_string()),
                    total_size: Err("no value supplied for total_size".to_string()),
                    updated_at: Err("no value supplied for updated_at".to_string()),
                }
            }
        }

        impl Blob {
            pub fn created_at<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::chrono::DateTime<::chrono::offset::Utc>>,
                T::Error: ::std::fmt::Display,
            {
                self.created_at = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for created_at: {e}"));
                self
            }
            pub fn id<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<super::TypedUuidForBlobId>,
                T::Error: ::std::fmt::Display,
            {
                self.id = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for id: {e}"));
                self
            }
            pub fn service_id<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<super::TypedUuidForServiceId>,
                T::Error: ::std::fmt::Display,
            {
                self.service_id = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for service_id: {e}"));
                self
            }
            pub fn size<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<i64>,
                T::Error: ::std::fmt::Display,
            {
                self.size = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for size: {e}"));
                self
            }
            pub fn state<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<super::BlobState>,
                T::Error: ::std::fmt::Display,
            {
                self.state = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for state: {e}"));
                self
            }
            pub fn total_size<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<i64>,
                T::Error: ::std::fmt::Display,
            {
                self.total_size = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for total_size: {e}"));
                self
            }
            pub fn updated_at<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::chrono::DateTime<::chrono::offset::Utc>>,
                T::Error: ::std::fmt::Display,
            {
                self.updated_at = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for updated_at: {e}"));
                self
            }
        }

        impl ::std::convert::TryFrom<Blob> for super::Blob {
            type Error = super::error::ConversionError;
            fn try_from(value: Blob) -> ::std::result::Result<Self, super::error::ConversionError> {
                Ok(Self {
                    created_at: value.created_at?,
                    id: value.id?,
                    service_id: value.service_id?,
                    size: value.size?,
                    state: value.state?,
                    total_size: value.total_size?,
                    updated_at: value.updated_at?,
                })
            }
        }

        impl ::std::convert::From<super::Blob> for Blob {
            fn from(value: super::Blob) -> Self {
                Self {
                    created_at: Ok(value.created_at),
                    id: Ok(value.id),
                    service_id: Ok(value.service_id),
                    size: Ok(value.size),
                    state: Ok(value.state),
                    total_size: Ok(value.total_size),
                    updated_at: Ok(value.updated_at),
                }
            }
        }

        #[derive(Clone, Debug)]
        pub struct CheckinBody {
            checked_in_at: ::std::result::Result<::chrono::DateTime<::chrono::offset::Utc>, ::std::string::String>,
        }

        impl ::std::default::Default for CheckinBody {
            fn default() -> Self {
                Self {
                    checked_in_at: Err("no value supplied for checked_in_at".to_string()),
                }
            }
        }

        impl CheckinBody {
            pub fn checked_in_at<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::chrono::DateTime<::chrono::offset::Utc>>,
                T::Error: ::std::fmt::Display,
            {
                self.checked_in_at = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for checked_in_at: {e}"));
                self
            }
        }

        impl ::std::convert::TryFrom<CheckinBody> for super::CheckinBody {
            type Error = super::error::ConversionError;
            fn try_from(value: CheckinBody) -> ::std::result::Result<Self, super::error::ConversionError> {
                Ok(Self {
                    checked_in_at: value.checked_in_at?,
                })
            }
        }

        impl ::std::convert::From<super::CheckinBody> for CheckinBody {
            fn from(value: super::CheckinBody) -> Self {
                Self {
                    checked_in_at: Ok(value.checked_in_at),
                }
            }
        }

        #[derive(Clone, Debug)]
        pub struct Error {
            error_code: ::std::result::Result<::std::option::Option<::std::string::String>, ::std::string::String>,
            message: ::std::result::Result<::std::string::String, ::std::string::String>,
            request_id: ::std::result::Result<::std::string::String, ::std::string::String>,
        }

        impl ::std::default::Default for Error {
            fn default() -> Self {
                Self {
                    error_code: Ok(Default::default()),
                    message: Err("no value supplied for message".to_string()),
                    request_id: Err("no value supplied for request_id".to_string()),
                }
            }
        }

        impl Error {
            pub fn error_code<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
                T::Error: ::std::fmt::Display,
            {
                self.error_code = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for error_code: {e}"));
                self
            }
            pub fn message<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::string::String>,
                T::Error: ::std::fmt::Display,
            {
                self.message = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for message: {e}"));
                self
            }
            pub fn request_id<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::string::String>,
                T::Error: ::std::fmt::Display,
            {
                self.request_id = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for request_id: {e}"));
                self
            }
        }

        impl ::std::convert::TryFrom<Error> for super::Error {
            type Error = super::error::ConversionError;
            fn try_from(value: Error) -> ::std::result::Result<Self, super::error::ConversionError> {
                Ok(Self {
                    error_code: value.error_code?,
                    message: value.message?,
                    request_id: value.request_id?,
                })
            }
        }

        impl ::std::convert::From<super::Error> for Error {
            fn from(value: super::Error) -> Self {
                Self {
                    error_code: Ok(value.error_code),
                    message: Ok(value.message),
                    request_id: Ok(value.request_id),
                }
            }
        }

        #[derive(Clone, Debug)]
        pub struct HealthCheck {
            checked_in_at: ::std::result::Result<::chrono::DateTime<::chrono::offset::Utc>, ::std::string::String>,
            created_at: ::std::result::Result<::chrono::DateTime<::chrono::offset::Utc>, ::std::string::String>,
            id: ::std::result::Result<super::TypedUuidForHealthCheckId, ::std::string::String>,
            server_registration_id:
                ::std::result::Result<super::TypedUuidForServerRegistrationId, ::std::string::String>,
        }

        impl ::std::default::Default for HealthCheck {
            fn default() -> Self {
                Self {
                    checked_in_at: Err("no value supplied for checked_in_at".to_string()),
                    created_at: Err("no value supplied for created_at".to_string()),
                    id: Err("no value supplied for id".to_string()),
                    server_registration_id: Err("no value supplied for server_registration_id".to_string()),
                }
            }
        }

        impl HealthCheck {
            pub fn checked_in_at<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::chrono::DateTime<::chrono::offset::Utc>>,
                T::Error: ::std::fmt::Display,
            {
                self.checked_in_at = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for checked_in_at: {e}"));
                self
            }
            pub fn created_at<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::chrono::DateTime<::chrono::offset::Utc>>,
                T::Error: ::std::fmt::Display,
            {
                self.created_at = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for created_at: {e}"));
                self
            }
            pub fn id<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<super::TypedUuidForHealthCheckId>,
                T::Error: ::std::fmt::Display,
            {
                self.id = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for id: {e}"));
                self
            }
            pub fn server_registration_id<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<super::TypedUuidForServerRegistrationId>,
                T::Error: ::std::fmt::Display,
            {
                self.server_registration_id = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for server_registration_id: {e}"));
                self
            }
        }

        impl ::std::convert::TryFrom<HealthCheck> for super::HealthCheck {
            type Error = super::error::ConversionError;
            fn try_from(value: HealthCheck) -> ::std::result::Result<Self, super::error::ConversionError> {
                Ok(Self {
                    checked_in_at: value.checked_in_at?,
                    created_at: value.created_at?,
                    id: value.id?,
                    server_registration_id: value.server_registration_id?,
                })
            }
        }

        impl ::std::convert::From<super::HealthCheck> for HealthCheck {
            fn from(value: super::HealthCheck) -> Self {
                Self {
                    checked_in_at: Ok(value.checked_in_at),
                    created_at: Ok(value.created_at),
                    id: Ok(value.id),
                    server_registration_id: Ok(value.server_registration_id),
                }
            }
        }

        #[derive(Clone, Debug)]
        pub struct Jwk {
            e: ::std::result::Result<::std::string::String, ::std::string::String>,
            kid: ::std::result::Result<::std::string::String, ::std::string::String>,
            kty: ::std::result::Result<::std::string::String, ::std::string::String>,
            n: ::std::result::Result<::std::string::String, ::std::string::String>,
            use_: ::std::result::Result<::std::string::String, ::std::string::String>,
        }

        impl ::std::default::Default for Jwk {
            fn default() -> Self {
                Self {
                    e: Err("no value supplied for e".to_string()),
                    kid: Err("no value supplied for kid".to_string()),
                    kty: Err("no value supplied for kty".to_string()),
                    n: Err("no value supplied for n".to_string()),
                    use_: Err("no value supplied for use_".to_string()),
                }
            }
        }

        impl Jwk {
            pub fn e<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::string::String>,
                T::Error: ::std::fmt::Display,
            {
                self.e = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for e: {e}"));
                self
            }
            pub fn kid<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::string::String>,
                T::Error: ::std::fmt::Display,
            {
                self.kid = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for kid: {e}"));
                self
            }
            pub fn kty<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::string::String>,
                T::Error: ::std::fmt::Display,
            {
                self.kty = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for kty: {e}"));
                self
            }
            pub fn n<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::string::String>,
                T::Error: ::std::fmt::Display,
            {
                self.n = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for n: {e}"));
                self
            }
            pub fn use_<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::string::String>,
                T::Error: ::std::fmt::Display,
            {
                self.use_ = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for use_: {e}"));
                self
            }
        }

        impl ::std::convert::TryFrom<Jwk> for super::Jwk {
            type Error = super::error::ConversionError;
            fn try_from(value: Jwk) -> ::std::result::Result<Self, super::error::ConversionError> {
                Ok(Self {
                    e: value.e?,
                    kid: value.kid?,
                    kty: value.kty?,
                    n: value.n?,
                    use_: value.use_?,
                })
            }
        }

        impl ::std::convert::From<super::Jwk> for Jwk {
            fn from(value: super::Jwk) -> Self {
                Self {
                    e: Ok(value.e),
                    kid: Ok(value.kid),
                    kty: Ok(value.kty),
                    n: Ok(value.n),
                    use_: Ok(value.use_),
                }
            }
        }

        #[derive(Clone, Debug)]
        pub struct Jwks {
            keys: ::std::result::Result<::std::vec::Vec<super::Jwk>, ::std::string::String>,
        }

        impl ::std::default::Default for Jwks {
            fn default() -> Self {
                Self {
                    keys: Err("no value supplied for keys".to_string()),
                }
            }
        }

        impl Jwks {
            pub fn keys<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::vec::Vec<super::Jwk>>,
                T::Error: ::std::fmt::Display,
            {
                self.keys = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for keys: {e}"));
                self
            }
        }

        impl ::std::convert::TryFrom<Jwks> for super::Jwks {
            type Error = super::error::ConversionError;
            fn try_from(value: Jwks) -> ::std::result::Result<Self, super::error::ConversionError> {
                Ok(Self { keys: value.keys? })
            }
        }

        impl ::std::convert::From<super::Jwks> for Jwks {
            fn from(value: super::Jwks) -> Self {
                Self { keys: Ok(value.keys) }
            }
        }

        #[derive(Clone, Debug)]
        pub struct OidcServerToken {
            token: ::std::result::Result<::std::string::String, ::std::string::String>,
        }

        impl ::std::default::Default for OidcServerToken {
            fn default() -> Self {
                Self {
                    token: Err("no value supplied for token".to_string()),
                }
            }
        }

        impl OidcServerToken {
            pub fn token<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::string::String>,
                T::Error: ::std::fmt::Display,
            {
                self.token = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for token: {e}"));
                self
            }
        }

        impl ::std::convert::TryFrom<OidcServerToken> for super::OidcServerToken {
            type Error = super::error::ConversionError;
            fn try_from(value: OidcServerToken) -> ::std::result::Result<Self, super::error::ConversionError> {
                Ok(Self { token: value.token? })
            }
        }

        impl ::std::convert::From<super::OidcServerToken> for OidcServerToken {
            fn from(value: super::OidcServerToken) -> Self {
                Self { token: Ok(value.token) }
            }
        }

        #[derive(Clone, Debug)]
        pub struct OidcServerTokenNonce {
            nonce: ::std::result::Result<::std::string::String, ::std::string::String>,
        }

        impl ::std::default::Default for OidcServerTokenNonce {
            fn default() -> Self {
                Self {
                    nonce: Err("no value supplied for nonce".to_string()),
                }
            }
        }

        impl OidcServerTokenNonce {
            pub fn nonce<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::string::String>,
                T::Error: ::std::fmt::Display,
            {
                self.nonce = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for nonce: {e}"));
                self
            }
        }

        impl ::std::convert::TryFrom<OidcServerTokenNonce> for super::OidcServerTokenNonce {
            type Error = super::error::ConversionError;
            fn try_from(value: OidcServerTokenNonce) -> ::std::result::Result<Self, super::error::ConversionError> {
                Ok(Self { nonce: value.nonce? })
            }
        }

        impl ::std::convert::From<super::OidcServerTokenNonce> for OidcServerTokenNonce {
            fn from(value: super::OidcServerTokenNonce) -> Self {
                Self { nonce: Ok(value.nonce) }
            }
        }

        #[derive(Clone, Debug)]
        pub struct OpenIdConfiguration {
            jwks_uri: ::std::result::Result<::std::string::String, ::std::string::String>,
        }

        impl ::std::default::Default for OpenIdConfiguration {
            fn default() -> Self {
                Self {
                    jwks_uri: Err("no value supplied for jwks_uri".to_string()),
                }
            }
        }

        impl OpenIdConfiguration {
            pub fn jwks_uri<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::string::String>,
                T::Error: ::std::fmt::Display,
            {
                self.jwks_uri = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for jwks_uri: {e}"));
                self
            }
        }

        impl ::std::convert::TryFrom<OpenIdConfiguration> for super::OpenIdConfiguration {
            type Error = super::error::ConversionError;
            fn try_from(value: OpenIdConfiguration) -> ::std::result::Result<Self, super::error::ConversionError> {
                Ok(Self {
                    jwks_uri: value.jwks_uri?,
                })
            }
        }

        impl ::std::convert::From<super::OpenIdConfiguration> for OpenIdConfiguration {
            fn from(value: super::OpenIdConfiguration) -> Self {
                Self {
                    jwks_uri: Ok(value.jwks_uri),
                }
            }
        }

        #[derive(Clone, Debug)]
        pub struct RegisterBlobBody {
            idempotency_key: ::std::result::Result<::std::option::Option<::std::string::String>, ::std::string::String>,
            size: ::std::result::Result<i64, ::std::string::String>,
        }

        impl ::std::default::Default for RegisterBlobBody {
            fn default() -> Self {
                Self {
                    idempotency_key: Ok(Default::default()),
                    size: Err("no value supplied for size".to_string()),
                }
            }
        }

        impl RegisterBlobBody {
            pub fn idempotency_key<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
                T::Error: ::std::fmt::Display,
            {
                self.idempotency_key = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for idempotency_key: {e}"));
                self
            }
            pub fn size<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<i64>,
                T::Error: ::std::fmt::Display,
            {
                self.size = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for size: {e}"));
                self
            }
        }

        impl ::std::convert::TryFrom<RegisterBlobBody> for super::RegisterBlobBody {
            type Error = super::error::ConversionError;
            fn try_from(value: RegisterBlobBody) -> ::std::result::Result<Self, super::error::ConversionError> {
                Ok(Self {
                    idempotency_key: value.idempotency_key?,
                    size: value.size?,
                })
            }
        }

        impl ::std::convert::From<super::RegisterBlobBody> for RegisterBlobBody {
            fn from(value: super::RegisterBlobBody) -> Self {
                Self {
                    idempotency_key: Ok(value.idempotency_key),
                    size: Ok(value.size),
                }
            }
        }

        #[derive(Clone, Debug)]
        pub struct RegisterBlobResponse {
            blob: ::std::result::Result<super::Blob, ::std::string::String>,
        }

        impl ::std::default::Default for RegisterBlobResponse {
            fn default() -> Self {
                Self {
                    blob: Err("no value supplied for blob".to_string()),
                }
            }
        }

        impl RegisterBlobResponse {
            pub fn blob<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<super::Blob>,
                T::Error: ::std::fmt::Display,
            {
                self.blob = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for blob: {e}"));
                self
            }
        }

        impl ::std::convert::TryFrom<RegisterBlobResponse> for super::RegisterBlobResponse {
            type Error = super::error::ConversionError;
            fn try_from(value: RegisterBlobResponse) -> ::std::result::Result<Self, super::error::ConversionError> {
                Ok(Self { blob: value.blob? })
            }
        }

        impl ::std::convert::From<super::RegisterBlobResponse> for RegisterBlobResponse {
            fn from(value: super::RegisterBlobResponse) -> Self {
                Self { blob: Ok(value.blob) }
            }
        }

        #[derive(Clone, Debug)]
        pub struct RegisterServerBody {
            instance: ::std::result::Result<super::TypedUuidForServerRegistrationInstanceId, ::std::string::String>,
        }

        impl ::std::default::Default for RegisterServerBody {
            fn default() -> Self {
                Self {
                    instance: Err("no value supplied for instance".to_string()),
                }
            }
        }

        impl RegisterServerBody {
            pub fn instance<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<super::TypedUuidForServerRegistrationInstanceId>,
                T::Error: ::std::fmt::Display,
            {
                self.instance = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for instance: {e}"));
                self
            }
        }

        impl ::std::convert::TryFrom<RegisterServerBody> for super::RegisterServerBody {
            type Error = super::error::ConversionError;
            fn try_from(value: RegisterServerBody) -> ::std::result::Result<Self, super::error::ConversionError> {
                Ok(Self {
                    instance: value.instance?,
                })
            }
        }

        impl ::std::convert::From<super::RegisterServerBody> for RegisterServerBody {
            fn from(value: super::RegisterServerBody) -> Self {
                Self {
                    instance: Ok(value.instance),
                }
            }
        }

        #[derive(Clone, Debug)]
        pub struct RegisterServerResponse {
            registration: ::std::result::Result<super::ServerRegistration, ::std::string::String>,
        }

        impl ::std::default::Default for RegisterServerResponse {
            fn default() -> Self {
                Self {
                    registration: Err("no value supplied for registration".to_string()),
                }
            }
        }

        impl RegisterServerResponse {
            pub fn registration<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<super::ServerRegistration>,
                T::Error: ::std::fmt::Display,
            {
                self.registration = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for registration: {e}"));
                self
            }
        }

        impl ::std::convert::TryFrom<RegisterServerResponse> for super::RegisterServerResponse {
            type Error = super::error::ConversionError;
            fn try_from(value: RegisterServerResponse) -> ::std::result::Result<Self, super::error::ConversionError> {
                Ok(Self {
                    registration: value.registration?,
                })
            }
        }

        impl ::std::convert::From<super::RegisterServerResponse> for RegisterServerResponse {
            fn from(value: super::RegisterServerResponse) -> Self {
                Self {
                    registration: Ok(value.registration),
                }
            }
        }

        #[derive(Clone, Debug)]
        pub struct ServerAttestation {
            attestation: ::std::result::Result<::serde_json::Value, ::std::string::String>,
        }

        impl ::std::default::Default for ServerAttestation {
            fn default() -> Self {
                Self {
                    attestation: Err("no value supplied for attestation".to_string()),
                }
            }
        }

        impl ServerAttestation {
            pub fn attestation<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::serde_json::Value>,
                T::Error: ::std::fmt::Display,
            {
                self.attestation = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for attestation: {e}"));
                self
            }
        }

        impl ::std::convert::TryFrom<ServerAttestation> for super::ServerAttestation {
            type Error = super::error::ConversionError;
            fn try_from(value: ServerAttestation) -> ::std::result::Result<Self, super::error::ConversionError> {
                Ok(Self {
                    attestation: value.attestation?,
                })
            }
        }

        impl ::std::convert::From<super::ServerAttestation> for ServerAttestation {
            fn from(value: super::ServerAttestation) -> Self {
                Self {
                    attestation: Ok(value.attestation),
                }
            }
        }

        #[derive(Clone, Debug)]
        pub struct ServerRegistration {
            created_at: ::std::result::Result<::chrono::DateTime<::chrono::offset::Utc>, ::std::string::String>,
            id: ::std::result::Result<super::TypedUuidForServerRegistrationId, ::std::string::String>,
            instance_id: ::std::result::Result<super::TypedUuidForServerRegistrationInstanceId, ::std::string::String>,
            service_id: ::std::result::Result<super::TypedUuidForServiceId, ::std::string::String>,
            state: ::std::result::Result<super::ServerRegistrationState, ::std::string::String>,
            updated_at: ::std::result::Result<::chrono::DateTime<::chrono::offset::Utc>, ::std::string::String>,
        }

        impl ::std::default::Default for ServerRegistration {
            fn default() -> Self {
                Self {
                    created_at: Err("no value supplied for created_at".to_string()),
                    id: Err("no value supplied for id".to_string()),
                    instance_id: Err("no value supplied for instance_id".to_string()),
                    service_id: Err("no value supplied for service_id".to_string()),
                    state: Err("no value supplied for state".to_string()),
                    updated_at: Err("no value supplied for updated_at".to_string()),
                }
            }
        }

        impl ServerRegistration {
            pub fn created_at<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::chrono::DateTime<::chrono::offset::Utc>>,
                T::Error: ::std::fmt::Display,
            {
                self.created_at = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for created_at: {e}"));
                self
            }
            pub fn id<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<super::TypedUuidForServerRegistrationId>,
                T::Error: ::std::fmt::Display,
            {
                self.id = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for id: {e}"));
                self
            }
            pub fn instance_id<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<super::TypedUuidForServerRegistrationInstanceId>,
                T::Error: ::std::fmt::Display,
            {
                self.instance_id = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for instance_id: {e}"));
                self
            }
            pub fn service_id<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<super::TypedUuidForServiceId>,
                T::Error: ::std::fmt::Display,
            {
                self.service_id = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for service_id: {e}"));
                self
            }
            pub fn state<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<super::ServerRegistrationState>,
                T::Error: ::std::fmt::Display,
            {
                self.state = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for state: {e}"));
                self
            }
            pub fn updated_at<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::chrono::DateTime<::chrono::offset::Utc>>,
                T::Error: ::std::fmt::Display,
            {
                self.updated_at = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for updated_at: {e}"));
                self
            }
        }

        impl ::std::convert::TryFrom<ServerRegistration> for super::ServerRegistration {
            type Error = super::error::ConversionError;
            fn try_from(value: ServerRegistration) -> ::std::result::Result<Self, super::error::ConversionError> {
                Ok(Self {
                    created_at: value.created_at?,
                    id: value.id?,
                    instance_id: value.instance_id?,
                    service_id: value.service_id?,
                    state: value.state?,
                    updated_at: value.updated_at?,
                })
            }
        }

        impl ::std::convert::From<super::ServerRegistration> for ServerRegistration {
            fn from(value: super::ServerRegistration) -> Self {
                Self {
                    created_at: Ok(value.created_at),
                    id: Ok(value.id),
                    instance_id: Ok(value.instance_id),
                    service_id: Ok(value.service_id),
                    state: Ok(value.state),
                    updated_at: Ok(value.updated_at),
                }
            }
        }
    }
}

#[derive(Clone, Debug)]
/// Client for OXVM Services API
///
/// Shared Oxide VM support services
///
/// Version: 0.1.0
pub struct Client {
    pub(crate) baseurl: String,
    pub(crate) client: reqwest::Client,
}

impl Client {
    /// Create a new client.
    ///
    /// `baseurl` is the base URL provided to the internal
    /// `reqwest::Client`, and should include a scheme and hostname,
    /// as well as port and a path stem if applicable.
    pub fn new(baseurl: &str) -> Self {
        #[cfg(not(target_arch = "wasm32"))]
        let client = {
            let dur = ::std::time::Duration::from_secs(15u64);
            reqwest::ClientBuilder::new().connect_timeout(dur).timeout(dur)
        };
        #[cfg(target_arch = "wasm32")]
        let client = reqwest::ClientBuilder::new();
        Self::new_with_client(baseurl, client.build().unwrap())
    }

    /// Construct a new client with an existing `reqwest::Client`,
    /// allowing more control over its configuration.
    ///
    /// `baseurl` is the base URL provided to the internal
    /// `reqwest::Client`, and should include a scheme and hostname,
    /// as well as port and a path stem if applicable.
    pub fn new_with_client(baseurl: &str, client: reqwest::Client) -> Self {
        Self {
            baseurl: baseurl.to_string(),
            client,
        }
    }
}

impl ClientInfo<()> for Client {
    fn api_version() -> &'static str {
        "0.1.0"
    }

    fn baseurl(&self) -> &str {
        self.baseurl.as_str()
    }

    fn client(&self) -> &reqwest::Client {
        &self.client
    }

    fn inner(&self) -> &() {
        &()
    }
}

impl ClientHooks<()> for &Client {}

impl Client {
    /// Sends a `GET` request to `/.well-known/jwks.json`
    ///
    /// ```ignore
    /// let response = client.jwks_json()
    ///    .send()
    ///    .await;
    /// ```
    pub fn jwks_json(&self) -> builder::JwksJson<'_> {
        builder::JwksJson::new(self)
    }

    /// Sends a `GET` request to `/.well-known/openid-configuration`
    ///
    /// ```ignore
    /// let response = client.openid_configuration()
    ///    .send()
    ///    .await;
    /// ```
    pub fn openid_configuration(&self) -> builder::OpenidConfiguration<'_> {
        builder::OpenidConfiguration::new(self)
    }

    /// Cancels a blob so that it can no longer be written to. Any data already
    /// sent will eventually
    ///
    /// be deleted.
    ///
    /// Sends a `POST` request to `/blob/{blob}/upload/cancel`
    ///
    /// ```ignore
    /// let response = client.cancel_blob_upload()
    ///    .blob(blob)
    ///    .send()
    ///    .await;
    /// ```
    pub fn cancel_blob_upload(&self) -> builder::CancelBlobUpload<'_> {
        builder::CancelBlobUpload::new(self)
    }

    /// Mark a blob as being fully uploaded and ready to be persisted
    ///
    /// Sends a `POST` request to `/blob/{blob}/upload/complete`
    ///
    /// ```ignore
    /// let response = client.complete_blob_upload()
    ///    .blob(blob)
    ///    .send()
    ///    .await;
    /// ```
    pub fn complete_blob_upload(&self) -> builder::CompleteBlobUpload<'_> {
        builder::CompleteBlobUpload::new(self)
    }

    /// Reset a blob to synchronously remove any data already uploaded to this
    /// blob
    ///
    /// Sends a `POST` request to `/blob/{blob}/upload/reset`
    ///
    /// ```ignore
    /// let response = client.reset_blob_upload()
    ///    .blob(blob)
    ///    .send()
    ///    .await;
    /// ```
    pub fn reset_blob_upload(&self) -> builder::ResetBlobUpload<'_> {
        builder::ResetBlobUpload::new(self)
    }

    /// Stream data to fill a registered blob. Any data that is streamed is
    /// appended to any data that
    ///
    /// has already been recieved. Concurrent writes to the same blob are not
    /// supported.
    ///
    /// Sends a `POST` request to `/blob/{blob}/upload/write`
    ///
    /// ```ignore
    /// let response = client.write_blob_upload()
    ///    .blob(blob)
    ///    .body(body)
    ///    .send()
    ///    .await;
    /// ```
    pub fn write_blob_upload(&self) -> builder::WriteBlobUpload<'_> {
        builder::WriteBlobUpload::new(self)
    }

    /// Accept a server's request to be added as a representative instance of a
    /// service
    ///
    /// Sends a `POST` request to `/server/{server}/accept`
    ///
    /// ```ignore
    /// let response = client.accept_server()
    ///    .server(server)
    ///    .send()
    ///    .await;
    /// ```
    pub fn accept_server(&self) -> builder::AcceptServer<'_> {
        builder::AcceptServer::new(self)
    }

    /// Register a new blob request to upload a blob to. Returns a blob instance
    /// that the
    ///
    /// requesting server is authorized to upload to.
    ///
    /// Sends a `POST` request to `/server/{server}/blob/register`
    ///
    /// ```ignore
    /// let response = client.register_blob()
    ///    .server(server)
    ///    .body(body)
    ///    .send()
    ///    .await;
    /// ```
    pub fn register_blob(&self) -> builder::RegisterBlob<'_> {
        builder::RegisterBlob::new(self)
    }

    /// Report a check in of a server for aliveness checks
    ///
    /// Sends a `POST` request to `/server/{server}/checkin`
    ///
    /// ```ignore
    /// let response = client.checkin_server()
    ///    .server(server)
    ///    .body(body)
    ///    .send()
    ///    .await;
    /// ```
    pub fn checkin_server(&self) -> builder::CheckinServer<'_> {
        builder::CheckinServer::new(self)
    }

    /// Sends a `POST` request to `/server/{server}/oidc/token`
    ///
    /// ```ignore
    /// let response = client.register_oidc_token_request()
    ///    .server(server)
    ///    .send()
    ///    .await;
    /// ```
    pub fn register_oidc_token_request(&self) -> builder::RegisterOidcTokenRequest<'_> {
        builder::RegisterOidcTokenRequest::new(self)
    }

    /// Sends a `POST` request to `/server/{server}/oidc/token/prove`
    ///
    /// ```ignore
    /// let response = client.prove_oidc_token_request()
    ///    .server(server)
    ///    .body(body)
    ///    .send()
    ///    .await;
    /// ```
    pub fn prove_oidc_token_request(&self) -> builder::ProveOidcTokenRequest<'_> {
        builder::ProveOidcTokenRequest::new(self)
    }

    /// Reject a server's request to be added as a representative instance of a
    /// service
    ///
    /// Sends a `POST` request to `/server/{server}/reject`
    ///
    /// ```ignore
    /// let response = client.reject_server()
    ///    .server(server)
    ///    .send()
    ///    .await;
    /// ```
    pub fn reject_server(&self) -> builder::RejectServer<'_> {
        builder::RejectServer::new(self)
    }

    /// Remove a server from the pool of representative instances of a service
    ///
    /// Sends a `POST` request to `/server/{server}/terminate`
    ///
    /// ```ignore
    /// let response = client.terminate_server()
    ///    .server(server)
    ///    .send()
    ///    .await;
    /// ```
    pub fn terminate_server(&self) -> builder::TerminateServer<'_> {
        builder::TerminateServer::new(self)
    }

    /// Request a server be registered as a representative instance of a
    /// service. The registration
    ///
    /// will need to be accepted before the server can begin check ins or blobs.
    ///
    /// Sends a `POST` request to `/service/{service}/register`
    ///
    /// ```ignore
    /// let response = client.register_server()
    ///    .service(service)
    ///    .body(body)
    ///    .send()
    ///    .await;
    /// ```
    pub fn register_server(&self) -> builder::RegisterServer<'_> {
        builder::RegisterServer::new(self)
    }
}

/// Types for composing operation parameters.
#[allow(clippy::all)]
pub mod builder {
    use super::types;
    #[allow(unused_imports)]
    use super::{
        encode_path, ByteStream, ClientHooks, ClientInfo, Error, OperationInfo, RequestBuilderExt, ResponseValue,
    };
    /// Builder for [`Client::jwks_json`]
    ///
    /// [`Client::jwks_json`]: super::Client::jwks_json
    #[derive(Debug, Clone)]
    pub struct JwksJson<'a> {
        client: &'a super::Client,
    }

    impl<'a> JwksJson<'a> {
        pub fn new(client: &'a super::Client) -> Self {
            Self { client: client }
        }

        /// Sends a `GET` request to `/.well-known/jwks.json`
        pub async fn send(self) -> Result<ResponseValue<types::Jwks>, Error<types::Error>> {
            let Self { client } = self;
            let url = format!("{}/.well-known/jwks.json", client.baseurl,);
            let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
            header_map.append(
                ::reqwest::header::HeaderName::from_static("api-version"),
                ::reqwest::header::HeaderValue::from_static(super::Client::api_version()),
            );
            #[allow(unused_mut)]
            let mut request = client
                .client
                .get(url)
                .header(
                    ::reqwest::header::ACCEPT,
                    ::reqwest::header::HeaderValue::from_static("application/json"),
                )
                .headers(header_map)
                .build()?;
            let info = OperationInfo {
                operation_id: "jwks_json",
            };
            client.pre(&mut request, &info).await?;
            let result = client.exec(request, &info).await;
            client.post(&result, &info).await?;
            let response = result?;
            match response.status().as_u16() {
                200u16 => ResponseValue::from_response(response).await,
                400u16..=499u16 => Err(Error::ErrorResponse(ResponseValue::from_response(response).await?)),
                500u16..=599u16 => Err(Error::ErrorResponse(ResponseValue::from_response(response).await?)),
                _ => Err(Error::UnexpectedResponse(response)),
            }
        }
    }

    /// Builder for [`Client::openid_configuration`]
    ///
    /// [`Client::openid_configuration`]: super::Client::openid_configuration
    #[derive(Debug, Clone)]
    pub struct OpenidConfiguration<'a> {
        client: &'a super::Client,
    }

    impl<'a> OpenidConfiguration<'a> {
        pub fn new(client: &'a super::Client) -> Self {
            Self { client: client }
        }

        /// Sends a `GET` request to `/.well-known/openid-configuration`
        pub async fn send(self) -> Result<ResponseValue<types::OpenIdConfiguration>, Error<types::Error>> {
            let Self { client } = self;
            let url = format!("{}/.well-known/openid-configuration", client.baseurl,);
            let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
            header_map.append(
                ::reqwest::header::HeaderName::from_static("api-version"),
                ::reqwest::header::HeaderValue::from_static(super::Client::api_version()),
            );
            #[allow(unused_mut)]
            let mut request = client
                .client
                .get(url)
                .header(
                    ::reqwest::header::ACCEPT,
                    ::reqwest::header::HeaderValue::from_static("application/json"),
                )
                .headers(header_map)
                .build()?;
            let info = OperationInfo {
                operation_id: "openid_configuration",
            };
            client.pre(&mut request, &info).await?;
            let result = client.exec(request, &info).await;
            client.post(&result, &info).await?;
            let response = result?;
            match response.status().as_u16() {
                200u16 => ResponseValue::from_response(response).await,
                400u16..=499u16 => Err(Error::ErrorResponse(ResponseValue::from_response(response).await?)),
                500u16..=599u16 => Err(Error::ErrorResponse(ResponseValue::from_response(response).await?)),
                _ => Err(Error::UnexpectedResponse(response)),
            }
        }
    }

    /// Builder for [`Client::cancel_blob_upload`]
    ///
    /// [`Client::cancel_blob_upload`]: super::Client::cancel_blob_upload
    #[derive(Debug, Clone)]
    pub struct CancelBlobUpload<'a> {
        client: &'a super::Client,
        blob: Result<types::TypedUuidForBlobId, String>,
    }

    impl<'a> CancelBlobUpload<'a> {
        pub fn new(client: &'a super::Client) -> Self {
            Self {
                client: client,
                blob: Err("blob was not initialized".to_string()),
            }
        }

        pub fn blob<V>(mut self, value: V) -> Self
        where
            V: std::convert::TryInto<types::TypedUuidForBlobId>,
        {
            self.blob = value
                .try_into()
                .map_err(|_| "conversion to `TypedUuidForBlobId` for blob failed".to_string());
            self
        }

        /// Sends a `POST` request to `/blob/{blob}/upload/cancel`
        pub async fn send(self) -> Result<ResponseValue<()>, Error<types::Error>> {
            let Self { client, blob } = self;
            let blob = blob.map_err(Error::InvalidRequest)?;
            let url = format!(
                "{}/blob/{}/upload/cancel",
                client.baseurl,
                encode_path(&blob.to_string()),
            );
            let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
            header_map.append(
                ::reqwest::header::HeaderName::from_static("api-version"),
                ::reqwest::header::HeaderValue::from_static(super::Client::api_version()),
            );
            #[allow(unused_mut)]
            let mut request = client
                .client
                .post(url)
                .header(
                    ::reqwest::header::ACCEPT,
                    ::reqwest::header::HeaderValue::from_static("application/json"),
                )
                .headers(header_map)
                .build()?;
            let info = OperationInfo {
                operation_id: "cancel_blob_upload",
            };
            client.pre(&mut request, &info).await?;
            let result = client.exec(request, &info).await;
            client.post(&result, &info).await?;
            let response = result?;
            match response.status().as_u16() {
                204u16 => Ok(ResponseValue::empty(response)),
                400u16..=499u16 => Err(Error::ErrorResponse(ResponseValue::from_response(response).await?)),
                500u16..=599u16 => Err(Error::ErrorResponse(ResponseValue::from_response(response).await?)),
                _ => Err(Error::UnexpectedResponse(response)),
            }
        }
    }

    /// Builder for [`Client::complete_blob_upload`]
    ///
    /// [`Client::complete_blob_upload`]: super::Client::complete_blob_upload
    #[derive(Debug, Clone)]
    pub struct CompleteBlobUpload<'a> {
        client: &'a super::Client,
        blob: Result<types::TypedUuidForBlobId, String>,
    }

    impl<'a> CompleteBlobUpload<'a> {
        pub fn new(client: &'a super::Client) -> Self {
            Self {
                client: client,
                blob: Err("blob was not initialized".to_string()),
            }
        }

        pub fn blob<V>(mut self, value: V) -> Self
        where
            V: std::convert::TryInto<types::TypedUuidForBlobId>,
        {
            self.blob = value
                .try_into()
                .map_err(|_| "conversion to `TypedUuidForBlobId` for blob failed".to_string());
            self
        }

        /// Sends a `POST` request to `/blob/{blob}/upload/complete`
        pub async fn send(self) -> Result<ResponseValue<()>, Error<types::Error>> {
            let Self { client, blob } = self;
            let blob = blob.map_err(Error::InvalidRequest)?;
            let url = format!(
                "{}/blob/{}/upload/complete",
                client.baseurl,
                encode_path(&blob.to_string()),
            );
            let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
            header_map.append(
                ::reqwest::header::HeaderName::from_static("api-version"),
                ::reqwest::header::HeaderValue::from_static(super::Client::api_version()),
            );
            #[allow(unused_mut)]
            let mut request = client
                .client
                .post(url)
                .header(
                    ::reqwest::header::ACCEPT,
                    ::reqwest::header::HeaderValue::from_static("application/json"),
                )
                .headers(header_map)
                .build()?;
            let info = OperationInfo {
                operation_id: "complete_blob_upload",
            };
            client.pre(&mut request, &info).await?;
            let result = client.exec(request, &info).await;
            client.post(&result, &info).await?;
            let response = result?;
            match response.status().as_u16() {
                204u16 => Ok(ResponseValue::empty(response)),
                400u16..=499u16 => Err(Error::ErrorResponse(ResponseValue::from_response(response).await?)),
                500u16..=599u16 => Err(Error::ErrorResponse(ResponseValue::from_response(response).await?)),
                _ => Err(Error::UnexpectedResponse(response)),
            }
        }
    }

    /// Builder for [`Client::reset_blob_upload`]
    ///
    /// [`Client::reset_blob_upload`]: super::Client::reset_blob_upload
    #[derive(Debug, Clone)]
    pub struct ResetBlobUpload<'a> {
        client: &'a super::Client,
        blob: Result<types::TypedUuidForBlobId, String>,
    }

    impl<'a> ResetBlobUpload<'a> {
        pub fn new(client: &'a super::Client) -> Self {
            Self {
                client: client,
                blob: Err("blob was not initialized".to_string()),
            }
        }

        pub fn blob<V>(mut self, value: V) -> Self
        where
            V: std::convert::TryInto<types::TypedUuidForBlobId>,
        {
            self.blob = value
                .try_into()
                .map_err(|_| "conversion to `TypedUuidForBlobId` for blob failed".to_string());
            self
        }

        /// Sends a `POST` request to `/blob/{blob}/upload/reset`
        pub async fn send(self) -> Result<ResponseValue<()>, Error<types::Error>> {
            let Self { client, blob } = self;
            let blob = blob.map_err(Error::InvalidRequest)?;
            let url = format!(
                "{}/blob/{}/upload/reset",
                client.baseurl,
                encode_path(&blob.to_string()),
            );
            let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
            header_map.append(
                ::reqwest::header::HeaderName::from_static("api-version"),
                ::reqwest::header::HeaderValue::from_static(super::Client::api_version()),
            );
            #[allow(unused_mut)]
            let mut request = client
                .client
                .post(url)
                .header(
                    ::reqwest::header::ACCEPT,
                    ::reqwest::header::HeaderValue::from_static("application/json"),
                )
                .headers(header_map)
                .build()?;
            let info = OperationInfo {
                operation_id: "reset_blob_upload",
            };
            client.pre(&mut request, &info).await?;
            let result = client.exec(request, &info).await;
            client.post(&result, &info).await?;
            let response = result?;
            match response.status().as_u16() {
                204u16 => Ok(ResponseValue::empty(response)),
                400u16..=499u16 => Err(Error::ErrorResponse(ResponseValue::from_response(response).await?)),
                500u16..=599u16 => Err(Error::ErrorResponse(ResponseValue::from_response(response).await?)),
                _ => Err(Error::UnexpectedResponse(response)),
            }
        }
    }

    /// Builder for [`Client::write_blob_upload`]
    ///
    /// [`Client::write_blob_upload`]: super::Client::write_blob_upload
    #[derive(Debug)]
    pub struct WriteBlobUpload<'a> {
        client: &'a super::Client,
        blob: Result<types::TypedUuidForBlobId, String>,
        body: Result<reqwest::Body, String>,
    }

    impl<'a> WriteBlobUpload<'a> {
        pub fn new(client: &'a super::Client) -> Self {
            Self {
                client: client,
                blob: Err("blob was not initialized".to_string()),
                body: Err("body was not initialized".to_string()),
            }
        }

        pub fn blob<V>(mut self, value: V) -> Self
        where
            V: std::convert::TryInto<types::TypedUuidForBlobId>,
        {
            self.blob = value
                .try_into()
                .map_err(|_| "conversion to `TypedUuidForBlobId` for blob failed".to_string());
            self
        }

        pub fn body<B>(mut self, value: B) -> Self
        where
            B: std::convert::TryInto<reqwest::Body>,
        {
            self.body = value
                .try_into()
                .map_err(|_| "conversion to `reqwest::Body` for body failed".to_string());
            self
        }

        /// Sends a `POST` request to `/blob/{blob}/upload/write`
        pub async fn send(self) -> Result<ResponseValue<()>, Error<types::Error>> {
            let Self { client, blob, body } = self;
            let blob = blob.map_err(Error::InvalidRequest)?;
            let body = body.map_err(Error::InvalidRequest)?;
            let url = format!(
                "{}/blob/{}/upload/write",
                client.baseurl,
                encode_path(&blob.to_string()),
            );
            let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
            header_map.append(
                ::reqwest::header::HeaderName::from_static("api-version"),
                ::reqwest::header::HeaderValue::from_static(super::Client::api_version()),
            );
            #[allow(unused_mut)]
            let mut request = client
                .client
                .post(url)
                .header(
                    ::reqwest::header::ACCEPT,
                    ::reqwest::header::HeaderValue::from_static("application/json"),
                )
                .header(
                    ::reqwest::header::CONTENT_TYPE,
                    ::reqwest::header::HeaderValue::from_static("application/octet-stream"),
                )
                .body(body)
                .headers(header_map)
                .build()?;
            let info = OperationInfo {
                operation_id: "write_blob_upload",
            };
            client.pre(&mut request, &info).await?;
            let result = client.exec(request, &info).await;
            client.post(&result, &info).await?;
            let response = result?;
            match response.status().as_u16() {
                204u16 => Ok(ResponseValue::empty(response)),
                400u16..=499u16 => Err(Error::ErrorResponse(ResponseValue::from_response(response).await?)),
                500u16..=599u16 => Err(Error::ErrorResponse(ResponseValue::from_response(response).await?)),
                _ => Err(Error::UnexpectedResponse(response)),
            }
        }
    }

    /// Builder for [`Client::accept_server`]
    ///
    /// [`Client::accept_server`]: super::Client::accept_server
    #[derive(Debug, Clone)]
    pub struct AcceptServer<'a> {
        client: &'a super::Client,
        server: Result<types::TypedUuidForServerRegistrationId, String>,
    }

    impl<'a> AcceptServer<'a> {
        pub fn new(client: &'a super::Client) -> Self {
            Self {
                client: client,
                server: Err("server was not initialized".to_string()),
            }
        }

        pub fn server<V>(mut self, value: V) -> Self
        where
            V: std::convert::TryInto<types::TypedUuidForServerRegistrationId>,
        {
            self.server = value
                .try_into()
                .map_err(|_| "conversion to `TypedUuidForServerRegistrationId` for server failed".to_string());
            self
        }

        /// Sends a `POST` request to `/server/{server}/accept`
        pub async fn send(self) -> Result<ResponseValue<()>, Error<types::Error>> {
            let Self { client, server } = self;
            let server = server.map_err(Error::InvalidRequest)?;
            let url = format!("{}/server/{}/accept", client.baseurl, encode_path(&server.to_string()),);
            let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
            header_map.append(
                ::reqwest::header::HeaderName::from_static("api-version"),
                ::reqwest::header::HeaderValue::from_static(super::Client::api_version()),
            );
            #[allow(unused_mut)]
            let mut request = client
                .client
                .post(url)
                .header(
                    ::reqwest::header::ACCEPT,
                    ::reqwest::header::HeaderValue::from_static("application/json"),
                )
                .headers(header_map)
                .build()?;
            let info = OperationInfo {
                operation_id: "accept_server",
            };
            client.pre(&mut request, &info).await?;
            let result = client.exec(request, &info).await;
            client.post(&result, &info).await?;
            let response = result?;
            match response.status().as_u16() {
                204u16 => Ok(ResponseValue::empty(response)),
                400u16..=499u16 => Err(Error::ErrorResponse(ResponseValue::from_response(response).await?)),
                500u16..=599u16 => Err(Error::ErrorResponse(ResponseValue::from_response(response).await?)),
                _ => Err(Error::UnexpectedResponse(response)),
            }
        }
    }

    /// Builder for [`Client::register_blob`]
    ///
    /// [`Client::register_blob`]: super::Client::register_blob
    #[derive(Debug, Clone)]
    pub struct RegisterBlob<'a> {
        client: &'a super::Client,
        server: Result<types::TypedUuidForServerRegistrationId, String>,
        body: Result<types::builder::RegisterBlobBody, String>,
    }

    impl<'a> RegisterBlob<'a> {
        pub fn new(client: &'a super::Client) -> Self {
            Self {
                client: client,
                server: Err("server was not initialized".to_string()),
                body: Ok(::std::default::Default::default()),
            }
        }

        pub fn server<V>(mut self, value: V) -> Self
        where
            V: std::convert::TryInto<types::TypedUuidForServerRegistrationId>,
        {
            self.server = value
                .try_into()
                .map_err(|_| "conversion to `TypedUuidForServerRegistrationId` for server failed".to_string());
            self
        }

        pub fn body<V>(mut self, value: V) -> Self
        where
            V: std::convert::TryInto<types::RegisterBlobBody>,
            <V as std::convert::TryInto<types::RegisterBlobBody>>::Error: std::fmt::Display,
        {
            self.body = value
                .try_into()
                .map(From::from)
                .map_err(|s| format!("conversion to `RegisterBlobBody` for body failed: {}", s));
            self
        }

        pub fn body_map<F>(mut self, f: F) -> Self
        where
            F: std::ops::FnOnce(types::builder::RegisterBlobBody) -> types::builder::RegisterBlobBody,
        {
            self.body = self.body.map(f);
            self
        }

        /// Sends a `POST` request to `/server/{server}/blob/register`
        pub async fn send(self) -> Result<ResponseValue<types::RegisterBlobResponse>, Error<types::Error>> {
            let Self { client, server, body } = self;
            let server = server.map_err(Error::InvalidRequest)?;
            let body = body
                .and_then(|v| types::RegisterBlobBody::try_from(v).map_err(|e| e.to_string()))
                .map_err(Error::InvalidRequest)?;
            let url = format!(
                "{}/server/{}/blob/register",
                client.baseurl,
                encode_path(&server.to_string()),
            );
            let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
            header_map.append(
                ::reqwest::header::HeaderName::from_static("api-version"),
                ::reqwest::header::HeaderValue::from_static(super::Client::api_version()),
            );
            #[allow(unused_mut)]
            let mut request = client
                .client
                .post(url)
                .header(
                    ::reqwest::header::ACCEPT,
                    ::reqwest::header::HeaderValue::from_static("application/json"),
                )
                .json(&body)
                .headers(header_map)
                .build()?;
            let info = OperationInfo {
                operation_id: "register_blob",
            };
            client.pre(&mut request, &info).await?;
            let result = client.exec(request, &info).await;
            client.post(&result, &info).await?;
            let response = result?;
            match response.status().as_u16() {
                200u16 => ResponseValue::from_response(response).await,
                400u16..=499u16 => Err(Error::ErrorResponse(ResponseValue::from_response(response).await?)),
                500u16..=599u16 => Err(Error::ErrorResponse(ResponseValue::from_response(response).await?)),
                _ => Err(Error::UnexpectedResponse(response)),
            }
        }
    }

    /// Builder for [`Client::checkin_server`]
    ///
    /// [`Client::checkin_server`]: super::Client::checkin_server
    #[derive(Debug, Clone)]
    pub struct CheckinServer<'a> {
        client: &'a super::Client,
        server: Result<types::TypedUuidForServerRegistrationId, String>,
        body: Result<types::builder::CheckinBody, String>,
    }

    impl<'a> CheckinServer<'a> {
        pub fn new(client: &'a super::Client) -> Self {
            Self {
                client: client,
                server: Err("server was not initialized".to_string()),
                body: Ok(::std::default::Default::default()),
            }
        }

        pub fn server<V>(mut self, value: V) -> Self
        where
            V: std::convert::TryInto<types::TypedUuidForServerRegistrationId>,
        {
            self.server = value
                .try_into()
                .map_err(|_| "conversion to `TypedUuidForServerRegistrationId` for server failed".to_string());
            self
        }

        pub fn body<V>(mut self, value: V) -> Self
        where
            V: std::convert::TryInto<types::CheckinBody>,
            <V as std::convert::TryInto<types::CheckinBody>>::Error: std::fmt::Display,
        {
            self.body = value
                .try_into()
                .map(From::from)
                .map_err(|s| format!("conversion to `CheckinBody` for body failed: {}", s));
            self
        }

        pub fn body_map<F>(mut self, f: F) -> Self
        where
            F: std::ops::FnOnce(types::builder::CheckinBody) -> types::builder::CheckinBody,
        {
            self.body = self.body.map(f);
            self
        }

        /// Sends a `POST` request to `/server/{server}/checkin`
        pub async fn send(self) -> Result<ResponseValue<types::HealthCheck>, Error<types::Error>> {
            let Self { client, server, body } = self;
            let server = server.map_err(Error::InvalidRequest)?;
            let body = body
                .and_then(|v| types::CheckinBody::try_from(v).map_err(|e| e.to_string()))
                .map_err(Error::InvalidRequest)?;
            let url = format!("{}/server/{}/checkin", client.baseurl, encode_path(&server.to_string()),);
            let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
            header_map.append(
                ::reqwest::header::HeaderName::from_static("api-version"),
                ::reqwest::header::HeaderValue::from_static(super::Client::api_version()),
            );
            #[allow(unused_mut)]
            let mut request = client
                .client
                .post(url)
                .header(
                    ::reqwest::header::ACCEPT,
                    ::reqwest::header::HeaderValue::from_static("application/json"),
                )
                .json(&body)
                .headers(header_map)
                .build()?;
            let info = OperationInfo {
                operation_id: "checkin_server",
            };
            client.pre(&mut request, &info).await?;
            let result = client.exec(request, &info).await;
            client.post(&result, &info).await?;
            let response = result?;
            match response.status().as_u16() {
                200u16 => ResponseValue::from_response(response).await,
                400u16..=499u16 => Err(Error::ErrorResponse(ResponseValue::from_response(response).await?)),
                500u16..=599u16 => Err(Error::ErrorResponse(ResponseValue::from_response(response).await?)),
                _ => Err(Error::UnexpectedResponse(response)),
            }
        }
    }

    /// Builder for [`Client::register_oidc_token_request`]
    ///
    /// [`Client::register_oidc_token_request`]: super::Client::register_oidc_token_request
    #[derive(Debug, Clone)]
    pub struct RegisterOidcTokenRequest<'a> {
        client: &'a super::Client,
        server: Result<types::TypedUuidForServerRegistrationId, String>,
    }

    impl<'a> RegisterOidcTokenRequest<'a> {
        pub fn new(client: &'a super::Client) -> Self {
            Self {
                client: client,
                server: Err("server was not initialized".to_string()),
            }
        }

        pub fn server<V>(mut self, value: V) -> Self
        where
            V: std::convert::TryInto<types::TypedUuidForServerRegistrationId>,
        {
            self.server = value
                .try_into()
                .map_err(|_| "conversion to `TypedUuidForServerRegistrationId` for server failed".to_string());
            self
        }

        /// Sends a `POST` request to `/server/{server}/oidc/token`
        pub async fn send(self) -> Result<ResponseValue<types::OidcServerTokenNonce>, Error<types::Error>> {
            let Self { client, server } = self;
            let server = server.map_err(Error::InvalidRequest)?;
            let url = format!(
                "{}/server/{}/oidc/token",
                client.baseurl,
                encode_path(&server.to_string()),
            );
            let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
            header_map.append(
                ::reqwest::header::HeaderName::from_static("api-version"),
                ::reqwest::header::HeaderValue::from_static(super::Client::api_version()),
            );
            #[allow(unused_mut)]
            let mut request = client
                .client
                .post(url)
                .header(
                    ::reqwest::header::ACCEPT,
                    ::reqwest::header::HeaderValue::from_static("application/json"),
                )
                .headers(header_map)
                .build()?;
            let info = OperationInfo {
                operation_id: "register_oidc_token_request",
            };
            client.pre(&mut request, &info).await?;
            let result = client.exec(request, &info).await;
            client.post(&result, &info).await?;
            let response = result?;
            match response.status().as_u16() {
                200u16 => ResponseValue::from_response(response).await,
                400u16..=499u16 => Err(Error::ErrorResponse(ResponseValue::from_response(response).await?)),
                500u16..=599u16 => Err(Error::ErrorResponse(ResponseValue::from_response(response).await?)),
                _ => Err(Error::UnexpectedResponse(response)),
            }
        }
    }

    /// Builder for [`Client::prove_oidc_token_request`]
    ///
    /// [`Client::prove_oidc_token_request`]: super::Client::prove_oidc_token_request
    #[derive(Debug, Clone)]
    pub struct ProveOidcTokenRequest<'a> {
        client: &'a super::Client,
        server: Result<types::TypedUuidForServerRegistrationId, String>,
        body: Result<types::builder::ServerAttestation, String>,
    }

    impl<'a> ProveOidcTokenRequest<'a> {
        pub fn new(client: &'a super::Client) -> Self {
            Self {
                client: client,
                server: Err("server was not initialized".to_string()),
                body: Ok(::std::default::Default::default()),
            }
        }

        pub fn server<V>(mut self, value: V) -> Self
        where
            V: std::convert::TryInto<types::TypedUuidForServerRegistrationId>,
        {
            self.server = value
                .try_into()
                .map_err(|_| "conversion to `TypedUuidForServerRegistrationId` for server failed".to_string());
            self
        }

        pub fn body<V>(mut self, value: V) -> Self
        where
            V: std::convert::TryInto<types::ServerAttestation>,
            <V as std::convert::TryInto<types::ServerAttestation>>::Error: std::fmt::Display,
        {
            self.body = value
                .try_into()
                .map(From::from)
                .map_err(|s| format!("conversion to `ServerAttestation` for body failed: {}", s));
            self
        }

        pub fn body_map<F>(mut self, f: F) -> Self
        where
            F: std::ops::FnOnce(types::builder::ServerAttestation) -> types::builder::ServerAttestation,
        {
            self.body = self.body.map(f);
            self
        }

        /// Sends a `POST` request to `/server/{server}/oidc/token/prove`
        pub async fn send(self) -> Result<ResponseValue<types::OidcServerToken>, Error<types::Error>> {
            let Self { client, server, body } = self;
            let server = server.map_err(Error::InvalidRequest)?;
            let body = body
                .and_then(|v| types::ServerAttestation::try_from(v).map_err(|e| e.to_string()))
                .map_err(Error::InvalidRequest)?;
            let url = format!(
                "{}/server/{}/oidc/token/prove",
                client.baseurl,
                encode_path(&server.to_string()),
            );
            let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
            header_map.append(
                ::reqwest::header::HeaderName::from_static("api-version"),
                ::reqwest::header::HeaderValue::from_static(super::Client::api_version()),
            );
            #[allow(unused_mut)]
            let mut request = client
                .client
                .post(url)
                .header(
                    ::reqwest::header::ACCEPT,
                    ::reqwest::header::HeaderValue::from_static("application/json"),
                )
                .json(&body)
                .headers(header_map)
                .build()?;
            let info = OperationInfo {
                operation_id: "prove_oidc_token_request",
            };
            client.pre(&mut request, &info).await?;
            let result = client.exec(request, &info).await;
            client.post(&result, &info).await?;
            let response = result?;
            match response.status().as_u16() {
                200u16 => ResponseValue::from_response(response).await,
                400u16..=499u16 => Err(Error::ErrorResponse(ResponseValue::from_response(response).await?)),
                500u16..=599u16 => Err(Error::ErrorResponse(ResponseValue::from_response(response).await?)),
                _ => Err(Error::UnexpectedResponse(response)),
            }
        }
    }

    /// Builder for [`Client::reject_server`]
    ///
    /// [`Client::reject_server`]: super::Client::reject_server
    #[derive(Debug, Clone)]
    pub struct RejectServer<'a> {
        client: &'a super::Client,
        server: Result<types::TypedUuidForServerRegistrationId, String>,
    }

    impl<'a> RejectServer<'a> {
        pub fn new(client: &'a super::Client) -> Self {
            Self {
                client: client,
                server: Err("server was not initialized".to_string()),
            }
        }

        pub fn server<V>(mut self, value: V) -> Self
        where
            V: std::convert::TryInto<types::TypedUuidForServerRegistrationId>,
        {
            self.server = value
                .try_into()
                .map_err(|_| "conversion to `TypedUuidForServerRegistrationId` for server failed".to_string());
            self
        }

        /// Sends a `POST` request to `/server/{server}/reject`
        pub async fn send(self) -> Result<ResponseValue<()>, Error<types::Error>> {
            let Self { client, server } = self;
            let server = server.map_err(Error::InvalidRequest)?;
            let url = format!("{}/server/{}/reject", client.baseurl, encode_path(&server.to_string()),);
            let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
            header_map.append(
                ::reqwest::header::HeaderName::from_static("api-version"),
                ::reqwest::header::HeaderValue::from_static(super::Client::api_version()),
            );
            #[allow(unused_mut)]
            let mut request = client
                .client
                .post(url)
                .header(
                    ::reqwest::header::ACCEPT,
                    ::reqwest::header::HeaderValue::from_static("application/json"),
                )
                .headers(header_map)
                .build()?;
            let info = OperationInfo {
                operation_id: "reject_server",
            };
            client.pre(&mut request, &info).await?;
            let result = client.exec(request, &info).await;
            client.post(&result, &info).await?;
            let response = result?;
            match response.status().as_u16() {
                204u16 => Ok(ResponseValue::empty(response)),
                400u16..=499u16 => Err(Error::ErrorResponse(ResponseValue::from_response(response).await?)),
                500u16..=599u16 => Err(Error::ErrorResponse(ResponseValue::from_response(response).await?)),
                _ => Err(Error::UnexpectedResponse(response)),
            }
        }
    }

    /// Builder for [`Client::terminate_server`]
    ///
    /// [`Client::terminate_server`]: super::Client::terminate_server
    #[derive(Debug, Clone)]
    pub struct TerminateServer<'a> {
        client: &'a super::Client,
        server: Result<types::TypedUuidForServerRegistrationId, String>,
    }

    impl<'a> TerminateServer<'a> {
        pub fn new(client: &'a super::Client) -> Self {
            Self {
                client: client,
                server: Err("server was not initialized".to_string()),
            }
        }

        pub fn server<V>(mut self, value: V) -> Self
        where
            V: std::convert::TryInto<types::TypedUuidForServerRegistrationId>,
        {
            self.server = value
                .try_into()
                .map_err(|_| "conversion to `TypedUuidForServerRegistrationId` for server failed".to_string());
            self
        }

        /// Sends a `POST` request to `/server/{server}/terminate`
        pub async fn send(self) -> Result<ResponseValue<()>, Error<types::Error>> {
            let Self { client, server } = self;
            let server = server.map_err(Error::InvalidRequest)?;
            let url = format!(
                "{}/server/{}/terminate",
                client.baseurl,
                encode_path(&server.to_string()),
            );
            let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
            header_map.append(
                ::reqwest::header::HeaderName::from_static("api-version"),
                ::reqwest::header::HeaderValue::from_static(super::Client::api_version()),
            );
            #[allow(unused_mut)]
            let mut request = client
                .client
                .post(url)
                .header(
                    ::reqwest::header::ACCEPT,
                    ::reqwest::header::HeaderValue::from_static("application/json"),
                )
                .headers(header_map)
                .build()?;
            let info = OperationInfo {
                operation_id: "terminate_server",
            };
            client.pre(&mut request, &info).await?;
            let result = client.exec(request, &info).await;
            client.post(&result, &info).await?;
            let response = result?;
            match response.status().as_u16() {
                204u16 => Ok(ResponseValue::empty(response)),
                400u16..=499u16 => Err(Error::ErrorResponse(ResponseValue::from_response(response).await?)),
                500u16..=599u16 => Err(Error::ErrorResponse(ResponseValue::from_response(response).await?)),
                _ => Err(Error::UnexpectedResponse(response)),
            }
        }
    }

    /// Builder for [`Client::register_server`]
    ///
    /// [`Client::register_server`]: super::Client::register_server
    #[derive(Debug, Clone)]
    pub struct RegisterServer<'a> {
        client: &'a super::Client,
        service: Result<::std::string::String, String>,
        body: Result<types::builder::RegisterServerBody, String>,
    }

    impl<'a> RegisterServer<'a> {
        pub fn new(client: &'a super::Client) -> Self {
            Self {
                client: client,
                service: Err("service was not initialized".to_string()),
                body: Ok(::std::default::Default::default()),
            }
        }

        pub fn service<V>(mut self, value: V) -> Self
        where
            V: std::convert::TryInto<::std::string::String>,
        {
            self.service = value
                .try_into()
                .map_err(|_| "conversion to `:: std :: string :: String` for service failed".to_string());
            self
        }

        pub fn body<V>(mut self, value: V) -> Self
        where
            V: std::convert::TryInto<types::RegisterServerBody>,
            <V as std::convert::TryInto<types::RegisterServerBody>>::Error: std::fmt::Display,
        {
            self.body = value
                .try_into()
                .map(From::from)
                .map_err(|s| format!("conversion to `RegisterServerBody` for body failed: {}", s));
            self
        }

        pub fn body_map<F>(mut self, f: F) -> Self
        where
            F: std::ops::FnOnce(types::builder::RegisterServerBody) -> types::builder::RegisterServerBody,
        {
            self.body = self.body.map(f);
            self
        }

        /// Sends a `POST` request to `/service/{service}/register`
        pub async fn send(self) -> Result<ResponseValue<types::RegisterServerResponse>, Error<types::Error>> {
            let Self { client, service, body } = self;
            let service = service.map_err(Error::InvalidRequest)?;
            let body = body
                .and_then(|v| types::RegisterServerBody::try_from(v).map_err(|e| e.to_string()))
                .map_err(Error::InvalidRequest)?;
            let url = format!(
                "{}/service/{}/register",
                client.baseurl,
                encode_path(&service.to_string()),
            );
            let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
            header_map.append(
                ::reqwest::header::HeaderName::from_static("api-version"),
                ::reqwest::header::HeaderValue::from_static(super::Client::api_version()),
            );
            #[allow(unused_mut)]
            let mut request = client
                .client
                .post(url)
                .header(
                    ::reqwest::header::ACCEPT,
                    ::reqwest::header::HeaderValue::from_static("application/json"),
                )
                .json(&body)
                .headers(header_map)
                .build()?;
            let info = OperationInfo {
                operation_id: "register_server",
            };
            client.pre(&mut request, &info).await?;
            let result = client.exec(request, &info).await;
            client.post(&result, &info).await?;
            let response = result?;
            match response.status().as_u16() {
                200u16 => ResponseValue::from_response(response).await,
                400u16..=499u16 => Err(Error::ErrorResponse(ResponseValue::from_response(response).await?)),
                500u16..=599u16 => Err(Error::ErrorResponse(ResponseValue::from_response(response).await?)),
                _ => Err(Error::UnexpectedResponse(response)),
            }
        }
    }
}

/// Items consumers will typically use such as the Client and
/// extension traits.
pub mod prelude {
    #[allow(unused_imports)]
    pub use super::Client;
}
