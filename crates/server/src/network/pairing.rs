//! Pending pairing offers and the limit on failed pairing attempts (decision 0041). Offers live in memory only, so a
//! restart retires every one of them. Only the SHA-256 of a secret is kept.

use std::collections::VecDeque;
use std::net::IpAddr;
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use massorca_protocol::methods::devices::Secret;
use massorca_protocol::{ErrorBody, codes};
use ring::digest::{SHA256, digest};
use ring::rand::{SecureRandom, SystemRandom};
use subtle::ConstantTimeEq;

use crate::permission::Scopes;

pub(crate) const DEFAULT_TTL: Duration = Duration::from_secs(600);
pub(crate) const MAX_TTL: Duration = Duration::from_secs(3600);
const MAX_PENDING: usize = 16;
/// Expired offers are kept this long so a late attempt hears "expired" rather than "invalid".
const EXPIRED_MEMORY: Duration = Duration::from_secs(3600);
const FAILURE_WINDOW: Duration = Duration::from_secs(300);
const FAILURES_PER_ADDRESS: usize = 5;
const FAILURES_OVERALL: usize = 30;

struct PendingOffer {
    secret_hash: [u8; 32],
    expires_at: Instant,
    scopes: Scopes,
}

#[derive(Default)]
struct Inner {
    offers: Vec<PendingOffer>,
    failures: VecDeque<(Instant, IpAddr)>,
}

#[derive(Default)]
pub(crate) struct Pairing {
    inner: Mutex<Inner>,
}

impl std::fmt::Debug for Pairing {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Pairing").finish_non_exhaustive()
    }
}

impl Pairing {
    /// Mints a one-time secret valid for `ttl`. The secret is returned once and never stored.
    pub(crate) fn create(
        &self,
        ttl: Duration,
        scopes: Scopes,
        now: Instant,
    ) -> Result<Secret, ErrorBody> {
        let mut inner = self.lock();
        inner.prune(now);
        let live = inner
            .offers
            .iter()
            .filter(|offer| offer.expires_at > now)
            .count();
        if live >= MAX_PENDING {
            return Err(ErrorBody::new(
                codes::PAIRING_TOO_MANY_OFFERS,
                format!("{MAX_PENDING} offers are already pending; use or let one expire first"),
            ));
        }
        let mut bytes = [0_u8; 32];
        SystemRandom::new()
            .fill(&mut bytes)
            .map_err(|_| ErrorBody::new(codes::INTERNAL, "no randomness for a pairing secret"))?;
        let secret = URL_SAFE_NO_PAD.encode(bytes);
        inner.offers.push(PendingOffer {
            secret_hash: hash(&secret),
            expires_at: now + ttl,
            scopes,
        });
        Ok(Secret::new(secret))
    }

    /// Consumes the offer `secret` belongs to and returns its scopes. Rate limiting is checked first, so a limited
    /// address learns nothing about the secret.
    pub(crate) fn redeem(
        &self,
        secret: &str,
        peer: IpAddr,
        now: Instant,
    ) -> Result<Scopes, ErrorBody> {
        let mut inner = self.lock();
        inner.prune(now);
        let from_peer = inner.failures.iter().filter(|(_, ip)| *ip == peer).count();
        if from_peer >= FAILURES_PER_ADDRESS || inner.failures.len() >= FAILURES_OVERALL {
            return Err(ErrorBody::new(
                codes::PAIRING_RATE_LIMITED,
                "too many failed pairing attempts; try again in a few minutes",
            ));
        }
        let presented = hash(secret);
        // Every offer is compared, so the time taken does not say which one matched.
        let found = inner
            .offers
            .iter()
            .enumerate()
            .fold(None, |found, (index, offer)| {
                if bool::from(offer.secret_hash.ct_eq(&presented)) {
                    Some(index)
                } else {
                    found
                }
            });
        let Some(index) = found else {
            inner.failures.push_back((now, peer));
            return Err(ErrorBody::new(
                codes::PAIRING_INVALID,
                "the pairing offer is not valid here; it may have been used already",
            ));
        };
        let offer = inner.offers.swap_remove(index);
        if offer.expires_at <= now {
            inner.failures.push_back((now, peer));
            return Err(ErrorBody::new(
                codes::PAIRING_EXPIRED,
                "the pairing offer has expired; ask for a new one",
            ));
        }
        Ok(offer.scopes)
    }

    /// Offers that can still be used.
    pub(crate) fn pending(&self, now: Instant) -> usize {
        self.lock()
            .offers
            .iter()
            .filter(|offer| offer.expires_at > now)
            .count()
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl Inner {
    fn prune(&mut self, now: Instant) {
        self.offers
            .retain(|offer| offer.expires_at + EXPIRED_MEMORY > now);
        while self
            .failures
            .front()
            .is_some_and(|(at, _)| *at + FAILURE_WINDOW <= now)
        {
            self.failures.pop_front();
        }
    }
}

fn hash(secret: &str) -> [u8; 32] {
    let mut out = [0_u8; 32];
    out.copy_from_slice(digest(&SHA256, secret.as_bytes()).as_ref());
    out
}

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;

    use super::*;

    const PEER: IpAddr = IpAddr::V4(Ipv4Addr::new(100, 64, 0, 9));
    const OTHER: IpAddr = IpAddr::V4(Ipv4Addr::new(100, 64, 0, 10));

    fn code(result: Result<Scopes, ErrorBody>) -> String {
        result.expect_err("refused").code
    }

    #[test]
    fn an_offer_is_used_once() {
        let pairing = Pairing::default();
        let now = Instant::now();
        let secret = pairing.create(DEFAULT_TTL, Scopes::ADMIN, now).unwrap();
        assert_eq!(pairing.pending(now), 1);
        assert_eq!(
            pairing.redeem(secret.expose(), PEER, now).unwrap(),
            Scopes::ADMIN
        );
        assert_eq!(
            code(pairing.redeem(secret.expose(), PEER, now)),
            codes::PAIRING_INVALID
        );
        assert_eq!(pairing.pending(now), 0);
    }

    #[test]
    fn an_expired_offer_says_so_and_is_gone() {
        let pairing = Pairing::default();
        let now = Instant::now();
        let secret = pairing
            .create(Duration::from_secs(1), Scopes::STANDARD, now)
            .unwrap();
        let later = now + Duration::from_secs(2);
        assert_eq!(
            code(pairing.redeem(secret.expose(), PEER, later)),
            codes::PAIRING_EXPIRED
        );
        assert_eq!(
            code(pairing.redeem(secret.expose(), PEER, later)),
            codes::PAIRING_INVALID
        );
    }

    #[test]
    fn failures_are_limited_per_address_then_forgiven() {
        let pairing = Pairing::default();
        let now = Instant::now();
        let secret = pairing.create(DEFAULT_TTL, Scopes::STANDARD, now).unwrap();
        for _ in 0..FAILURES_PER_ADDRESS {
            assert_eq!(
                code(pairing.redeem("wrong", PEER, now)),
                codes::PAIRING_INVALID
            );
        }
        // Even the right secret is refused while the address is limited, and the offer survives.
        assert_eq!(
            code(pairing.redeem(secret.expose(), PEER, now)),
            codes::PAIRING_RATE_LIMITED
        );
        assert_eq!(
            pairing.redeem(secret.expose(), OTHER, now).unwrap(),
            Scopes::STANDARD
        );
        let after = now + FAILURE_WINDOW;
        assert_eq!(
            code(pairing.redeem("wrong", PEER, after)),
            codes::PAIRING_INVALID
        );
    }

    #[test]
    fn failures_are_limited_overall() {
        let pairing = Pairing::default();
        let now = Instant::now();
        for host in 0..FAILURES_OVERALL {
            let ip = IpAddr::V4(Ipv4Addr::new(10, 0, 0, u8::try_from(host).unwrap()));
            assert_eq!(
                code(pairing.redeem("wrong", ip, now)),
                codes::PAIRING_INVALID
            );
        }
        assert_eq!(
            code(pairing.redeem("wrong", OTHER, now)),
            codes::PAIRING_RATE_LIMITED
        );
    }

    #[test]
    fn pending_offers_are_capped() {
        let pairing = Pairing::default();
        let now = Instant::now();
        for _ in 0..MAX_PENDING {
            pairing.create(DEFAULT_TTL, Scopes::STANDARD, now).unwrap();
        }
        let refused = pairing
            .create(DEFAULT_TTL, Scopes::STANDARD, now)
            .expect_err("capped");
        assert_eq!(refused.code, codes::PAIRING_TOO_MANY_OFFERS);
    }

    #[test]
    fn secrets_are_fresh_and_long() {
        let pairing = Pairing::default();
        let now = Instant::now();
        let a = pairing.create(DEFAULT_TTL, Scopes::STANDARD, now).unwrap();
        let b = pairing.create(DEFAULT_TTL, Scopes::STANDARD, now).unwrap();
        assert_ne!(a, b);
        assert_eq!(a.expose().len(), 43);
    }
}
