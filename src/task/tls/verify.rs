use chrono::{DateTime, TimeZone, Utc};
use rustls::*;
use webpki::{DNSNameRef, Error};
use x509_certificate::X509Certificate;
use TLSError::WebPKIError;

pub struct Verifier {
    default: WebPKIVerifier,
    roots:   RootCertStore,
}

#[derive(Clone, Debug)]
pub enum Identity {
    Valid(DateTime<Utc>),
    Error(Error),
    Unknown,
}

impl Verifier {
    pub fn new(roots: RootCertStore) -> Self {
        let default = WebPKIVerifier::new();
        Self { default, roots }
    }

    pub fn verify(
        &self,
        chain: &[Certificate],
        name:  DNSNameRef<'_>,
    ) -> Result<Identity, TLSError> {
        let default = &self.default;
        let roots   = &self.roots;

        let cert  = chain.first().ok_or(TLSError::NoCertificatesPresented)?;
        let until = match X509Certificate::from_der(cert) {
            Ok(cert) => cert.as_ref().tbs_certificate.validity.not_after.as_ref().clone(),
            Err(_)   => Utc.timestamp(0, 0),
        };

        match default.verify_server_cert(&roots, chain, name, &[]) {
            Ok(_)               => Ok(Identity::Valid(until)),
            Err(WebPKIError(e)) => Ok(Identity::Error(e)),
            Err(e)              => Err(e),
        }
    }

}

impl ServerCertVerifier for Verifier {
    fn verify_server_cert(
        &self,
        _roots:           &RootCertStore,
        _presented_certs: &[Certificate],
        _dns_name:        DNSNameRef<'_>,
        _ocsp_response:   &[u8],
    ) -> Result<ServerCertVerified, TLSError> {
        Ok(ServerCertVerified::assertion())
    }
}

impl Default for Identity {
    fn default() -> Self {
        Self::Unknown
    }
}
