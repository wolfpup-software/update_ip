use std::io;

use bytes::Buf;
use bytes::Bytes;
use http::Request;
use http::Response;
use http_body_util::{BodyExt, Empty};
use hyper::body::Incoming;
use hyper::client::conn::http1;
use hyper_util::rt::TokioIo;
use native_tls::TlsConnector;
use std::time::SystemTime;
use tokio::net::TcpStream;

// https://help.dyn.com/remote-access-api/perform-update/

use crate::type_flyweight::ResponseJson;

/*
    all upstream requests require a jsonable or (de)serializeable effort

    requests can return a body reader if not string

    serde can take a reader
    and strings can take a reader

    can let downstream functions decide
*/

pub async fn request_http1_tls_response(
    req: Request<Empty<Bytes>>,
) -> Result<Response<Incoming>, String> {
    let (host, addr) = match create_host_and_authority(&req) {
        Some(stream) => stream,
        _ => return Err("failed to get host and address from uri".to_string()),
    };
    let io = match create_tls_stream(&host, &addr).await {
        Ok(stream) => stream,
        Err(e) => return Err(e),
    };
    let (mut sender, conn) = match http1::handshake(io).await {
        Ok(handshake) => handshake,
        Err(e) => return Err(e.to_string()),
    };
    tokio::task::spawn(async move {
        if let Err(_err) = conn.await { /* log connection error */ }
    });

    let res = match sender.send_request(req).await {
        Ok(res) => res,
        Err(e) => return Err(e.to_string()),
    };

    Ok(res)
}

// this has multiple "types" of errors
// signal that it is an inappropriate grouping?
async fn create_tls_stream(
    host: &str,
    addr: &str,
) -> Result<TokioIo<tokio_native_tls::TlsStream<TcpStream>>, String> {
    let tls_connector = match TlsConnector::new() {
        Ok(cx) => tokio_native_tls::TlsConnector::from(cx),
        Err(e) => return Err(e.to_string()),
    };

    let client_stream = match TcpStream::connect(addr).await {
        Ok(s) => s,
        Err(e) => {
            return Err(e.to_string());
        }
    };

    let tls_stream = match tls_connector.connect(host, client_stream).await {
        Ok(s) => TokioIo::new(s),
        Err(e) => return Err(e.to_string()),
    };

    Ok(tls_stream)
}

fn create_host_and_authority(req: &Request<Empty<Bytes>>) -> Option<(&str, String)> {
    // need to check for port or default
    let host = match req.uri().host() {
        Some(h) => h,
        _ => return None,
    };

    let scheme = match req.uri().scheme() {
        Some(s) => s.as_str(),
        _ => http::uri::Scheme::HTTPS.as_str(),
    };

    let port = match req.uri().port() {
        Some(p) => p.to_string(),
        _ => match scheme {
            "http" => "80".to_string(),
            _ => "443".to_string(),
        },
    };

    let authority = host.to_string() + ":" + &port;

    Some((host, authority))
}

pub async fn response_body_to_string(response: Response<Incoming>) -> Result<String, String> {
    // asynchronously aggregate the chunks of the body
    let body = match response.collect().await {
        Ok(b) => b.aggregate(),
        Err(e) => return Err(e.to_string()),
    };

    let ip_str = match io::read_to_string(body.reader()) {
        Ok(b) => b,
        Err(e) => return Err(e.to_string()),
    };

    Ok(ip_str.trim().to_string())
}

pub fn create_request_with_empty_body(url_string: &str) -> Result<Request<Empty<Bytes>>, String> {
    let url = match http::Uri::try_from(url_string) {
        Ok(u) => u,
        Err(e) => return Err(e.to_string()),
    };

    let authority = match url.authority() {
        Some(u) => u.clone(),
        _ => return Err("authority missing in url".to_string()),
    };

    // add port when applicable

    let req = match Request::builder()
        .uri(url)
        .header(hyper::header::HOST, authority.as_str())
        .body(Empty::<Bytes>::new())
    {
        Ok(r) => r,
        Err(e) => return Err(e.to_string()),
    };

    Ok(req)
}

/*
    only adds ascii safe headers and header values.
    w3c spec accepts opaque values.
*/
pub fn get_headers(res: &Response<Incoming>) -> Vec<(String, String)> {
    let mut headers = Vec::<(String, String)>::new();
    for (key, value) in res.headers() {
        let value_str = match value.to_str() {
            Ok(v) => v.to_string(),
            _ => continue,
        };
        headers.push((key.to_string(), value_str))
    }
    headers
}

pub fn get_timestamp() -> Result<u128, String> {
    match SystemTime::now().duration_since(SystemTime::UNIX_EPOCH) {
        Ok(n) => Ok(n.as_millis()),
        Err(e) => Err(e.to_string()),
    }
}

pub async fn convert_response_to_json(res: Response<Incoming>) -> Result<ResponseJson, String> {
    let timestamp = match get_timestamp() {
        Ok(n) => n,
        Err(e) => return Err(e.to_string()),
    };

    let headers = get_headers(&res);
    let status = res.status().as_u16();

    let body_str = match response_body_to_string(res).await {
        Ok(r) => r,
        Err(e) => return Err(e.to_string()),
    };

    Ok(ResponseJson {
        status_code: status,
        body: body_str,
        headers: headers,
        timestamp: timestamp,
    })
}

pub fn get_https_dyndns2_subset_request(
    service_domain: &str,
    ip_addr: &str,
    hostname: &str,
    username: &str,
    password: &str,
) -> String {
    let auth_str = domain.username.to_string() + ":" + &domain.password;

    let mut domain_result = results::create_domain_result(&domain.hostname);
    let auth = general_purpose::STANDARD.encode(&auth_str.as_bytes());
    let auth_value = "Basic ".to_string() + &auth;

    // build request
    let request = match Request::builder()
        .uri(uri_str)
        .header(hyper::header::USER_AGENT, CLIENT_HEADER_VALUE)
        .header(hyper::header::AUTHORIZATION, auth_value)
        .body(Empty::<Bytes>::new())
    {
        Ok(s) => Some(s),
        Err(e) => {
            domain_result.errors.push(e.to_string());
            None
        }
    };

    "https://".to_string()
        + service_domain
        + "/nic/update?hostname="
        + hostname
        + "&myip="
        + ip_addr
}
