use super::*;
use std::time::Duration;

// ─── TCP echo server via Rust OS threads ──────────────────────
// Tests the full server lifecycle: socket → bind → listen → accept → recv → send → close.
// Server runs in a separate OS thread; client runs in the test thread.

const ECHO_PORT: i32 = 31077;
const MULTI_PORT: i32 = 31078;
const WRAP_PORT: i32 = 31079;

const SERVER_ECHO: &str = r#"
func main() -> string {
    let fd = socket(2, 1, 0)
    if fd < 0 { return "socket failed" }
    let ret = bind(fd, PORT)
    if ret < 0 { close_fd(fd); return "bind failed" }
    let ret2 = listen(fd, 1)
    if ret2 < 0 { close_fd(fd); return "listen failed" }
    let client_fd = accept(fd)
    if client_fd < 0 { close_fd(fd); return "accept failed" }
    let data = recv(client_fd, 1024)
    let sent = send(client_fd, "echo: " + data)
    close_fd(client_fd)
    close_fd(fd)
    data
}
"#;

const CLIENT_ECHO: &str = r#"
func main() -> string {
    let fd = socket(2, 1, 0)
    if fd < 0 { return "client socket failed" }
    let ret = connect(fd, "127.0.0.1", PORT)
    if ret < 0 { close_fd(fd); return "connect failed" }
    let sent = send(fd, "hello")
    let data = recv(fd, 1024)
    close_fd(fd)
    data
}
"#;

#[test]
fn net_echo_server() {
    let server_src = SERVER_ECHO.replace("PORT", &ECHO_PORT.to_string());
    let client_src = CLIENT_ECHO.replace("PORT", &ECHO_PORT.to_string());

    let server = std::thread::spawn(move || {
        run_source(&server_src)
    });

    std::thread::sleep(Duration::from_millis(100));

    let client_result = run_source(&client_src);
    let server_result = server.join().unwrap();

    assert_eq!(server_result, interp::Value::String("hello".to_string()),
        "Server should receive 'hello', got {:?}", server_result);
    assert_eq!(client_result, interp::Value::String("echo: hello".to_string()),
        "Client should receive 'echo: hello', got {:?}", client_result);
}

#[test]
fn net_echo_server_sequential() {
    // Sequential ping-pong: server recv→send→recv→send, client send→recv→send→recv.
    // Each step blocks until the counterparty's action completes, ensuring ordering
    // without relying on TCP message boundaries.
    let server_src = r#"
func main() -> string {
    let fd = socket(2, 1, 0)
    if fd < 0 { return "socket failed" }
    let ret = bind(fd, PORT)
    if ret < 0 { close_fd(fd); return "bind failed" }
    let ret2 = listen(fd, 1)
    if ret2 < 0 { close_fd(fd); return "listen failed" }
    let client_fd = accept(fd)
    if client_fd < 0 { close_fd(fd); return "accept failed" }
    let msg1 = recv(client_fd, 1024)
    send(client_fd, "ack1: " + msg1)
    let msg2 = recv(client_fd, 1024)
    send(client_fd, "ack2: " + msg2)
    close_fd(client_fd)
    close_fd(fd)
    msg1 + msg2
}
"#.replace("PORT", &MULTI_PORT.to_string());

    let client_src = r#"
func main() -> string {
    let fd = socket(2, 1, 0)
    if fd < 0 { return "client socket failed" }
    let ret = connect(fd, "127.0.0.1", PORT)
    if ret < 0 { close_fd(fd); return "connect failed" }
    send(fd, "ab")
    let resp1 = recv(fd, 1024)
    send(fd, "cd")
    let resp2 = recv(fd, 1024)
    close_fd(fd)
    resp1 + resp2
}
"#.replace("PORT", &MULTI_PORT.to_string());

    let server = std::thread::spawn(move || {
        run_source(&server_src)
    });

    std::thread::sleep(Duration::from_millis(100));

    let client_result = run_source(&client_src);
    let server_result = server.join().unwrap();

    assert_eq!(server_result, interp::Value::String("abcd".to_string()),
        "Server should receive 'ab' + 'cd', got {:?}", server_result);
    assert_eq!(client_result, interp::Value::String("ack1: aback2: cd".to_string()),
        "Client should receive ack'd responses, got {:?}", client_result);
}

#[test]
fn net_echo_server_accept_wrapper() {
    // Test that tcp_accept wrapper works end-to-end
    let server_src = r#"
func main() -> string {
    let fd = socket(2, 1, 0)
    if fd < 0 { return "socket failed" }
    let ret = bind(fd, PORT)
    if ret < 0 { close_fd(fd); return "bind failed" }
    let ret2 = listen(fd, 1)
    if ret2 < 0 { close_fd(fd); return "listen failed" }
    let client_fd = accept(fd)
    if client_fd < 0 { close_fd(fd); return "accept failed" }
    let data = recv(client_fd, 1024)
    let s = send(client_fd, "received: " + data)
    close_fd(client_fd)
    close_fd(fd)
    data
}
"#.replace("PORT", &WRAP_PORT.to_string());

    let client_src = r#"
func main() -> string {
    let fd = socket(2, 1, 0)
    if fd < 0 { return "client socket failed" }
    let ret = connect(fd, "127.0.0.1", PORT)
    if ret < 0 { close_fd(fd); return "connect failed" }
    let s = send(fd, "world")
    let data = recv(fd, 1024)
    close_fd(fd)
    data
}
"#.replace("PORT", &WRAP_PORT.to_string());

    let server = std::thread::spawn(move || {
        run_source(&server_src)
    });

    std::thread::sleep(Duration::from_millis(100));

    let client_result = run_source(&client_src);
    let server_result = server.join().unwrap();

    assert_eq!(server_result, interp::Value::String("world".to_string()),
        "Server should receive 'world', got {:?}", server_result);
    assert_eq!(client_result, interp::Value::String("received: world".to_string()),
        "Client should receive 'received: world', got {:?}", client_result);
}
