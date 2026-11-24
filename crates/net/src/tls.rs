use std::sync::Arc;

use rustls::DigitallySignedStruct;
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::{CryptoProvider, WebPkiSupportedAlgorithms, verify_tls13_signature};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::server::danger::{ClientCertVerified, ClientCertVerifier};
use rustls::{DistinguishedName, SignatureScheme};

use crate::{NetError, Pin, TlsIdentity};

fn provider() -> Arc<CryptoProvider> {
    Arc::new(rustls::crypto::ring::default_provider())
}

/// Server side: TLS 1.3 only, a client certificate is required; which device it is gets decided later from its pin.
pub fn server_tls_config(identity: &TlsIdentity) -> Result<Arc<rustls::ServerConfig>, NetError> {
    let provider = provider();
    let verifier = AnyClientCert {
        algorithms: provider.signature_verification_algorithms,
    };
    let config = rustls::ServerConfig::builder_with_provider(provider)
        .with_protocol_versions(&[&rustls::version::TLS13])?
        .with_client_cert_verifier(Arc::new(verifier))
        .with_single_cert(vec![identity.certificate()], identity.private_key())?;
    Ok(Arc::new(config))
}

/// Client side: TLS 1.3 only, the server is accepted only when its key matches `server_pin`; presents `identity`.
pub fn client_tls_config(
    server_pin: Pin,
    identity: &TlsIdentity,
) -> Result<Arc<rustls::ClientConfig>, NetError> {
    let provider = provider();
    let verifier = PinnedServer {
        pin: server_pin,
        algorithms: provider.signature_verification_algorithms,
    };
    let config = rustls::ClientConfig::builder_with_provider(provider)
        .with_protocol_versions(&[&rustls::version::TLS13])?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(verifier))
        .with_client_auth_cert(vec![identity.certificate()], identity.private_key())?;
    Ok(Arc::new(config))
}

fn tls12_refused() -> rustls::Error {
    rustls::Error::General("TLS 1.2 is not supported".to_owned())
}

/// Accepts any client certificate whose handshake signature checks out; identity comes from the pin afterwards.
#[derive(Debug)]
struct AnyClientCert {
    algorithms: WebPkiSupportedAlgorithms,
}

impl ClientCertVerifier for AnyClientCert {
    fn offer_client_auth(&self) -> bool {
        true
    }

    fn client_auth_mandatory(&self) -> bool {
        true
    }

    fn root_hint_subjects(&self) -> &[DistinguishedName] {
        &[]
    }

    fn verify_client_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _now: UnixTime,
    ) -> Result<ClientCertVerified, rustls::Error> {
        // Only a parseable certificate can yield a pin later.
        Pin::of_certificate(end_entity).map_err(|_| {
            rustls::Error::InvalidCertificate(rustls::CertificateError::BadEncoding)
        })?;
        Ok(ClientCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        Err(tls12_refused())
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls13_signature(message, cert, dss, &self.algorithms)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.algorithms.supported_schemes()
    }
}

/// The pin of a server certificate that did not match, carried out of the TLS handshake inside its error.
#[derive(Debug)]
pub(crate) struct PresentedPin(pub(crate) Pin);

impl std::fmt::Display for PresentedPin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "the server presented {}", self.0)
    }
}

impl std::error::Error for PresentedPin {}

/// Accepts the server only when its certificate's key hashes to the pin; names and dates are ignored.
#[derive(Debug)]
struct PinnedServer {
    pin: Pin,
    algorithms: WebPkiSupportedAlgorithms,
}

impl ServerCertVerifier for PinnedServer {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        match Pin::of_certificate(end_entity) {
            Ok(pin) if pin == self.pin => Ok(ServerCertVerified::assertion()),
            // The pin it did present travels with the refusal, so the error can name both keys.
            Ok(pin) => Err(rustls::Error::InvalidCertificate(
                rustls::CertificateError::Other(rustls::OtherError(Arc::new(PresentedPin(pin)))),
            )),
            Err(_) => Err(rustls::Error::InvalidCertificate(
                rustls::CertificateError::ApplicationVerificationFailure,
            )),
        }
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        Err(tls12_refused())
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls13_signature(message, cert, dss, &self.algorithms)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.algorithms.supported_schemes()
    }
}
