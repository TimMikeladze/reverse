//! End-to-end test of the MCP server: start the real binary, speak
//! JSON-RPC over its stdin and stdout, and check what an agent would see.

use serde_json::{json, Value as Json};
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

struct Server {
    child: std::process::Child,
    stdin: std::process::ChildStdin,
    stdout: BufReader<std::process::ChildStdout>,
    next_id: u64,
}

impl Server {
    fn start() -> Server {
        let mut child = Command::new(env!("CARGO_BIN_EXE_reverse"))
            .arg("mcp")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("cannot start the MCP server");
        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        Server {
            child,
            stdin,
            stdout,
            next_id: 1,
        }
    }
    fn notify(&mut self, method: &str) {
        writeln!(
            self.stdin,
            "{}",
            json!({ "jsonrpc": "2.0", "method": method })
        )
        .unwrap();
    }
    fn request(&mut self, method: &str, params: Json) -> Json {
        let id = self.next_id;
        self.next_id += 1;
        writeln!(
            self.stdin,
            "{}",
            json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params })
        )
        .unwrap();
        let mut line = String::new();
        self.stdout.read_line(&mut line).unwrap();
        let reply: Json =
            serde_json::from_str(&line).unwrap_or_else(|e| panic!("bad reply {:?}: {}", line, e));
        assert_eq!(reply["jsonrpc"], "2.0");
        assert_eq!(reply["id"], id);
        reply
    }
    fn tool(&mut self, name: &str, args: Json) -> Json {
        let reply = self.request("tools/call", json!({ "name": name, "arguments": args }));
        reply["result"].clone()
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
fn an_agent_can_check_fix_run_and_test_over_mcp() {
    let mut s = Server::start();

    let init = s.request(
        "initialize",
        json!({ "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "test", "version": "0" } }),
    );
    assert_eq!(init["result"]["protocolVersion"], "2025-06-18");
    assert_eq!(init["result"]["serverInfo"]["name"], "reverse");
    assert!(init["result"]["capabilities"]["tools"].is_object());
    s.notify("notifications/initialized");

    let tools = s.request("tools/list", json!({}));
    let names: Vec<&str> = tools["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        [
            "verse_check",
            "verse_run",
            "verse_test",
            "verse_explain",
            "verse_guide"
        ]
    );
    for t in tools["result"]["tools"].as_array().unwrap() {
        assert_eq!(
            t["inputSchema"]["type"], "object",
            "{} needs an object schema",
            t["name"]
        );
        assert!(t["description"].as_str().unwrap().len() > 40);
    }

    // 1. The agent writes Verse the way most models do at first.
    let draft = "Half(X : int)<decides> : int =\n    Mod[X, 2] == 0\n    Floor(X / 2)\n\nMain() : void =\n    Print(\"half of 8 is \" + Half(8))\n";
    let r = s.tool("verse_check", json!({ "source": draft }));
    assert_eq!(r["isError"], false);
    let report = &r["structuredContent"];
    assert_eq!(report["ok"], false);
    let codes: Vec<&str> = report["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["code"].as_str().unwrap())
        .collect();
    assert!(codes.contains(&"V0005"), "{:?}", codes); // ==
    assert!(codes.contains(&"V3002"), "{:?}", codes); // Half(8) needs []
    assert!(r["content"][0]["text"].as_str().unwrap().contains("help:"));

    // 2. It asks what a code means.
    let ex = s.tool("verse_explain", json!({ "code": "V3002" }));
    assert!(ex["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("square brackets"));

    // 3. It applies the fixes and runs the program.
    let fixed = "Half(X : int)<decides> : int =\n    Mod[X, 2] = 0\n    Floor(X / 2)\n\nMain() : void =\n    Print(\"half of 8 is {Half[8] or 0}\")\n\nTestHalf()<decides><transacts> : void =\n    Half[8] = 4\n    not Half[7]\n";
    let r = s.tool("verse_check", json!({ "source": fixed }));
    assert_eq!(
        r["structuredContent"]["ok"], true,
        "{}",
        r["content"][0]["text"]
    );
    let r = s.tool("verse_run", json!({ "source": fixed }));
    assert_eq!(r["structuredContent"]["output"], json!(["half of 8 is 4"]));
    assert_eq!(r["content"][0]["text"], "half of 8 is 4\n");

    // 4. And its tests.
    let r = s.tool("verse_test", json!({ "source": fixed }));
    assert_eq!(r["structuredContent"]["passed"], 1);
    assert_eq!(r["structuredContent"]["failed"], 0);

    // A runaway program is stopped, and the server keeps serving.
    let r = s.tool(
        "verse_run",
        json!({ "source": "Main() : void =\n    loop:\n        X := 1\n", "max_steps": 5000 }),
    );
    assert_eq!(r["structuredContent"]["error"]["kind"], "step_limit");
    let r = s.tool(
        "verse_run",
        json!({ "source": "Main() : void = Print(\"still here\")" }),
    );
    assert_eq!(r["structuredContent"]["output"], json!(["still here"]));

    // Bad requests are reported, not fatal.
    let r = s.tool("verse_check", json!({}));
    assert_eq!(r["isError"], true);
    let r = s.tool("no_such_tool", json!({}));
    assert_eq!(r["isError"], true);
    let reply = s.request("no/such/method", json!({}));
    assert_eq!(reply["error"]["code"], -32601);
    writeln!(s.stdin, "this is not json").unwrap();
    let mut line = String::new();
    s.stdout.read_line(&mut line).unwrap();
    assert!(line.contains("-32700"));
    let pong = s.request("ping", json!({}));
    assert!(pong["result"].is_object());

    let guide = s.tool("verse_guide", json!({}));
    assert!(guide["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("The ten mistakes to avoid"));
}
