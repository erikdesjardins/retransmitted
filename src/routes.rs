use hyper::client::HttpConnector;
use hyper::http::HeaderValue;
use hyper::{header, Body, Client, Method, Request, Response, StatusCode, Uri};
use hyper_rustls::HttpsConnector;
use std::str::FromStr;

pub struct State {
    pub client: Client<HttpsConnector<HttpConnector>>,
    pub secret_key: String,
}

#[allow(clippy::declare_interior_mutable_const)]
pub async fn respond_to_request(
    mut req: Request<Body>,
    state: &State,
) -> Result<Response<Body>, hyper::Error> {
    const X_RETRANSMITTED_KEY: &str = "x-retransmitted-key";
    const ANY: HeaderValue = HeaderValue::from_static("*");
    const ALLOWED_HEADERS: HeaderValue = HeaderValue::from_static(X_RETRANSMITTED_KEY);

    if req.method() == Method::OPTIONS {
        log::info!("{} {} -> [preflight response]", req.method(), req.uri());
        let mut resp = Response::new(Body::empty());
        resp.headers_mut()
            .append(header::ACCESS_CONTROL_ALLOW_ORIGIN, ANY);
        resp.headers_mut()
            .append(header::ACCESS_CONTROL_ALLOW_HEADERS, ALLOWED_HEADERS);
        return Ok(resp);
    }

    let provided_key = match req.headers_mut().remove(X_RETRANSMITTED_KEY) {
        Some(k) => k,
        None => {
            log::info!("{} {} -> [missing key]", req.method(), req.uri());
            let mut resp = Response::new(Body::empty());
            *resp.status_mut() = StatusCode::BAD_REQUEST;
            return Ok(resp);
        }
    };
    match ring::constant_time::verify_slices_are_equal(
        provided_key.as_bytes(),
        state.secret_key.as_bytes(),
    ) {
        Ok(()) => {}
        Err(ring::error::Unspecified) => {
            log::warn!("{} {} -> [invalid key]", req.method(), req.uri());
            let mut resp = Response::new(Body::empty());
            *resp.status_mut() = StatusCode::BAD_REQUEST;
            return Ok(resp);
        }
    }

    let path_and_query = match req
        .uri()
        .path_and_query()
        .and_then(|p| p.as_str().strip_prefix('/'))
    {
        Some(p_q) => p_q,
        None => {
            log::warn!("{} {} -> [missing url]", req.method(), req.uri());
            let mut resp = Response::new(Body::empty());
            *resp.status_mut() = StatusCode::BAD_REQUEST;
            return Ok(resp);
        }
    };
    let uri = match Uri::from_str(path_and_query) {
        Ok(a) => a,
        Err(e) => {
            log::warn!(
                "{} {} -> [invalid url] {:?} {}",
                req.method(),
                req.uri(),
                path_and_query,
                e
            );
            let mut resp = Response::new(Body::empty());
            *resp.status_mut() = StatusCode::BAD_REQUEST;
            return Ok(resp);
        }
    };

    log::info!("{} {} -> {}", req.method(), req.uri(), uri);
    *req.uri_mut() = uri;
    let mut resp = state.client.request(req).await;
    match resp.as_mut() {
        Ok(resp) => {
            resp.headers_mut()
                .append(header::ACCESS_CONTROL_ALLOW_ORIGIN, ANY);
        }
        Err(e) => {
            log::error!("-> [proxy error] {}", e);
        }
    }
    resp
}
