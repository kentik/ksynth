use std::convert::{TryFrom, TryInto};
use std::time::SystemTime;
use chrono::{DateTime, TimeZone, Utc};
use rustls::{Certificate, RootCertStore, ServerName, Error};
use rustls::client::{ServerCertVerified, ServerCertVerifier, WebPkiVerifier};
use x509_certificate::{X509Certificate, asn1time::Time};

pub struct Verifier {
    default: WebPkiVerifier,
}

#[derive(Clone, Debug)]
pub enum Identity {
    Valid(DateTime<Utc>),
    Error(Error),
    Unknown,
}

impl Verifier {
    pub fn new(roots: RootCertStore) -> Self {
        let default = WebPkiVerifier::new(roots, None);
        Self { default }
    }

    pub fn verify(
        &self,
        chain: &[Certificate],
        name:  &ServerName,
    ) -> Result<Identity, Error> {
        let default = &self.default;

        let (cert, chain) = chain.split_first().ok_or(Error::NoCertificatesPresented)?;
        let until = X509Certificate::from_der(cert).map(|cert| {
            match &cert.as_ref().tbs_certificate.validity.not_after {
                Time::UtcTime(time)     => *time.clone(),
                Time::GeneralTime(time) => time.clone().into(),
            }
        }).unwrap_or_else(|_| Utc.timestamp(0, 0));

        let scts = &mut Vec::new().into_iter();
        let ocsp = &[];
        let now  = SystemTime::now();

        match default.verify_server_cert(cert, chain, name, scts, ocsp, now) {
            Ok(_)  => Ok(Identity::Valid(until)),
            Err(e) => Ok(e.try_into()?),
        }
    }
}

impl ServerCertVerifier for Verifier {
    fn verify_server_cert(
        &self,
        _end_entity:      &Certificate,
        _intermediates:   &[Certificate],
        _server_name:     &ServerName,
        _scts:            &mut dyn Iterator<Item = &[u8]>,
        _ocsp_response:   &[u8],
        _now:             SystemTime,
    ) -> Result<ServerCertVerified, Error> {
        Ok(ServerCertVerified::assertion())
    }
}

impl Default for Identity {
    fn default() -> Self {
        Self::Unknown
    }
}

impl TryFrom<Error> for Identity {
    type Error = Error;

    fn try_from(e: Error) -> Result<Self, Error> {
        Ok(match e {
            Error::InvalidCertificateEncoding      => Identity::Error(e),
            Error::InvalidCertificateSignatureType => Identity::Error(e),
            Error::InvalidCertificateSignature     => Identity::Error(e),
            Error::InvalidCertificateData(_)       => Identity::Error(e),
            _                                      => return Err(e),
        })
    }
}
