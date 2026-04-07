use serde_json::json;
use tempfile::tempdir;

use mempalace_rs::mcp_server::McpServer;

#[test]
fn serves_basic_json_rpc_tool_calls() {
    let dir = tempdir().unwrap();
    let server = McpServer::new(Some(dir.path().to_path_buf()), Some(dir.path().join("palace"))).unwrap();

    let init = server
        .handle_request(json!({
            "jsonrpc":"2.0",
            "id":1,
            "method":"initialize"
        }))
        .unwrap()
        .unwrap();
    assert_eq!(init["result"]["serverInfo"]["name"], "mempalace");

    let listed = server
        .handle_request(json!({
            "jsonrpc":"2.0",
            "id":2,
            "method":"tools/list"
        }))
        .unwrap()
        .unwrap();
    assert!(listed["result"]["tools"].as_array().unwrap().iter().any(|tool| tool["name"] == "mempalace_status"));

    let added = server
        .handle_request(json!({
            "jsonrpc":"2.0",
            "id":3,
            "method":"tools/call",
            "params":{
                "name":"mempalace_add_drawer",
                "arguments":{
                    "wing":"wing_code",
                    "room":"backend",
                    "content":"GraphQL auth migration notes"
                }
            }
        }))
        .unwrap()
        .unwrap();
    assert!(added["result"]["content"][0]["text"].as_str().unwrap().contains("\"success\": true"));

    let searched = server
        .handle_request(json!({
            "jsonrpc":"2.0",
            "id":4,
            "method":"tools/call",
            "params":{
                "name":"mempalace_search",
                "arguments":{
                    "query":"GraphQL auth"
                }
            }
        }))
        .unwrap()
        .unwrap();
    assert!(searched["result"]["content"][0]["text"].as_str().unwrap().contains("GraphQL auth migration notes"));
}
