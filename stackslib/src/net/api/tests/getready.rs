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
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use super::{run_json_request_with_ibd, TestRPC};
use crate::net::api::gethealth::{RPCGetHealthResponse, RPCNodeStatus};
use crate::net::api::getready::RPCGetReadyRequestHandler;
use crate::net::connection::ConnectionOptions;
use crate::net::httpcore::{StacksHttp, StacksHttpRequest};
use crate::net::ProtocolFamily;

#[test]
fn test_try_parse_request() {
    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 33333);
    let mut http = StacksHttp::new(addr, &ConnectionOptions::default());

    let request = StacksHttpRequest::new_getready(addr.into());
    let bytes = request.try_serialize().unwrap();

    let (parsed_preamble, offset) = http.read_preamble(&bytes).unwrap();
    let mut handler = RPCGetReadyRequestHandler::new();
    let mut parsed_request = http
        .handle_try_parse_request(
            &mut handler,
            &parsed_preamble.expect_request(),
            &bytes[offset..],
        )
        .unwrap();

    // parsed request consumes headers that would not be in a constructed request
    parsed_request.clear_headers();
    let (preamble, _contents) = parsed_request.destruct();

    assert_eq!(&preamble, request.preamble());
}

#[test]
fn test_get_ready_returns_ok_when_fully_synced() {
    let mut rpc_test = TestRPC::setup(function_name!());
    rpc_test.peer_2.refresh_burnchain_view();
    let expected_stacks_tip_height = rpc_test.peer_2.network.stacks_tip.height;
    let expected_burn_block_height = rpc_test.peer_2.network.chain_view.burn_block_height;

    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 33333);
    let request = StacksHttpRequest::new_getready(addr.into());
    let mut responses = rpc_test.run(vec![request]);
    let response = responses.remove(0);

    let (http_resp_preamble, contents) = response.destruct();
    assert_eq!(http_resp_preamble.status_code, 200, "Expected HTTP 200 OK");

    let response_json_val: serde_json::Value = contents
        .try_into()
        .unwrap_or_else(|e| panic!("Failed to parse JSON: {e}"));
    let ready_response: RPCGetHealthResponse = serde_json::from_value(response_json_val)
        .unwrap_or_else(|e| panic!("Failed to deserialize RPCGetHealthResponse: {e}"));

    assert_eq!(ready_response.status, RPCNodeStatus::Healthy);
    assert_eq!(ready_response.stacks_tip_height, expected_stacks_tip_height);
    assert_eq!(ready_response.burn_block_height, expected_burn_block_height);
    assert!(ready_response.is_fully_synced);
    assert!(!ready_response.server_version.is_empty());
}

#[test]
fn test_get_ready_returns_service_unavailable_when_syncing() {
    let mut rpc_test = TestRPC::setup(function_name!());
    rpc_test.peer_2.refresh_burnchain_view();
    let expected_stacks_tip_height = rpc_test.peer_2.network.stacks_tip.height;
    let expected_burn_block_height = rpc_test.peer_2.network.chain_view.burn_block_height;

    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 33333);
    let request = StacksHttpRequest::new_getready(addr.into());
    let (http_resp_preamble, response_json_val) =
        run_json_request_with_ibd(rpc_test, request, true);

    assert_eq!(
        http_resp_preamble.status_code, 503,
        "Expected HTTP 503 Service Unavailable"
    );
    let ready_response: RPCGetHealthResponse = serde_json::from_value(response_json_val).unwrap();

    assert_eq!(ready_response.status, RPCNodeStatus::Syncing);
    assert_eq!(ready_response.stacks_tip_height, expected_stacks_tip_height);
    assert_eq!(ready_response.burn_block_height, expected_burn_block_height);
    assert!(!ready_response.is_fully_synced);
    assert!(!ready_response.server_version.is_empty());
}
