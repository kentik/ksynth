use std::ops::Deref;
use anyhow::Result;
use rustls::OwnedTrustAnchor;
use rustls_native_certs::load_native_certs;
use webpki::TrustAnchor;
use webpki_roots::TLS_SERVER_ROOTS;

pub struct TrustAnchors(Vec<OwnedTrustAnchor>);

impl TrustAnchors {
    pub fn native() -> Result<Self> {
        let anchors = load_native_certs()?;
        Ok(Self(anchors.iter().map(|der| {
            let anchor = TrustAnchor::try_from_cert_der(&der.0)?;
            Ok(OwnedTrustAnchor::from_subject_spki_name_constraints(
                anchor.subject,
                anchor.spki,
                anchor.name_constraints,
            ))
        }).collect::<Result<_>>()?))
    }

    pub fn webpki() -> Self {
        let anchors = &TLS_SERVER_ROOTS;
        Self(anchors.0.iter().map(|anchor| {
            OwnedTrustAnchor::from_subject_spki_name_constraints(
                anchor.subject,
                anchor.spki,
                anchor.name_constraints,
            )
        }).collect())
    }
}

impl Deref for TrustAnchors {
    type Target = [OwnedTrustAnchor];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
