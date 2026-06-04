//! # arvik-tls
//!
//! TLS / HTTPS support for the Arvik web framework.
//!
//! Rustls is the primary backend and provides the guaranteed HTTP/2 ALPN path.
//! The native-tls backend is optional and uses platform TLS where available.

use std::path::PathBuf;

/// TLS errors returned by Arvik TLS configuration and reload operations.
#[derive(Debug, thiserror::Error)]
pub enum TlsError {
    /// File IO failed.
    #[error("TLS IO error: {0}")]
    Io(#[from] std::io::Error),

    /// PEM parsing failed.
    #[error("TLS PEM error: {0}")]
    Pem(String),

    /// TLS configuration failed.
    #[error("TLS configuration error: {0}")]
    Config(String),

    /// Certificate generation failed.
    #[error("TLS certificate generation error: {0}")]
    CertificateGeneration(String),

    /// File watching failed.
    #[cfg(feature = "tls-hot-reload")]
    #[error("TLS watch error: {0}")]
    Notify(#[from] notify::Error),

    /// native-tls failed.
    #[cfg(feature = "native-tls")]
    #[error("native-tls error: {0}")]
    NativeTls(#[from] native_tls::Error),
}

/// Result alias for TLS operations.
pub type Result<T> = std::result::Result<T, TlsError>;

/// rustls backend.
#[cfg(feature = "rustls")]
pub mod rustls {
    use std::path::Path;
    use std::sync::Arc;
    #[cfg(feature = "tls-hot-reload")]
    use std::time::Duration;

    use ::rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
    use parking_lot::RwLock;
    use tokio::net::TcpStream;
    use tokio_rustls::TlsAcceptor;

    use crate::{Result, TlsError};

    const ALPN_HTTP2: &[u8] = b"h2";
    const ALPN_HTTP11: &[u8] = b"http/1.1";
    const EXPIRY_WARN_DAYS: u64 = 30;

    /// Reloadable rustls server configuration.
    ///
    /// Clones share the same active TLS configuration. Reloading one clone
    /// affects only future accepted connections; already accepted TLS sessions
    /// keep using the server config captured during their handshake.
    #[derive(Clone)]
    pub struct RustlsConfig {
        inner: Arc<RwLock<Arc<::rustls::ServerConfig>>>,
    }

    impl RustlsConfig {
        /// Build a rustls config from certificate and private key PEM files.
        pub async fn from_pem_file(
            cert_path: impl AsRef<Path>,
            key_path: impl AsRef<Path>,
        ) -> Result<Self> {
            let cert_pem = tokio::fs::read(cert_path).await?;
            let key_pem = tokio::fs::read(key_path).await?;
            Self::from_pem(cert_pem, key_pem).await
        }

        /// Build a rustls config from in-memory PEM bytes.
        pub async fn from_pem(
            cert_pem: impl AsRef<[u8]>,
            key_pem: impl AsRef<[u8]>,
        ) -> Result<Self> {
            let certs = parse_certs(cert_pem.as_ref())?;
            let key = parse_private_key(key_pem.as_ref())?;
            Self::from_der(certs, key)
        }

        /// Build a rustls config from DER certificate chain and private key.
        pub fn from_der(
            certs: Vec<CertificateDer<'static>>,
            key: PrivateKeyDer<'static>,
        ) -> Result<Self> {
            let config = build_server_config(certs, key)?;
            Ok(Self {
                inner: Arc::new(RwLock::new(Arc::new(config))),
            })
        }

        /// Build a self-signed certificate for local development.
        ///
        /// This is intended for development and tests only.
        pub async fn self_signed<I, S>(subject_alt_names: I) -> Result<Self>
        where
            I: IntoIterator<Item = S>,
            S: Into<String>,
        {
            let names = subject_alt_names
                .into_iter()
                .map(Into::into)
                .collect::<Vec<_>>();
            let certified_key = rcgen::generate_simple_self_signed(names)
                .map_err(|err| TlsError::CertificateGeneration(err.to_string()))?;
            let cert = CertificateDer::from(certified_key.cert);
            let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(
                certified_key.signing_key.serialize_der(),
            ));
            Self::from_der(vec![cert], key)
        }

        /// Reload the active config from certificate and key PEM files.
        ///
        /// If parsing or rustls validation fails, the old config stays active.
        pub async fn reload_from_pem_file(
            &self,
            cert_path: impl AsRef<Path>,
            key_path: impl AsRef<Path>,
        ) -> Result<()> {
            let cert_pem = tokio::fs::read(cert_path).await?;
            let key_pem = tokio::fs::read(key_path).await?;
            self.reload_from_pem(cert_pem, key_pem).await
        }

        /// Reload the active config from in-memory PEM bytes.
        ///
        /// If parsing or rustls validation fails, the old config stays active.
        pub async fn reload_from_pem(
            &self,
            cert_pem: impl AsRef<[u8]>,
            key_pem: impl AsRef<[u8]>,
        ) -> Result<()> {
            let certs = parse_certs(cert_pem.as_ref())?;
            let key = parse_private_key(key_pem.as_ref())?;
            let new_config = Arc::new(build_server_config(certs, key)?);
            *self.inner.write() = new_config;
            tracing::info!("Reloaded rustls certificate configuration");
            Ok(())
        }

        /// Return the currently active rustls server config.
        pub fn current_config(&self) -> Arc<::rustls::ServerConfig> {
            Arc::clone(&self.inner.read())
        }

        /// Return a TLS acceptor using the currently active config.
        pub fn acceptor(&self) -> TlsAcceptor {
            TlsAcceptor::from(self.current_config())
        }

        /// Accept a TCP stream with the currently active TLS config.
        pub async fn accept(
            &self,
            stream: TcpStream,
        ) -> Result<tokio_rustls::server::TlsStream<TcpStream>> {
            self.acceptor()
                .accept(stream)
                .await
                .map_err(|err| TlsError::Config(err.to_string()))
        }

        /// Watch certificate and key files, debouncing changes before reload.
        #[cfg(feature = "tls-hot-reload")]
        pub fn watch_pem_files(
            &self,
            cert_path: impl Into<std::path::PathBuf>,
            key_path: impl Into<std::path::PathBuf>,
            debounce: Duration,
        ) -> Result<TlsReloadWatcher> {
            TlsReloadWatcher::new(self.clone(), cert_path.into(), key_path.into(), debounce)
        }
    }

    fn parse_certs(pem: &[u8]) -> Result<Vec<CertificateDer<'static>>> {
        let mut reader = std::io::Cursor::new(pem);
        let certs = rustls_pemfile::certs(&mut reader)
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|err| TlsError::Pem(err.to_string()))?;

        if certs.is_empty() {
            return Err(TlsError::Pem("no certificates found".to_string()));
        }

        Ok(certs)
    }

    fn parse_private_key(pem: &[u8]) -> Result<PrivateKeyDer<'static>> {
        let mut reader = std::io::Cursor::new(pem);
        rustls_pemfile::private_key(&mut reader)
            .map_err(|err| TlsError::Pem(err.to_string()))?
            .ok_or_else(|| TlsError::Pem("no private key found".to_string()))
    }

    fn build_server_config(
        certs: Vec<CertificateDer<'static>>,
        key: PrivateKeyDer<'static>,
    ) -> Result<::rustls::ServerConfig> {
        warn_certificate_expiry(&certs);

        let mut config = ::rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(certs, key)
            .map_err(|err| TlsError::Config(err.to_string()))?;
        config.alpn_protocols = vec![ALPN_HTTP2.to_vec(), ALPN_HTTP11.to_vec()];
        Ok(config)
    }

    fn warn_certificate_expiry(certs: &[CertificateDer<'static>]) {
        let Some(leaf) = certs.first() else {
            return;
        };

        let Ok((_, cert)) = x509_parser::parse_x509_certificate(leaf.as_ref()) else {
            tracing::warn!("Unable to parse leaf certificate for expiry warning");
            return;
        };

        match cert.validity().time_to_expiration() {
            Some(remaining)
                if remaining.whole_seconds() <= (EXPIRY_WARN_DAYS * 24 * 60 * 60) as i64 =>
            {
                tracing::warn!(
                    days_remaining = remaining.whole_seconds() / 86_400,
                    "TLS certificate expires soon"
                );
            }
            None => tracing::warn!("TLS certificate is not currently valid or has expired"),
            Some(_) => {}
        }
    }

    /// Active TLS file watcher.
    #[cfg(feature = "tls-hot-reload")]
    pub struct TlsReloadWatcher {
        _watcher: notify::RecommendedWatcher,
        task: tokio::task::JoinHandle<()>,
    }

    #[cfg(feature = "tls-hot-reload")]
    impl TlsReloadWatcher {
        fn new(
            config: RustlsConfig,
            cert_path: std::path::PathBuf,
            key_path: std::path::PathBuf,
            debounce: Duration,
        ) -> Result<Self> {
            use notify::{RecursiveMode, Watcher as _};

            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<()>();
            let mut watcher = notify::recommended_watcher(
                move |event: notify::Result<notify::Event>| match event {
                    Ok(_) => {
                        let _ = tx.send(());
                    }
                    Err(err) => tracing::warn!("TLS certificate watch error: {}", err),
                },
            )?;

            watcher.watch(&cert_path, RecursiveMode::NonRecursive)?;
            watcher.watch(&key_path, RecursiveMode::NonRecursive)?;

            let task = tokio::spawn(async move {
                while rx.recv().await.is_some() {
                    tokio::time::sleep(debounce).await;
                    while rx.try_recv().is_ok() {}

                    if let Err(err) = config.reload_from_pem_file(&cert_path, &key_path).await {
                        tracing::warn!(
                            "TLS certificate reload failed; keeping previous config active: {}",
                            err
                        );
                    }
                }
            });

            Ok(Self {
                _watcher: watcher,
                task,
            })
        }
    }

    #[cfg(feature = "tls-hot-reload")]
    impl Drop for TlsReloadWatcher {
        fn drop(&mut self) {
            self.task.abort();
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[tokio::test]
        async fn self_signed_config_uses_h2_then_http11_alpn() {
            let config = RustlsConfig::self_signed(["localhost"]).await.unwrap();
            let current = config.current_config();
            assert_eq!(
                current.alpn_protocols,
                vec![b"h2".to_vec(), b"http/1.1".to_vec()]
            );
        }

        #[tokio::test]
        async fn invalid_pem_does_not_replace_active_config() {
            let config = RustlsConfig::self_signed(["localhost"]).await.unwrap();
            let before = config.current_config();

            let err = config
                .reload_from_pem("not a cert", "not a key")
                .await
                .unwrap_err();

            assert!(matches!(err, TlsError::Pem(_)));
            let after = config.current_config();
            assert!(Arc::ptr_eq(&before, &after));
        }
    }
}

/// native-tls backend.
#[cfg(feature = "native-tls")]
pub mod native {
    use std::path::Path;
    use std::sync::Arc;

    use tokio::net::TcpStream;

    use crate::Result;

    /// native-tls server configuration.
    ///
    /// ALPN is best-effort and platform-dependent. Rustls is Arvik's
    /// guaranteed HTTP/2 ALPN backend.
    #[derive(Clone)]
    pub struct NativeTlsConfig {
        acceptor: Arc<tokio_native_tls::TlsAcceptor>,
    }

    impl NativeTlsConfig {
        /// Build a native-tls config from PKCS#12 / PFX bytes.
        pub fn from_pkcs12(data: impl AsRef<[u8]>, password: &str) -> Result<Self> {
            let identity = native_tls::Identity::from_pkcs12(data.as_ref(), password)?;
            let mut builder = native_tls::TlsAcceptor::builder(identity);
            builder.accept_alpn(&["h2", "http/1.1"]);
            let acceptor = builder.build()?;
            Ok(Self {
                acceptor: Arc::new(tokio_native_tls::TlsAcceptor::from(acceptor)),
            })
        }

        /// Build a native-tls config from a PKCS#12 / PFX file.
        pub async fn from_pkcs12_file(path: impl AsRef<Path>, password: &str) -> Result<Self> {
            let data = tokio::fs::read(path).await?;
            Self::from_pkcs12(data, password)
        }

        /// Accept a TCP stream with native-tls.
        pub async fn accept(
            &self,
            stream: TcpStream,
        ) -> Result<tokio_native_tls::TlsStream<TcpStream>> {
            Ok(self.acceptor.accept(stream).await?)
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn invalid_pkcs12_is_rejected() {
            match NativeTlsConfig::from_pkcs12(b"not pkcs12", "password") {
                Ok(_) => panic!("invalid PKCS#12 archive was accepted"),
                Err(err) => {
                    let _ = format!("{err}");
                }
            }
        }
    }
}

/// Paths used by TLS examples and reload helpers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TlsFilePaths {
    /// Certificate PEM path.
    pub cert: PathBuf,
    /// Private key PEM path.
    pub key: PathBuf,
}
