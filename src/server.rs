use crate::err::Error;
use crate::routes::{respond_to_request, State};
use hyper::service::{make_service_fn, service_fn};
use hyper::{Client, Server};
use hyper_rustls::HttpsConnectorBuilder;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;

pub async fn run(addr: SocketAddr, secret_key: String) -> Result<(), Error> {
    let client = Client::builder().build(
        HttpsConnectorBuilder::new()
            .with_native_roots()
            .https_or_http()
            .enable_http1()
            .enable_http2()
            .build(),
    );

    let state = Arc::new(State { client, secret_key });
    let make_svc = make_service_fn(move |_| {
        let state = Arc::clone(&state);
        let svc = service_fn(move |req| {
            let state = Arc::clone(&state);
            async move { respond_to_request(req, &state).await }
        });
        async move { Ok::<_, Infallible>(svc) }
    });

    Server::try_bind(&addr)?.serve(make_svc).await?;

    Ok(())
}
