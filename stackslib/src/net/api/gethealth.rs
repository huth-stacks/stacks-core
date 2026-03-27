// Copyright (C) 2013-2020 Blockstack PBC, a public benefit corporation
// Copyright (C) 2020-2025 Stacks Open Internet Foundation
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.

use regex::{Captures, Regex};
use stacks_common::types::net::PeerHost;

use crate::net::http::{
    http_reason, parse_json, Error, HttpContentType, HttpRequest, HttpRequestContents,
    HttpRequestPreamble, HttpResponse, HttpResponseContents, HttpResponsePayload,
    HttpResponsePreamble,
};
use crate::net::httpcore::{RPCRequestHandler, StacksHttpRequest, StacksHttpResponse};
use crate::net::p2p::PeerNetwork;
use crate::net::{Error as NetError, StacksNodeState};
use crate::version_string;

/// The node health status reported by `/v3/health` and `/v3/ready`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RPCNodeStatus {
    Healthy,
    Syncing,
    Unhealthy,
}

/// The response for the GET /v3/health endpoint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RPCGetHealthResponse {
    pub status: RPCNodeStatus,
    pub stacks_tip_height: u64,
    pub burn_block_height: u64,
    pub is_fully_synced: bool,
    pub server_version: String,
}

impl RPCGetHealthResponse {
    fn from_network(network: &PeerNetwork, ibd: bool) -> Self {
        let is_fully_synced = !ibd;
        let status = if is_fully_synced {
            RPCNodeStatus::Healthy
        } else {
            RPCNodeStatus::Syncing
        };

        Self {
            status,
            stacks_tip_height: network.stacks_tip.height,
            burn_block_height: network.chain_view.burn_block_height,
            is_fully_synced,
            server_version: version_string("stacks-node", option_env!("STACKS_NODE_VERSION")),
        }
    }

    pub(crate) fn health_status_code(&self) -> u16 {
        match self.status {
            RPCNodeStatus::Healthy | RPCNodeStatus::Syncing => 200,
            RPCNodeStatus::Unhealthy => 503,
        }
    }

    pub(crate) fn readiness_status_code(&self) -> u16 {
        if self.is_fully_synced {
            200
        } else {
            503
        }
    }
}

pub(crate) fn get_node_health(node: &mut StacksNodeState) -> RPCGetHealthResponse {
    let ibd = node.ibd;
    node.with_node_state(|network, _sortdb, _chainstate, _mempool, _rpc_args| {
        RPCGetHealthResponse::from_network(network, ibd)
    })
}

pub(crate) fn node_health_http_response(
    preamble: &HttpRequestPreamble,
    status_code: u16,
    data: &RPCGetHealthResponse,
) -> Result<(HttpResponsePreamble, HttpResponseContents), NetError> {
    let body = HttpResponseContents::try_from_json(data)?;
    let preamble = HttpResponsePreamble::from_http_request_preamble(
        preamble,
        status_code,
        http_reason(status_code),
        body.content_length(),
        HttpContentType::JSON,
    );
    Ok((preamble, body))
}

pub(crate) fn decode_node_health_response(
    response: StacksHttpResponse,
) -> Result<RPCGetHealthResponse, NetError> {
    let (_preamble, payload) = response.destruct();
    let response_json: serde_json::Value = payload.try_into()?;
    serde_json::from_value(response_json)
        .map_err(|_e| Error::DecodeError("Failed to decode JSON".to_string()).into())
}

#[derive(Clone)]
/// Empty request handler for the GET /v3/health endpoint.
pub struct RPCGetHealthRequestHandler {}

impl RPCGetHealthRequestHandler {
    pub fn new() -> Self {
        Self {}
    }
}

/// Decode the HTTP request
impl HttpRequest for RPCGetHealthRequestHandler {
    fn verb(&self) -> &'static str {
        "GET"
    }

    fn path_regex(&self) -> Regex {
        Regex::new(r#"^/v3/health$"#).unwrap()
    }

    fn metrics_identifier(&self) -> &str {
        "/v3/health"
    }

    /// Try to decode this request.
    /// There's nothing to load here, so just make sure the request is well-formed.
    fn try_parse_request(
        &mut self,
        preamble: &HttpRequestPreamble,
        _captures: &Captures,
        query: Option<&str>,
        _body: &[u8],
    ) -> Result<HttpRequestContents, Error> {
        if preamble.get_content_length() != 0 {
            return Err(Error::DecodeError(
                "Invalid Http request: expected 0-length body for GetHealth".to_string(),
            ));
        }

        Ok(HttpRequestContents::new().query_string(query))
    }
}

impl RPCRequestHandler for RPCGetHealthRequestHandler {
    /// Reset internal state
    fn restart(&mut self) {}

    /// Make the response
    fn try_handle_request(
        &mut self,
        preamble: HttpRequestPreamble,
        _contents: HttpRequestContents,
        node: &mut StacksNodeState,
    ) -> Result<(HttpResponsePreamble, HttpResponseContents), NetError> {
        let health = get_node_health(node);
        node_health_http_response(&preamble, health.health_status_code(), &health)
    }
}

/// Decode the HTTP response
impl HttpResponse for RPCGetHealthRequestHandler {
    fn try_parse_response(
        &self,
        preamble: &HttpResponsePreamble,
        body: &[u8],
    ) -> Result<HttpResponsePayload, Error> {
        let health: RPCGetHealthResponse = parse_json(preamble, body)?;
        Ok(HttpResponsePayload::try_from_json(health)?)
    }
}

impl StacksHttpRequest {
    pub fn new_gethealth(host: PeerHost) -> StacksHttpRequest {
        StacksHttpRequest::new_for_peer(
            host,
            "GET".into(),
            "/v3/health".into(),
            HttpRequestContents::new(),
        )
        .expect("FATAL: failed to construct request from infallible data")
    }
}

impl StacksHttpResponse {
    pub fn decode_gethealth(self) -> Result<RPCGetHealthResponse, NetError> {
        decode_node_health_response(self)
    }
}
