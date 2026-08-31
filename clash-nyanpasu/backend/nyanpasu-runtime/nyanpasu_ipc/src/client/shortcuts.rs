use std::{
    pin::Pin,
    task::{Context, Poll},
};

use futures_util::{Stream, StreamExt};
use reqwest_websocket::{Message, Upgrade};

use crate::api::{
    self,
    contract::{
        CoreApply, CoreCheck, CoreRecover, CoreRestart, CoreStart, CoreStop, CoreV2Operation,
        CoreV2Status, CoreV2Submit, LogsInspect, LogsRetrieve, NetworkSetDns, Status,
    },
    core::{
        apply::{CORE_APPLY_ENDPOINT, CoreApplyData},
        v2::{
            CORE_V2_OPERATION_ENDPOINT, CORE_V2_STATUS_ENDPOINT, CORE_V2_SUBMIT_ENDPOINT,
            OperationInfo,
        },
    },
    log::{LOGS_INSPECT_ENDPOINT, LOGS_RETRIEVE_ENDPOINT},
    status::STATUS_ENDPOINT,
    ws::events::{EVENT_URI, Event},
};

use super::{ClientError, Result};

pub use super::Client;

impl Client {
    pub async fn status(&self) -> Result<api::status::StatusResBody<'static>> {
        self.call::<Status>(None)
            .await?
            .data
            .ok_or(ClientError::EmptyData {
                operation: STATUS_ENDPOINT,
            })
    }

    pub async fn start_core(&self, payload: &api::core::start::CoreStartReq<'_>) -> Result<()> {
        self.call::<CoreStart>(Some(payload)).await.map(|_| ())
    }

    pub async fn stop_core(&self) -> Result<()> {
        self.call::<CoreStop>(None).await.map(|_| ())
    }

    pub async fn restart_core(&self) -> Result<()> {
        self.call::<CoreRestart>(None).await.map(|_| ())
    }

    /// Apply a config to the running core. See
    /// [`CoreApplyData::outcome`](api::core::apply::CoreApplyData::outcome):
    /// `rolled_back` is a successful call reporting that the **old** config is
    /// what runs.
    pub async fn apply_config(
        &self,
        payload: &api::core::apply::CoreApplyReq<'_>,
    ) -> Result<CoreApplyData> {
        self.call::<CoreApply>(Some(payload))
            .await?
            .data
            .ok_or(ClientError::EmptyData {
                operation: CORE_APPLY_ENDPOINT,
            })
    }

    /// Dry-run a config against a core binary without touching the running one.
    pub async fn check_config(&self, payload: &api::core::check::CoreCheckReq<'_>) -> Result<()> {
        self.call::<CoreCheck>(Some(payload)).await.map(|_| ())
    }

    /// Submit one v2 control-plane operation. The reply is the operation's
    /// admission-time snapshot; poll [`Self::core_operation`] for the result.
    pub async fn submit_core(
        &self,
        payload: &api::core::v2::CoreSubmitReq<'_>,
    ) -> Result<OperationInfo> {
        self.call::<CoreV2Submit>(Some(payload))
            .await?
            .data
            .ok_or(ClientError::EmptyData {
                operation: CORE_V2_SUBMIT_ENDPOINT,
            })
    }

    /// Query one v2 operation, optionally long-polling for its terminal state.
    pub async fn core_operation(
        &self,
        payload: &api::core::v2::CoreOperationReq<'_>,
    ) -> Result<OperationInfo> {
        self.call::<CoreV2Operation>(Some(payload))
            .await?
            .data
            .ok_or(ClientError::EmptyData {
                operation: CORE_V2_OPERATION_ENDPOINT,
            })
    }

    /// The daemon's canonical core status projection.
    pub async fn core_status_v2(&self) -> Result<api::status::CoreInfos> {
        self.call::<CoreV2Status>(None)
            .await?
            .data
            .ok_or(ClientError::EmptyData {
                operation: CORE_V2_STATUS_ENDPOINT,
            })
    }

    /// Clear the manager's quarantine latch. Idempotent.
    pub async fn recover_core(&self) -> Result<()> {
        self.call::<CoreRecover>(None).await.map(|_| ())
    }

    pub async fn inspect_logs(&self) -> Result<api::log::LogsResBody<'static>> {
        self.call::<LogsInspect>(None)
            .await?
            .data
            .ok_or(ClientError::EmptyData {
                operation: LOGS_INSPECT_ENDPOINT,
            })
    }

    pub async fn retrieve_logs(&self) -> Result<api::log::LogsResBody<'static>> {
        self.call::<LogsRetrieve>(None)
            .await?
            .data
            .ok_or(ClientError::EmptyData {
                operation: LOGS_RETRIEVE_ENDPOINT,
            })
    }

    pub async fn set_dns(
        &self,
        payload: &api::network::set_dns::NetworkSetDnsReq<'_>,
    ) -> Result<()> {
        self.call::<NetworkSetDns>(Some(payload)).await.map(|_| ())
    }

    /// Subscribe to the events pushed by the service over `/ws/events`.
    ///
    /// Snapshot first: the service pushes one [`Event::CoreStatusChanged`] the
    /// moment the socket opens, one after every dropped-event recovery, and one
    /// per manager transition — including the `Starting`/`Restarting`
    /// transitions the two-valued [`Event::CoreStateChanged`] cannot express.
    /// There is nothing to negotiate and no version parameter to pass; the
    /// service ignores the query string.
    ///
    /// [`Event::CoreStateChanged`] keeps arriving alongside the snapshots, so a
    /// consumer of both sees each transition twice. The snapshot is idempotent,
    /// so the simplest correct handling is to let the last frame win.
    pub async fn events(&self) -> Result<EventStream> {
        let response = self
            .get(EVENT_URI)
            .upgrade()
            .send()
            .await
            .map_err(|source| ClientError::WebSocket {
                operation: EVENT_URI,
                source,
            })?;
        let websocket =
            response
                .into_websocket()
                .await
                .map_err(|source| ClientError::WebSocket {
                    operation: EVENT_URI,
                    source,
                })?;
        let stream = websocket.filter_map(|message| async move {
            let bytes = match message {
                Ok(Message::Binary(bytes)) => bytes,
                Ok(Message::Text(text)) => text.into(),
                // pings are answered internally, everything else is not an event
                Ok(_) => return None,
                Err(source) => {
                    return Some(Err(ClientError::WebSocket {
                        operation: EVENT_URI,
                        source,
                    }));
                }
            };
            Some(
                serde_json::from_slice(&bytes).map_err(|source| ClientError::Decode {
                    operation: EVENT_URI,
                    source,
                }),
            )
        });
        Ok(EventStream {
            inner: Box::pin(stream),
        })
    }
}

/// A stream of [`Event`]s pushed by the service.
pub struct EventStream {
    inner: Pin<Box<dyn Stream<Item = Result<Event>> + Send>>,
}

impl Stream for EventStream {
    type Item = Result<Event>;

    #[inline]
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.inner.poll_next_unpin(cx)
    }
}

impl std::fmt::Debug for EventStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EventStream").finish_non_exhaustive()
    }
}
