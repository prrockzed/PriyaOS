//! The scripted HTTP server answers as told and notices hang-ups.

use std::time::Duration;

use priya_testkit::http_script::{BodyStep, HttpReply, ScriptedHttp};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

async fn send(server: &ScriptedHttp, request: &str) -> TcpStream {
    let addr = server
        .base_url()
        .trim_start_matches("http://")
        .trim_end_matches("/v1")
        .to_owned();
    let mut socket = TcpStream::connect(addr).await.expect("the server listens");
    socket
        .write_all(request.as_bytes())
        .await
        .expect("the request is sent");
    socket
}

async fn read_all(mut socket: TcpStream) -> String {
    let mut text = String::new();
    socket
        .read_to_string(&mut text)
        .await
        .expect("the reply is read");
    text
}

// covers: AI-21
// suite: unit
#[tokio::test]
async fn the_server_plays_replies_in_order_and_records_requests() {
    let server = ScriptedHttp::start(vec![
        HttpReply::sse(&["{\"a\":1}", "[DONE]"]).header("x-test", "yes"),
        HttpReply::status(429, "{}").header("retry-after", "7"),
    ])
    .await
    .unwrap();
    assert!(server.base_url().starts_with("http://127.0.0.1:"));

    let first = read_all(
        send(&server, "POST /v1/chat/completions HTTP/1.1\r\nHost: x\r\nAuthorization: Bearer k\r\nContent-Length: 4\r\n\r\nbody").await,
    )
    .await;
    assert!(first.starts_with("HTTP/1.1 200 "), "{first}");
    assert!(first.contains("x-test: yes\r\n"));
    assert!(
        first.ends_with("data: {\"a\":1}\n\ndata: [DONE]\n\n"),
        "{first}"
    );

    let second = read_all(send(&server, "GET /v1/models HTTP/1.1\r\n\r\n").await).await;
    assert!(second.starts_with("HTTP/1.1 429 ") && second.contains("retry-after: 7"));
    let third = read_all(send(&server, "GET /v1/models HTTP/1.1\r\n\r\n").await).await;
    assert!(third.starts_with("HTTP/1.1 500 ") && third.contains("no reply left"));

    let requests = server.requests();
    assert_eq!(requests.len(), 3);
    assert_eq!(requests[0].method, "POST");
    assert_eq!(requests[0].path, "/v1/chat/completions");
    assert_eq!(requests[0].header("authorization"), Some("Bearer k"));
    assert_eq!(requests[0].body, b"body");
    assert_eq!(requests[1].header("authorization"), None);
}

// covers: AI-21
// suite: unit
#[tokio::test]
async fn a_client_hanging_up_closes_the_connection_at_once() {
    let server = ScriptedHttp::start(vec![HttpReply::sse_steps(vec![
        BodyStep::Send(b"data: {}\n\n".to_vec()),
        BodyStep::Pause(Duration::from_secs(30)),
        BodyStep::Hang,
    ])])
    .await
    .unwrap();
    let mut socket = send(&server, "POST /v1/chat/completions HTTP/1.1\r\n\r\n").await;
    let mut buffer = [0_u8; 256];
    let n = socket.read(&mut buffer).await.unwrap();
    assert!(n > 0);
    assert_eq!(server.open(), 1);
    drop(socket);
    let deadline = tokio::time::Instant::now() + Duration::from_millis(100);
    while server.open() > 0 {
        assert!(tokio::time::Instant::now() < deadline, "still open");
        tokio::time::sleep(Duration::from_millis(2)).await;
    }
}

// covers: AI-21
// suite: unit
#[tokio::test]
async fn bodies_are_paced_large_requests_are_read_and_extra_bytes_ignored() {
    let server = ScriptedHttp::start(vec![HttpReply::sse_steps(vec![
        BodyStep::Send(b"data: 1\n\n".to_vec()),
        BodyStep::Pause(Duration::from_millis(150)),
        BodyStep::Send(b"data: 2\n\n".to_vec()),
    ])])
    .await
    .unwrap();
    let body = "x".repeat(10_000);
    let mut socket = send(
        &server,
        &format!(
            "POST /v1/chat/completions HTTP/1.1\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        ),
    )
    .await;
    // Bytes past the body don't end the reply.
    socket.write_all(b"stray").await.unwrap();
    let begun = tokio::time::Instant::now();
    let reply = read_all(socket).await;
    assert!(begun.elapsed() >= Duration::from_millis(150), "paced");
    assert!(reply.ends_with("data: 1\n\ndata: 2\n\n"), "{reply}");
    assert_eq!(server.requests()[0].body.len(), 10_000);
    // The server's task ends a moment after the client sees the end.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(1);
    while server.open() > 0 {
        assert!(
            tokio::time::Instant::now() < deadline,
            "closed once the reply is done"
        );
        tokio::time::sleep(Duration::from_millis(2)).await;
    }
}
