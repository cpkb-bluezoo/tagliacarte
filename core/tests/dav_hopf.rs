//! End-to-end: the DAV client (hopf HTTP + Tractrix) against hopf-webdav
//! behind hopf-http's Basic auth, in process: PUT, PROPFIND, GET, If-Match
//! and a refused anonymous request.

use std::net::SocketAddr;
use std::sync::Arc;

use hopf_auth::PasswordStore;
use hopf_core::{ProtocolHandler, TcpListenerConfig};
use hopf_http::{BasicAuthConfig, BasicAuthFactory, CleartextHttpEndpoint, HttpLimits, ServerHandlerFactory};
use hopf_webdav::{WebDavConfig, WebDavFactory};
use tagliacarte_core::net::hopf_runtime;
use tagliacarte_core::protocol::dav::DavClient;

fn start_server(root: &std::path::Path) -> SocketAddr {
    let config = WebDavConfig {
        root_path: root.to_path_buf(),
        allow_write: true,
        webdav_enabled: true,
        ..WebDavConfig::default()
    }
    // Auth is the Basic wrapper below; the crate wants that acknowledged.
    .allow_unauthenticated_access();
    let rt = hopf_runtime();
    let dav = Arc::new(WebDavFactory::new(config, Arc::clone(rt.storage())).unwrap());
    let store = Arc::new(PasswordStore::new().with_user("alice", "secret"));
    let factory: Arc<dyn ServerHandlerFactory> =
        Arc::new(BasicAuthFactory::new(dav, store, BasicAuthConfig::new("dav")));
    let (addr, _) = rt
        .add_tcp_listener(TcpListenerConfig::new("127.0.0.1:0".parse().unwrap(), move || {
            Box::new(CleartextHttpEndpoint::new(Arc::clone(&factory), HttpLimits::default())) as Box<dyn ProtocolHandler>
        }))
        .unwrap();
    addr
}

const VCARD: &[u8] = b"BEGIN:VCARD\r\nVERSION:3.0\r\nFN:Alice Example\r\nEMAIL:alice@example.com\r\nEND:VCARD\r\n";

#[test]
fn put_propfind_get_and_if_match_against_hopf_webdav() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("book")).unwrap();
    let addr = start_server(dir.path());
    let base = format!("http://{}:{}/book/", addr.ip(), addr.port());

    let (mut client, collection) = DavClient::for_url(&base).unwrap();
    client = client.with_basic_auth("alice", "secret");
    assert_eq!(collection, "/book/");

    let put = client.put("/book/a.vcf", "text/vcard; charset=utf-8", VCARD.to_vec(), None).unwrap();
    assert!(put.is_success(), "PUT status {}", put.status);

    let members = client.propfind(&collection).unwrap();
    let member = members
        .iter()
        .find(|m| m.href.ends_with("/book/a.vcf"))
        .unwrap_or_else(|| panic!("a.vcf listed: {members:?}"));
    assert!(member.etag.is_some(), "server reports an entity tag");
    assert!(members.iter().any(|m| m.href.trim_end_matches('/').ends_with("/book")), "collection itself listed");

    let got = client.get(&member.href).unwrap();
    assert_eq!(got.status, 200);
    assert_eq!(got.body, VCARD);

    // An If-Match that cannot match is refused before anything is written.
    let stale = client.put("/book/a.vcf", "text/vcard", b"BEGIN:VCARD\r\nEND:VCARD\r\n".to_vec(), Some("\"not-the-etag\"")).unwrap();
    assert_eq!(stale.status, 412);
    assert_eq!(std::fs::read(dir.path().join("book/a.vcf")).unwrap(), VCARD);

    // Without credentials the server challenges instead of listing.
    let (mut anon, _) = DavClient::for_url(&base).unwrap();
    let err = anon.propfind("/book/").err().expect("anonymous PROPFIND refused");
    assert!(err.to_string().contains("401"), "{err}");
}
