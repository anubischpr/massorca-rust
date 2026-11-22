//! The secret vault's building blocks (decision 0071): the redacted [`SecretValue`], name and scope rules, the
//! XChaCha20-Poly1305 cipher and the OS-protected master key. No value, key byte or ciphertext reaches any error,
//! `Debug` output or log line.
