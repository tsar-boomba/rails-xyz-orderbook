use std::{
    convert::Infallible,
    net::SocketAddrV4,
    sync::{Arc, LazyLock},
};

use bytes::Bytes;
use fastwebsockets::WebSocket;
use hickory_resolver::{
    Resolver, name_server::GenericConnector, proto::runtime::TokioRuntimeProvider,
};
use http_body_util::combinators::BoxBody;
use hyper::{
    Request, Response, Uri,
    body::Incoming,
    header::{CONNECTION, HOST, HeaderValue, SEC_WEBSOCKET_KEY, SEC_WEBSOCKET_VERSION, UPGRADE},
    upgrade::Upgraded,
};
use hyper_util::rt::{TokioExecutor, TokioIo};
use rustls::{RootCertStore, pki_types::ServerName};
use tokio::net::TcpStream;
use tokio_rustls::TlsConnector;

/// HTTP/1.1 Client that (only) supports DNS resolution and TLS
#[derive(Clone)]
pub struct Client {
    resolver: Arc<Resolver<GenericConnector<TokioRuntimeProvider>>>,
    connector: TlsConnector,
}

impl Client {
    pub fn new() -> color_eyre::Result<Self> {
        let resolver = {
            // To make this independent, if targeting macOS, BSD, Linux, or Windows, we can use the system's configuration:
            #[cfg(any(unix, windows))]
            {
                use hickory_resolver::{
                    TokioResolver, config, name_server::TokioConnectionProvider,
                };

                // use the system resolver configuration
                Arc::new(
                    TokioResolver::builder(TokioConnectionProvider::default())
                        .expect("failed to create resolver")
                        .build(),
                )
            }

            // For other operating systems, we can use one of the preconfigured definitions
            #[cfg(not(any(unix, windows)))]
            {
                // Directly reference the config types
                use hickory_resolver::{
                    Resolver,
                    config::{GOOGLE, ResolverConfig, ResolverOpts},
                };

                // Get a new resolver with the google nameservers as the upstream recursive resolvers
                Arc::new(Resolver::tokio(
                    ResolverConfig::udp_and_tcp(),
                    ResolverOpts::default(),
                ))
            }
        };

        static ROOT_CERT_STORE: LazyLock<Arc<RootCertStore>> = LazyLock::new(|| {
            let mut root_cert_store = rustls::RootCertStore::empty();
            for cert in rustls_native_certs::load_native_certs().unwrap() {
                root_cert_store.add(cert).unwrap();
            }

            Arc::new(root_cert_store)
        });

        let config = rustls::ClientConfig::builder()
            .with_root_certificates((*ROOT_CERT_STORE).clone())
            .with_no_client_auth();
        let connector = TlsConnector::from(Arc::new(config));

        Ok(Self {
            resolver,
            connector,
        })
    }

    async fn socket_addr_for_uri(&self, uri: &Uri) -> color_eyre::Result<SocketAddrV4> {
        let domain = uri.host().unwrap();
        let port = uri.port_u16().unwrap_or(443);
        let addr = self
            .resolver
            .ipv4_lookup(domain)
            .await?
            .iter()
            .next()
            .unwrap()
            .0;

        Ok(SocketAddrV4::new(addr, port))
    }

    pub async fn request(
        &self,
        uri: Uri,
        mut req: Request<BoxBody<Bytes, Infallible>>,
    ) -> color_eyre::Result<Response<Incoming>> {
        let domain = uri.host().unwrap().to_string();
        let socket_addr = self.socket_addr_for_uri(&uri).await?;
        req.headers_mut()
            .append(HOST, HeaderValue::try_from(&domain)?);
        *req.uri_mut() = uri;

        let stream = TcpStream::connect(socket_addr).await?;
        let domain = ServerName::try_from(domain.as_str())?.to_owned();
        let stream = self.connector.connect(domain, stream).await?;

        let (mut send_req, conn) = hyper::client::conn::http1::Builder::new()
            .writev(true)
            .handshake(TokioIo::new(stream))
            .await?;

        tokio::spawn(async move {
            if let Err(err) = conn.with_upgrades().await {
                println!("error in http conn: {err:?}");
            }
        });

        Ok(send_req.send_request(req).await?)
    }

    pub async fn websocket(
        &self,
        uri: Uri,
        mut req: Request<BoxBody<Bytes, Infallible>>,
    ) -> color_eyre::Result<(WebSocket<TokioIo<Upgraded>>, Response<Incoming>)> {
        let domain = uri.host().unwrap().to_string();
        let socket_addr = self.socket_addr_for_uri(&uri).await?;
        req.headers_mut()
            .append(HOST, HeaderValue::try_from(&domain)?);
        *req.uri_mut() = uri;

        // Add headers needed for websocket initiation
        req.headers_mut()
            .append(UPGRADE, HeaderValue::from_static("websocket"));
        req.headers_mut()
            .append(CONNECTION, HeaderValue::from_static("Upgrade"));
        req.headers_mut().append(
            SEC_WEBSOCKET_KEY,
            fastwebsockets::handshake::generate_key().try_into()?,
        );
        req.headers_mut()
            .append(SEC_WEBSOCKET_VERSION, HeaderValue::from_static("13"));

        let stream = TcpStream::connect(socket_addr).await?;
        let domain = ServerName::try_from(domain.as_str())?.to_owned();
        let stream = self.connector.connect(domain, stream).await?;

        Ok(fastwebsockets::handshake::client(&TokioExecutor::new(), req, stream).await?)
    }
}
