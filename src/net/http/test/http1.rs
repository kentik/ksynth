use anyhow::Result;
use http::{Method, StatusCode, Version};
use tokio_test::block_on;
use crate::net::Network;
use crate::net::http::Request;
use super::setup;

#[test]
fn ipv4() -> Result<()> {
    block_on(async {
        let protocols = &[b"http/1.1".to_vec()];

        let (client, server) = setup("127.0.0.1", protocols).await?;

        let url      = format!("http://localhost:{}/", server.http.port()).parse()?;
        let request  = Request::new(Network::IPv4, Method::GET, url)?;
        let response = client.request(request).await?;

        assert_eq!(server.http, response.peer.addr);
        assert_eq!(Version::HTTP_11, response.head.version);
        assert_eq!(StatusCode::OK, response.head.status);

        let url      = format!("https://localhost:{}/", server.https.port()).parse()?;
        let request  = Request::new(Network::IPv4, Method::GET, url)?;
        let response = client.request(request).await?;

        assert_eq!(server.https, response.peer.addr);
        assert_eq!(Version::HTTP_11, response.head.version);
        assert_eq!(StatusCode::OK, response.head.status);

        let url      = format!("http://localhost:{}/", server.http.port()).parse()?;
        let request  = Request::new(Network::IPv6, Method::GET, url)?;
        let response = client.request(request).await;

        assert!(response.is_err());

        Ok(())
    })
}

#[test]
fn ipv6() -> Result<()> {
    block_on(async {
        let protocols = &[b"http/1.1".to_vec()];

        let (client, server) = setup("::1", protocols).await?;

        let url      = format!("http://localhost:{}/", server.http.port()).parse()?;
        let request  = Request::new(Network::IPv6, Method::GET, url)?;
        let response = client.request(request).await?;

        assert_eq!(server.http, response.peer.addr);
        assert_eq!(Version::HTTP_11, response.head.version);
        assert_eq!(StatusCode::OK, response.head.status);

        let url      = format!("https://localhost:{}/", server.https.port()).parse()?;
        let request  = Request::new(Network::IPv6, Method::GET, url)?;
        let response = client.request(request).await?;

        assert_eq!(server.https, response.peer.addr);
        assert_eq!(Version::HTTP_11, response.head.version);
        assert_eq!(StatusCode::OK, response.head.status);

        let url      = format!("http://localhost:{}/", server.http.port()).parse()?;
        let request  = Request::new(Network::IPv4, Method::GET, url)?;
        let response = client.request(request).await;

        assert!(response.is_err());

        Ok(())
    })
}
