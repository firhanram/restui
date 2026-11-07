use std::{
    io::{Read, Write},
    net::TcpListener,
    thread,
};

use restui::{
    http,
    model::{Method, QueryParam, RequestConfig},
};
use uuid::Uuid;

fn serve_once() -> (String, thread::JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = format!("http://{}", listener.local_addr().unwrap());
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut received = Vec::new();
        let mut buffer = [0_u8; 4096];
        loop {
            let count = stream.read(&mut buffer).unwrap();
            if count == 0 {
                break;
            }
            received.extend_from_slice(&buffer[..count]);
            if let Some(header_end) = received.windows(4).position(|bytes| bytes == b"\r\n\r\n") {
                let body_start = header_end + 4;
                let headers = String::from_utf8_lossy(&received[..body_start]);
                let content_length = headers
                    .lines()
                    .find_map(|line| {
                        line.to_ascii_lowercase()
                            .strip_prefix("content-length:")
                            .and_then(|value| value.trim().parse::<usize>().ok())
                    })
                    .unwrap_or(0);
                if received.len() >= body_start + content_length {
                    break;
                }
            }
        }

        let body = "{\"ok\":true}";
        write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        )
        .unwrap();
        String::from_utf8(received).unwrap()
    });
    (address, handle)
}

fn config(method: Method, url: String) -> RequestConfig {
    RequestConfig {
        id: Uuid::new_v4(),
        name: "Integration test".into(),
        method,
        url,
        params: Vec::new(),
        body: String::new(),
    }
}

#[tokio::test]
async fn sends_enabled_get_params_and_formats_json_response() {
    let (address, server) = serve_once();
    let mut request = config(Method::Get, format!("{address}/search"));
    request.params = vec![
        QueryParam::new("q".into(), "rust lang".into()),
        QueryParam {
            key: "disabled".into(),
            value: "yes".into(),
            enabled: false,
        },
    ];

    let response = http::execute(request).await.unwrap();
    let received = server.join().unwrap();

    assert!(received.starts_with("GET /search?q=rust+lang HTTP/1.1\r\n"));
    assert!(!received.contains("disabled=yes"));
    assert_eq!(response.status.as_u16(), 200);
    assert!(response.is_json);
    assert_eq!(response.display_body, "{\n  \"ok\": true\n}");
}

#[tokio::test]
async fn sends_post_put_and_patch_json_bodies() {
    for method in [Method::Post, Method::Put, Method::Patch] {
        let (address, server) = serve_once();
        let mut request = config(method, format!("{address}/items"));
        request.body = "{\"name\":\"Ada\"}".into();

        let response = http::execute(request).await.unwrap();
        let received = server.join().unwrap();

        assert!(received.starts_with(&format!("{} /items HTTP/1.1\r\n", method.as_str())));
        assert!(
            received
                .to_ascii_lowercase()
                .contains("content-type: application/json\r\n")
        );
        assert!(received.ends_with("{\"name\":\"Ada\"}"));
        assert_eq!(response.status.as_u16(), 200);
    }
}
