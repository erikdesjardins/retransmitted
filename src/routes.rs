use crate::path::extract_uri_from_path;
use hyper::client::HttpConnector;
use hyper::http::HeaderValue;
use hyper::{header, Body, Client, Method, Request, Response, StatusCode};
use hyper_rustls::HttpsConnector;
use std::convert::Infallible;
use std::mem;

pub struct State {
    pub client: Client<HttpsConnector<HttpConnector>>,
    pub secret_key: String,
}

#[allow(clippy::declare_interior_mutable_const)]
pub async fn respond_to_request(
    mut req: Request<Body>,
    state: &State,
) -> Result<Response<Body>, Infallible> {
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
            *resp.status_mut() = StatusCode::UNAUTHORIZED;
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
            *resp.status_mut() = StatusCode::UNAUTHORIZED;
            return Ok(resp);
        }
    }

    let uri = match extract_uri_from_path(req.uri()) {
        None => {
            log::warn!("{} {} -> [missing url]", req.method(), req.uri());
            let mut resp = Response::new(Body::empty());
            *resp.status_mut() = StatusCode::BAD_REQUEST;
            return Ok(resp);
        }
        Some(Err((e, unparsed))) => {
            log::warn!(
                "{} {} -> [invalid url] {:?} {}",
                req.method(),
                req.uri(),
                unparsed,
                e
            );
            let mut resp = Response::new(Body::empty());
            *resp.status_mut() = StatusCode::BAD_REQUEST;
            return Ok(resp);
        }
        Some(Ok(u)) => u,
    };

    let orig_method = req.method().clone();
    let orig_uri = mem::replace(req.uri_mut(), uri);
    let mut resp = match state.client.request(req).await {
        Ok(r) => r,
        Err(e) => {
            log::error!("{} {} -> [proxy error] {}", orig_method, orig_uri, e);
            let mut resp = Response::new(Body::empty());
            *resp.status_mut() = StatusCode::BAD_GATEWAY;
            return Ok(resp);
        }
    };

    log::info!("{} {} -> [success]", orig_method, orig_uri);
    resp.headers_mut()
        .append(header::ACCESS_CONTROL_ALLOW_ORIGIN, ANY);
    Ok(resp)
}
