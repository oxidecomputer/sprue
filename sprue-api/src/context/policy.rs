// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use cedar_policy::{
    Authorizer, CedarSchemaError, Context, Decision, Entities, Entity, ParseErrors, PolicySet,
    Request, RequestValidationError, Response, Schema, entities_errors::EntitiesError,
};
use sprue_model::{Deployment, ServerRegistration, Service};
use std::str::FromStr;
use thiserror::Error;

use crate::policy::{
    PolicyError, action::REGISTER_SERVER, principal::ServerPrincipal, resource::ServiceResource,
};

#[derive(Debug, Error)]
pub enum PolicyEngineError {
    #[error("Failed to construct Cedar context")]
    Context(String),
    #[error("Failed to construct entities set for policy evaluation")]
    EntitySetConstruction(#[from] EntitiesError),
    #[error("Failed to construct entitie for policy evaluation")]
    PolicyConstruct(PolicyError),
    #[error("Failed to parse Cedar policy")]
    PolicyParse(#[from] ParseErrors),
    #[error("Failed to construct Cedar request")]
    Request(#[from] RequestValidationError),
    #[error("Failed to parse Cedar schema")]
    SchemaParse(#[from] CedarSchemaError),
}

/// The result of a policy evaluation for a server registration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyDecision {
    /// The policy explicitly allows the registration.
    Accept,
    /// The policy explicitly denies the registration.
    Reject,
}

/// A Cedar-based policy engine for evaluating server registration decisions.
///
/// Policies are loaded once from configuration at startup.
#[derive(Clone)]
pub struct PolicyEngine {
    policy_set: PolicySet,
    schema: Schema,
    authorizer: Authorizer,
}

impl PolicyEngine {
    /// Create a new policy engine from Cedar policy and schema text
    pub fn new(policy_src: &str, schema: &str) -> Result<Self, PolicyEngineError> {
        let policy_set = policy_src.parse::<PolicySet>()?;
        let schema = Schema::from_str(schema)?;

        tracing::info!(
            policy_count = policy_set.policies().count(),
            "Loaded Cedar registration policies"
        );

        Ok(Self {
            policy_set,
            schema,
            authorizer: Authorizer::new(),
        })
    }

    /// Evaluate whether a server registration should be automatically accepted
    pub fn evaluate_server_auto_registration<'a>(
        &self,
        server: &ServerRegistration,
        service: &Service,
        deployments: impl IntoIterator<Item = &'a Deployment>,
    ) -> Result<PolicyDecision, PolicyEngineError> {
        let service_id = service.id;
        let instance_id = server.instance_id;

        let principal: Entity = ServerPrincipal::from(server)
            .try_into()
            .map_err(PolicyEngineError::PolicyConstruct)?;
        let action = REGISTER_SERVER.clone();
        let resource: Entity = ServiceResource::new(service, deployments)
            .try_into()
            .map_err(PolicyEngineError::PolicyConstruct)?;

        let request = Request::new(
            principal.uid(),
            action,
            resource.uid(),
            Context::empty(),
            None,
        )?;

        let entities = Entities::from_entities([principal, resource], Some(&self.schema))?;
        let response: Response =
            self.authorizer
                .is_authorized(&request, &self.policy_set, &entities);

        match response.decision() {
            Decision::Allow => {
                tracing::info!(
                    ?service_id,
                    ?instance_id,
                    "Policy engine accepted server registration"
                );
                Ok(PolicyDecision::Accept)
            }
            Decision::Deny => {
                let reasons: Vec<String> = response
                    .diagnostics()
                    .errors()
                    .map(|e| e.to_string())
                    .collect();
                if !reasons.is_empty() {
                    tracing::info!(
                        ?service_id,
                        ?instance_id,
                        ?reasons,
                        "Policy engine rejected server registration with errors"
                    );
                }
                tracing::info!(
                    ?service_id,
                    ?instance_id,
                    "Policy engine rejected server registration"
                );
                Ok(PolicyDecision::Reject)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use newtype_uuid::{GenericUuid, TypedUuid};
    use sprue_model::{
        Deployment, DeploymentId, ProjectId, ServerRegistration, ServerRegistrationId,
        ServerRegistrationInstanceId, ServerRegistrationState, Service, ServiceId, SiloId,
    };

    const SCHEMA: &str = r#"
        namespace Sprue {
            entity Project;
            entity Silo;
            entity Instance {
                project: Project,
                silo: Silo,
            };
            entity Service {
                deployments: Set<{
                    "project": Project,
                    "silo": Silo,
                }>,
            };
            action registerServer
                appliesTo { principal: [Instance], resource: [Service] };
        }
    "#;

    fn new_id<T: newtype_uuid::TypedUuidKind>() -> TypedUuid<T> {
        TypedUuid::from_untyped_uuid(uuid::Uuid::new_v4())
    }

    fn make_service() -> Service {
        Service {
            id: new_id::<ServiceId>(),
            name: "test-service".to_string(),
            created_at: Utc::now(),
        }
    }

    fn make_server(service: &Service) -> ServerRegistration {
        ServerRegistration {
            id: new_id::<ServerRegistrationId>(),
            service_id: service.id,
            instance_id: new_id::<ServerRegistrationInstanceId>(),
            project_id: new_id::<ProjectId>(),
            silo_id: new_id::<SiloId>(),
            nonce: None,
            state: ServerRegistrationState::Proven,
            expires_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn make_deployment(service: &Service, server: &ServerRegistration) -> Deployment {
        Deployment {
            id: new_id::<DeploymentId>(),
            service_id: service.id,
            project_id: server.project_id,
            silo_id: server.silo_id,
            created_at: Utc::now(),
        }
    }

    fn engine(policy: &str) -> PolicyEngine {
        PolicyEngine::new(policy, SCHEMA).unwrap()
    }

    #[test]
    fn permit_all_policy_accepts() {
        let engine =
            engine(r#"permit(principal, action == Sprue::Action::"registerServer", resource);"#);
        let service = make_service();
        let server = make_server(&service);

        let decision = engine
            .evaluate_server_auto_registration(&server, &service, &[])
            .unwrap();
        assert_eq!(decision, PolicyDecision::Accept);
    }

    #[test]
    fn empty_policy_denies() {
        let engine = engine("");
        let service = make_service();
        let server = make_server(&service);

        let decision = engine
            .evaluate_server_auto_registration(&server, &service, &[])
            .unwrap();
        assert_eq!(decision, PolicyDecision::Reject);
    }

    #[test]
    fn explicit_forbid_denies() {
        let engine =
            engine(r#"forbid(principal, action == Sprue::Action::"registerServer", resource);"#);
        let service = make_service();
        let server = make_server(&service);

        let decision = engine
            .evaluate_server_auto_registration(&server, &service, &[])
            .unwrap();
        assert_eq!(decision, PolicyDecision::Reject);
    }

    #[test]
    fn forbid_overrides_permit() {
        let engine = engine(
            r#"
            permit(principal, action == Sprue::Action::"registerServer", resource);
            forbid(principal, action == Sprue::Action::"registerServer", resource);
            "#,
        );
        let service = make_service();
        let server = make_server(&service);

        let decision = engine
            .evaluate_server_auto_registration(&server, &service, &[])
            .unwrap();
        assert_eq!(decision, PolicyDecision::Reject);
    }

    #[test]
    fn policy_scoped_to_specific_service() {
        let target_service = make_service();
        let other_service = make_service();
        let server = make_server(&target_service);

        let policy = format!(
            r#"permit(
                principal,
                action == Sprue::Action::"registerServer",
                resource == Sprue::Service::"{}"
            );"#,
            target_service.id
        );
        let engine = PolicyEngine::new(&policy, SCHEMA).unwrap();

        let accepted = engine
            .evaluate_server_auto_registration(&server, &target_service, &[])
            .unwrap();
        assert_eq!(accepted, PolicyDecision::Accept);

        // Same server against a different service should be rejected
        let rejected = engine
            .evaluate_server_auto_registration(&server, &other_service, &[])
            .unwrap();
        assert_eq!(rejected, PolicyDecision::Reject);
    }

    #[test]
    fn policy_checks_principal_project() {
        let service = make_service();
        let server = make_server(&service);

        let policy = format!(
            r#"permit(
                principal,
                action == Sprue::Action::"registerServer",
                resource
            ) when {{
                principal.project == Sprue::Project::"{}"
            }};"#,
            server.project_id
        );
        let engine = PolicyEngine::new(&policy, SCHEMA).unwrap();

        let accepted = engine
            .evaluate_server_auto_registration(&server, &service, &[])
            .unwrap();
        assert_eq!(accepted, PolicyDecision::Accept);

        // A server with a different project should be rejected
        let other_server = make_server(&service);
        let rejected = engine
            .evaluate_server_auto_registration(&other_server, &service, &[])
            .unwrap();
        assert_eq!(rejected, PolicyDecision::Reject);
    }

    #[test]
    fn policy_with_deployment_match() {
        let service = make_service();
        let server = make_server(&service);
        let deployment = make_deployment(&service, &server);

        // Permit only when the server's project/silo appears in the service's deployments
        let policy = r#"
            permit(
                principal,
                action == Sprue::Action::"registerServer",
                resource
            ) when {
                resource.deployments.contains({
                    "project": principal.project,
                    "silo": principal.silo
                })
            };
        "#;
        let engine = PolicyEngine::new(policy, SCHEMA).unwrap();

        // With a matching deployment, the server is accepted
        let accepted = engine
            .evaluate_server_auto_registration(&server, &service, &[deployment])
            .unwrap();
        assert_eq!(accepted, PolicyDecision::Accept);

        // Without deployments, the server is rejected
        let rejected = engine
            .evaluate_server_auto_registration(&server, &service, &[])
            .unwrap();
        assert_eq!(rejected, PolicyDecision::Reject);
    }

    #[test]
    fn invalid_policy_returns_error() {
        let result = PolicyEngine::new("this is not valid cedar", SCHEMA);
        assert!(result.is_err());
    }

    #[test]
    fn invalid_schema_returns_error() {
        let result = PolicyEngine::new("", "this is not valid cedar schema");
        assert!(result.is_err());
    }
}
