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
/// Failures from every address together; reaching it only limits addresses that have failed themselves.
const FAILURES_OVERALL: usize = 30;
/// Failures remembered at most, so many addresses cannot grow the list without bound.
const FAILURES_REMEMBERED: usize = 1024;

struct PendingOffer {
    secret_hash: [u8; 32],
    expires_at: Instant,
    scopes: Scopes,
}

#[derive(Default)]
struct Inner {
    offers: Vec<PendingOffer>,
    failures: VecDeque<(Instant, FailureSource)>,
}

/// Where failed attempts are counted: one IPv4 address, or one IPv6 /64 (a host can use any address in its /64).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FailureSource {
    V4([u8; 4]),
    V6([u8; 8]),
}

impl FailureSource {
    fn of(peer: IpAddr) -> Self {
        match peer {
            IpAddr::V4(ip) => Self::V4(ip.octets()),
            IpAddr::V6(ip) => match ip.to_ipv4_mapped() {
                Some(v4) => Self::V4(v4.octets()),
                None => {
                    let mut prefix = [0_u8; 8];
                    prefix.copy_from_slice(&ip.octets()[..8]);
                    Self::V6(prefix)
                }
            },
        }
    }
}

/// An offer taken out of the pending list while its device is stored. Dropping it without [`commit`](Self::commit)
/// returns the offer, so a failed store does not spend it.
pub(crate) struct Reservation<'a> {
    pairing: &'a Pairing,
    offer: Option<PendingOffer>,
}

impl Reservation<'_> {
    pub(crate) fn scopes(&self) -> Scopes {
        self.offer
            .as_ref()
            .map(|offer| offer.scopes)
            .unwrap_or_default()
    }

    /// Spends the offer.
    pub(crate) fn commit(mut self) {
        self.offer = None;
    }
}

impl Drop for Reservation<'_> {
    fn drop(&mut self) {
        if let Some(offer) = self.offer.take() {
            self.pairing.lock().offers.push(offer);
        }
    }
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

    /// Takes the offer `secret` belongs to out of the pending list and holds it for the caller. Rate limiting is
    /// checked first, so a limited address learns nothing about the secret. The offer is spent only by
    /// [`Reservation::commit`]; dropping the reservation (the device could not be stored) puts it back.
    pub(crate) fn redeem(
        &self,
        secret: &str,
        peer: IpAddr,
        now: Instant,
    ) -> Result<Reservation<'_>, ErrorBody> {
        let mut inner = self.lock();
        inner.prune(now);
        let source = FailureSource::of(peer);
        let from_peer = inner
            .failures
            .iter()
            .filter(|(_, from)| *from == source)
            .count();
        // The overall cap only limits sources that have failed already, so nobody can lock out a clean address.
        if from_peer >= FAILURES_PER_ADDRESS
            || (from_peer > 0 && inner.failures.len() >= FAILURES_OVERALL)
        {
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
            inner.record_failure(now, source);
            return Err(ErrorBody::new(
                codes::PAIRING_INVALID,
                "the pairing offer is not valid here; it may have been used already",
            ));
        };
        let offer = inner.offers.swap_remove(index);
        if offer.expires_at <= now {
            inner.record_failure(now, source);
            return Err(ErrorBody::new(
                codes::PAIRING_EXPIRED,
                "the pairing offer has expired; ask for a new one",
            ));
        }
        Ok(Reservation {
            pairing: self,
            offer: Some(offer),
        })
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
    fn record_failure(&mut self, now: Instant, source: FailureSource) {
        if self.failures.len() >= FAILURES_REMEMBERED {
            self.failures.pop_front();
        }
        self.failures.push_back((now, source));
    }

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
    use std::net::{Ipv4Addr, Ipv6Addr};

    use super::*;

    const PEER: IpAddr = IpAddr::V4(Ipv4Addr::new(100, 64, 0, 9));
    const OTHER: IpAddr = IpAddr::V4(Ipv4Addr::new(100, 64, 0, 10));

    fn code(result: Result<Reservation<'_>, ErrorBody>) -> String {
        result.err().expect("refused").code
    }

    /// Redeems and spends the offer, like a pairing whose device was stored.
    fn spend(pairing: &Pairing, secret: &str, peer: IpAddr, now: Instant) -> Scopes {
        let reservation = pairing.redeem(secret, peer, now).expect("redeemed");
        let scopes = reservation.scopes();
        reservation.commit();
        scopes
    }

    #[test]
    fn an_offer_is_used_once() {
        let pairing = Pairing::default();
        let now = Instant::now();
        let secret = pairing.create(DEFAULT_TTL, Scopes::ADMIN, now).unwrap();
        assert_eq!(pairing.pending(now), 1);
        assert_eq!(spend(&pairing, secret.expose(), PEER, now), Scopes::ADMIN);
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
            spend(&pairing, secret.expose(), OTHER, now),
            Scopes::STANDARD
        );
        let after = now + FAILURE_WINDOW;
        assert_eq!(
            code(pairing.redeem("wrong", PEER, after)),
            codes::PAIRING_INVALID
        );
    }

    #[test]
    fn the_overall_limit_never_locks_out_a_clean_address() {
        let pairing = Pairing::default();
        let now = Instant::now();
        let secret = pairing.create(DEFAULT_TTL, Scopes::STANDARD, now).unwrap();
        for host in 0..FAILURES_OVERALL {
            let ip = IpAddr::V4(Ipv4Addr::new(10, 0, 0, u8::try_from(host).unwrap()));
            assert_eq!(
                code(pairing.redeem("wrong", ip, now)),
                codes::PAIRING_INVALID
            );
        }
        // An address that failed before is limited once the overall count is reached...
        let failed = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 0));
        assert_eq!(
            code(pairing.redeem(secret.expose(), failed, now)),
            codes::PAIRING_RATE_LIMITED
        );
        // ...but an address with no failures still pairs.
        assert_eq!(
            spend(&pairing, secret.expose(), OTHER, now),
            Scopes::STANDARD
        );
    }

    #[test]
    fn ipv6_failures_count_per_64_prefix() {
        let pairing = Pairing::default();
        let now = Instant::now();
        let secret = pairing.create(DEFAULT_TTL, Scopes::STANDARD, now).unwrap();
        for host in 0..FAILURES_PER_ADDRESS {
            let ip = IpAddr::V6(Ipv6Addr::new(
                0xfd7a,
                0x115c,
                0xa1e0,
                0,
                0,
                0,
                0,
                u16::try_from(host).unwrap() + 1,
            ));
            assert_eq!(
                code(pairing.redeem("wrong", ip, now)),
                codes::PAIRING_INVALID
            );
        }
        let same_prefix = IpAddr::V6(Ipv6Addr::new(0xfd7a, 0x115c, 0xa1e0, 0, 0xab, 0, 0, 0x99));
        assert_eq!(
            code(pairing.redeem(secret.expose(), same_prefix, now)),
            codes::PAIRING_RATE_LIMITED
        );
        let other_prefix = IpAddr::V6(Ipv6Addr::new(0xfd7a, 0x115c, 0xa1e0, 1, 0, 0, 0, 1));
        assert_eq!(
            spend(&pairing, secret.expose(), other_prefix, now),
            Scopes::STANDARD
        );
    }

    #[test]
    fn a_reservation_dropped_without_commit_keeps_the_offer() {
        let pairing = Pairing::default();
        let now = Instant::now();
        let secret = pairing.create(DEFAULT_TTL, Scopes::ADMIN, now).unwrap();
        let reservation = pairing.redeem(secret.expose(), PEER, now).unwrap();
        // While the device is being stored, nobody else can use the offer.
        assert_eq!(pairing.pending(now), 0);
        assert_eq!(
            code(pairing.redeem(secret.expose(), OTHER, now)),
            codes::PAIRING_INVALID
        );
        drop(reservation);
        assert_eq!(pairing.pending(now), 1);
        assert_eq!(spend(&pairing, secret.expose(), PEER, now), Scopes::ADMIN);
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
