use std::fmt;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use rcgen::{CertificateParams, DistinguishedName, DnType, KeyPair, PublicKeyData};
use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};

use crate::{NetError, Pin};

/// An ECDSA P-256 key pair and its self-signed certificate. `Debug` never prints the key.
pub struct TlsIdentity {
    key: PrivatePkcs8KeyDer<'static>,
    cert: CertificateDer<'static>,
    pin: Pin,
}

impl TlsIdentity {
    /// A fresh key and a self-signed certificate naming `common_name`; names are never checked, only the pin.
    pub fn generate(common_name: &str) -> Result<Self, NetError> {
        let key_pair = KeyPair::generate().map_err(certificate_error)?;
        let mut params = CertificateParams::default();
        let mut name = DistinguishedName::new();
        name.push(DnType::CommonName, common_name);
        params.distinguished_name = name;
        let cert = params.self_signed(&key_pair).map_err(certificate_error)?;
        Ok(Self {
            key: PrivatePkcs8KeyDer::from(key_pair.serialize_der()),
            cert: cert.der().clone(),
            pin: Pin::of_spki(&key_pair.subject_public_key_info()),
        })
    }

    /// Loads both files, or creates and saves both when neither exists. A half-present pair is an error, so a key is
    /// never overwritten.
    pub fn load_or_create(
        key_path: &Path,
        cert_path: &Path,
        common_name: &str,
    ) -> Result<Self, NetError> {
        match (key_path.try_exists()?, cert_path.try_exists()?) {
            (true, true) => Self::load(key_path, cert_path),
            (false, false) => {
                let identity = Self::generate(common_name)?;
                identity.save(key_path, cert_path)?;
                Ok(identity)
            }
            (true, false) => Err(NetError::Certificate(format!(
                "key {} exists without its certificate {}",
                key_path.display(),
                cert_path.display()
            ))),
            (false, true) => Err(NetError::Certificate(format!(
                "certificate {} exists without its key {}",
                cert_path.display(),
                key_path.display()
            ))),
        }
    }

    /// Loads a PKCS#8 PEM key and a PEM certificate, refusing a certificate that is not for that key.
    pub fn load(key_path: &Path, cert_path: &Path) -> Result<Self, NetError> {
        let key = fs::read(key_path)?;
        let cert = fs::read(cert_path)?;
        Self::parse(
            &key,
            &cert,
            &key_path.display().to_string(),
            &cert_path.display().to_string(),
        )
    }

    /// The identity `to_pem` wrote, for a store that keeps the PEM text itself (the phone's sealed store).
    pub fn from_pem(key_pem: &[u8], cert_pem: &[u8]) -> Result<Self, NetError> {
        Self::parse(
            key_pem,
            cert_pem,
            "the device key",
            "the device certificate",
        )
    }

    fn parse(key: &[u8], cert: &[u8], key_name: &str, cert_name: &str) -> Result<Self, NetError> {
        let key = PrivatePkcs8KeyDer::from_pem_slice(key)
            .map_err(|_| NetError::Certificate(format!("{key_name} is not a PKCS#8 PEM key")))?;
        let cert = CertificateDer::from_pem_slice(cert)
            .map_err(|_| NetError::Certificate(format!("{cert_name} is not a PEM certificate")))?;
        let key_pair = KeyPair::try_from(&key).map_err(certificate_error)?;
        let pin = Pin::of_certificate(&cert)?;
        if pin != Pin::of_spki(&key_pair.subject_public_key_info()) {
            return Err(NetError::Certificate(format!(
                "{cert_name} is not the certificate of {key_name}"
            )));
        }
        Ok(Self { key, cert, pin })
    }

    /// Writes both files through a `.tmp` sibling and a rename. On Unix the key is 0600 and new parent directories
    /// 0700; on Windows the per-user profile directory's ACL protects them.
    pub fn save(&self, key_path: &Path, cert_path: &Path) -> Result<(), NetError> {
        let (key_pem, cert_pem) = self.to_pem()?;
        write_atomically(key_path, key_pem.as_bytes(), true)?;
        write_atomically(cert_path, cert_pem.as_bytes(), false)?;
        Ok(())
    }

    /// The private key and the certificate as PEM text, for a caller that writes the files itself.
    pub fn to_pem(&self) -> Result<(String, String), NetError> {
        let key_pair = KeyPair::try_from(&self.key).map_err(certificate_error)?;
        Ok((
            key_pair.serialize_pem(),
            pem_encode("CERTIFICATE", &self.cert),
        ))
    }

    pub fn pin(&self) -> Pin {
        self.pin
    }

    pub fn certificate_der(&self) -> &[u8] {
        &self.cert
    }

    pub(crate) fn certificate(&self) -> CertificateDer<'static> {
        self.cert.clone()
    }

    pub(crate) fn private_key(&self) -> PrivateKeyDer<'static> {
        PrivateKeyDer::Pkcs8(self.key.clone_key())
    }
}

impl fmt::Debug for TlsIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TlsIdentity")
            .field("pin", &self.pin)
            .field("key", &format_args!("<redacted>"))
            .finish()
    }
}

fn certificate_error(error: rcgen::Error) -> NetError {
    NetError::Certificate(error.to_string())
}

fn pem_encode(label: &str, der: &[u8]) -> String {
    use base64::Engine;
    let encoded = base64::engine::general_purpose::STANDARD.encode(der);
    let mut pem = format!("-----BEGIN {label}-----\n");
    for line in encoded.as_bytes().chunks(64) {
        pem.push_str(&String::from_utf8_lossy(line));
        pem.push('\n');
    }
    pem.push_str(&format!("-----END {label}-----\n"));
    pem
}

fn write_atomically(path: &Path, contents: &[u8], secret: bool) -> Result<(), NetError> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        create_private_dirs(parent)?;
    }
    let mut tmp_name = path
        .file_name()
        .map(|name| name.to_os_string())
        .ok_or_else(|| NetError::Certificate(format!("{} has no file name", path.display())))?;
    tmp_name.push(".tmp");
    let tmp: PathBuf = path.with_file_name(tmp_name);
    // A stale temp file could carry wider permissions; `create_new` then applies the mode below.
    match fs::remove_file(&tmp) {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => return Err(error.into()),
        _ => {}
    }
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    if secret {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&tmp)?;
    // Before any key byte lands, so a wide inherited ACL never covers it; the rename keeps the DACL.
    #[cfg(windows)]
    if secret {
        massorca_profile::restrict_to_owner(&tmp)?;
    }
    #[cfg(not(any(unix, windows)))]
    let _ = secret;
    file.write_all(contents)?;
    file.sync_all()?;
    drop(file);
    fs::rename(&tmp, path)?;
    Ok(())
}

fn create_private_dirs(dir: &Path) -> Result<(), NetError> {
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    #[cfg(windows)]
    let existed = dir.exists();
    builder.create(dir)?;
    // Like the Unix mode, only a directory this call creates is restricted.
    #[cfg(windows)]
    if !existed {
        massorca_profile::restrict_to_owner(dir)?;
    }
    Ok(())
}
