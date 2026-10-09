//! Exercise the actual line transport, including recovery and modern result fields.
use photocraft_automation::PhotocraftMcp;
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

#[tokio::test(flavor = "multi_thread")]
async fn malformed_line_recovers_and_modern_lists_are_complete() {
    let (mut input, server_in) = tokio::io::duplex(1 << 20);
    let (server_out, output) = tokio::io::duplex(1 << 20);
    let server = tokio::spawn(PhotocraftMcp::headless().serve_io(server_in, server_out));
    let mut lines = BufReader::new(output).lines();
    let init = json!({"jsonrpc":"2.0", "id":1, "method":"initialize", "params":{"protocolVersion":"2025-06-18", "capabilities":{}, "clientInfo":{"name":"test", "version":"1"}}});
    input.write_all(format!("{init}\n").as_bytes()).await.unwrap();
    lines.next_line().await.unwrap().unwrap();
    input.write_all(b"{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\nBROKEN\n").await.unwrap();
    let error: Value = serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
    assert_eq!(error["id"], Value::Null);
    assert_eq!(error["error"]["code"], -32700);
    let meta = json!({"io.modelcontextprotocol/protocolVersion":"2026-07-28", "io.modelcontextprotocol/clientInfo":{"name":"test", "version":"1"}, "io.modelcontextprotocol/clientCapabilities":{}});
    for (id, method, extra) in [(2, "tools/list", json!({})), (3, "resources/list", json!({})), (4, "resources/read", json!({"uri":"photocraft://document"}))] {
        let mut params = extra;
        params["_meta"] = meta.clone();
        input.write_all(format!("{}\n", json!({"jsonrpc":"2.0", "id":id, "method":method, "params":params})).as_bytes()).await.unwrap();
        let reply: Value = serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
        assert_eq!(reply["id"], id);
        assert_eq!(reply["result"]["resultType"], "complete", "{reply}");
        assert!(reply["result"]["ttlMs"].is_number());
        assert_eq!(reply["result"]["cacheScope"], "private");
    }
    drop(input);
    tokio::time::timeout(std::time::Duration::from_secs(5), server).await.unwrap().unwrap().unwrap();
}
