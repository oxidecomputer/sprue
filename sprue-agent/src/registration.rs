// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use anyhow::anyhow;
use sprue_sdk::{
    Client,
    types::{ServerRegistration, ServerRegistrationState, TypedUuidForServerRegistrationId},
};
use std::{fmt, time::Duration};
use tokio::sync::{Notify, watch};

use crate::{cmd, platform::Platform};

/// How often a registration that is waiting for approval is checked again.
const WAITING_POLL_INTERVAL: Duration = Duration::from_secs(15);
/// How often an ended registration is checked to see whether it was deleted.
const ENDED_POLL_INTERVAL: Duration = Duration::from_secs(5 * 60);
const INITIAL_RETRY_INTERVAL: Duration = Duration::from_secs(1);
const MAX_RETRY_INTERVAL: Duration = Duration::from_secs(60);

/// The reason that a registration can make no further progress.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndReason {
    Rejected,
    Terminated,
}

/// What the agent currently knows about its registration with sprue.
#[derive(Debug, Clone)]
pub enum Standing {
    /// The agent has not yet learned its registration from sprue.
    Unregistered,
    /// The registration has been proven and is waiting to be accepted.
    Waiting(TypedUuidForServerRegistrationId),
    /// The registration has been accepted, so the agent may request tokens.
    Accepted(TypedUuidForServerRegistrationId),
    /// The registration was rejected or terminated. The agent does not try to
    /// register again unless the registration is deleted.
    Ended(TypedUuidForServerRegistrationId, EndReason),
}

impl Standing {
    /// The id of the current registration. This is None when the agent is not
    /// registered.
    pub fn registration_id(&self) -> Option<TypedUuidForServerRegistrationId> {
        match self {
            Self::Unregistered => None,
            Self::Waiting(id) | Self::Accepted(id) | Self::Ended(id, _) => Some(id.clone()),
        }
    }
}

// The generated id type does not implement PartialEq, so the ids are compared
// by their inner uuid.
impl PartialEq for Standing {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Unregistered, Self::Unregistered) => true,
            (Self::Waiting(a), Self::Waiting(b)) | (Self::Accepted(a), Self::Accepted(b)) => {
                a.0 == b.0
            }
            (Self::Ended(a, x), Self::Ended(b, y)) => a.0 == b.0 && x == y,
            _ => false,
        }
    }
}

impl fmt::Display for Standing {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unregistered => write!(f, "not registered"),
            Self::Waiting(id) => write!(f, "registration {} is waiting for approval", id.0),
            Self::Accepted(id) => write!(f, "registration {} is accepted", id.0),
            Self::Ended(id, EndReason::Rejected) => write!(f, "registration {} was rejected", id.0),
            Self::Ended(id, EndReason::Terminated) => {
                write!(f, "registration {} was terminated", id.0)
            }
        }
    }
}

impl TryFrom<ServerRegistration> for Standing {
    type Error = anyhow::Error;

    fn try_from(registration: ServerRegistration) -> Result<Self, Self::Error> {
        match registration.state {
            ServerRegistrationState::Proven => Ok(Self::Waiting(registration.id)),
            ServerRegistrationState::Accepted => Ok(Self::Accepted(registration.id)),
            ServerRegistrationState::Rejected => {
                Ok(Self::Ended(registration.id, EndReason::Rejected))
            }
            ServerRegistrationState::Terminated => {
                Ok(Self::Ended(registration.id, EndReason::Terminated))
            }
            ServerRegistrationState::Pending | ServerRegistrationState::Expired => Err(anyhow!(
                "Registration {} is in an unexpected state {:?}",
                registration.id.0,
                registration.state
            )),
        }
    }
}

/// Tracks the standing of this agent and drives it towards being accepted.
pub struct Registration {
    standing: watch::Sender<Standing>,
    recheck: Notify,
}

impl Registration {
    pub fn new() -> Self {
        Self {
            standing: watch::Sender::new(Standing::Unregistered),
            recheck: Notify::new(),
        }
    }

    pub fn standing(&self) -> Standing {
        self.standing.borrow().clone()
    }

    pub fn subscribe(&self) -> watch::Receiver<Standing> {
        self.standing.subscribe()
    }

    /// Ask for the standing of an accepted registration to be checked again.
    /// This is used when sprue refuses a request that carries this agent's
    /// token, which happens once the registration is no longer accepted.
    pub fn recheck(&self) {
        self.recheck.notify_one();
    }

    /// Drive the registration forever. Each pass answers a fresh challenge,
    /// and sprue replies with the registration of this instance. Sprue creates
    /// a new registration when none exists, which covers both the first start
    /// and a registration that an operator deleted.
    ///
    /// The current standing decides when the next pass happens. An agent that
    /// has not yet heard from sprue starts at once. A registration that is
    /// waiting for approval is checked on a short interval. An accepted
    /// registration is only checked when sprue refuses a request. An ended
    /// registration is checked on a long interval so that the agent notices
    /// when it is deleted.
    pub async fn run(&self, client: &Client, service: &str, platform: &dyn Platform) {
        let mut retry = INITIAL_RETRY_INTERVAL;
        loop {
            let standing = self.standing();
            match &standing {
                Standing::Unregistered => {}
                Standing::Waiting(_) => tokio::time::sleep(WAITING_POLL_INTERVAL).await,
                Standing::Accepted(_) => self.recheck.notified().await,
                Standing::Ended(..) => {
                    tracing::debug!(%standing, "Waiting for an ended registration to be deleted");
                    tokio::time::sleep(ENDED_POLL_INTERVAL).await;
                }
            }
            let next = cmd::register_server(client, service, platform).await;

            match next {
                Ok(next) => {
                    retry = INITIAL_RETRY_INTERVAL;
                    if next != standing {
                        tracing::info!(from = %standing, to = %next, "Registration standing changed");
                    }
                    self.standing.send_replace(next);
                }
                Err(err) => {
                    tracing::warn!(?err, %standing, retry_in = ?retry, "Failed to update registration standing");

                    // A failed check of an accepted registration still needs
                    // to happen once sprue can be reached again.
                    if let Standing::Accepted(_) = standing {
                        self.recheck();
                    }
                    tokio::time::sleep(retry).await;
                    retry = (retry * 2).min(MAX_RETRY_INTERVAL);
                }
            }
        }
    }
}
