//! Pending pairing offers and the limit on failed pairing attempts (decision 0041). Offers live in memory only, so a
//! restart retires every one of them. Only the SHA-256 of a secret is kept.

use std::collections::VecDeque;
use std::net::IpAddr;
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use massorca_core_store::now_ms;
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
    /// Names the offer for retiring it; not a credential.
    id: String,
    secret_hash: [u8; 32],
    /// Decides expiry; the wall-clock `expires_at_ms` is only reported.
    expires_at: Instant,
    expires_at_ms: i64,
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

/// A new offer: its id, and its secret, returned once and never stored.
#[derive(Debug)]
pub(crate) struct Minted {
    pub(crate) id: String,
    pub(crate) secret: Secret,
    pub(crate) expires_at_ms: i64,
}

/// A pending offer as the service reports it: what names it and when it ends, never its secret.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PendingOfferInfo {
    pub(crate) offer_id: String,
    pub(crate) expires_at_ms: i64,
    pub(crate) scopes: Scopes,
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
    /// Mints a one-time secret valid for `ttl`, and an id that names the offer. The secret is returned once and never
    /// stored.
    pub(crate) fn create(
        &self,
        ttl: Duration,
        scopes: Scopes,
        now: Instant,
    ) -> Result<Minted, ErrorBody> {
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
        let random = SystemRandom::new();
        let mut bytes = [0_u8; 32];
        let mut id = [0_u8; 9];
        random
            .fill(&mut bytes)
            .and_then(|()| random.fill(&mut id))
            .map_err(|_| ErrorBody::new(codes::INTERNAL, "no randomness for a pairing secret"))?;
        let secret = URL_SAFE_NO_PAD.encode(bytes);
        let id = URL_SAFE_NO_PAD.encode(id);
        let expires_at_ms =
            now_ms().saturating_add(i64::try_from(ttl.as_millis()).unwrap_or(i64::MAX));
        inner.offers.push(PendingOffer {
            id: id.clone(),
            secret_hash: hash(&secret),
            expires_at: now + ttl,
            expires_at_ms,
            scopes,
        });
        Ok(Minted {
            id,
            secret: Secret::new(secret),
            expires_at_ms,
        })
    }

    /// Retires the pending offer `id` names, so its secret no longer pairs. False when no such offer is pending: it
    /// was never made, is used or being used, or has expired.
    pub(crate) fn cancel(&self, id: &str, now: Instant) -> bool {
        let mut inner = self.lock();
        inner.prune(now);
        let before = inner.offers.len();
        inner
            .offers
            .retain(|offer| offer.id != id || offer.expires_at <= now);
        inner.offers.len() < before
    }

    /// Retires every pending offer, and answers how many there were.
    pub(crate) fn cancel_all(&self, now: Instant) -> usize {
        let mut inner = self.lock();
        inner.prune(now);
        let before = inner.offers.len();
        inner.offers.retain(|offer| offer.expires_at <= now);
        before - inner.offers.len()
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
        self.pending_offers(now).len()
    }

    /// The offers that can still be used, with their ids, wall-clock expiry and scopes.
    pub(crate) fn pending_offers(&self, now: Instant) -> Vec<PendingOfferInfo> {
        self.lock()
            .offers
            .iter()
            .filter(|offer| offer.expires_at > now)
            .map(|offer| PendingOfferInfo {
                offer_id: offer.id.clone(),
                expires_at_ms: offer.expires_at_ms,
                scopes: offer.scopes,
            })
            .collect()
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
        let secret = pairing
            .create(DEFAULT_TTL, Scopes::ADMIN, now)
            .unwrap()
            .secret;
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
            .unwrap()
            .secret;
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
        let secret = pairing
            .create(DEFAULT_TTL, Scopes::STANDARD, now)
            .unwrap()
            .secret;
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
        let secret = pairing
            .create(DEFAULT_TTL, Scopes::STANDARD, now)
            .unwrap()
            .secret;
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
        let secret = pairing
            .create(DEFAULT_TTL, Scopes::STANDARD, now)
            .unwrap()
            .secret;
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
        let secret = pairing
            .create(DEFAULT_TTL, Scopes::ADMIN, now)
            .unwrap()
            .secret;
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
    fn a_cancelled_offer_no_longer_pairs_and_others_stay() {
        let pairing = Pairing::default();
        let now = Instant::now();
        let first = pairing.create(DEFAULT_TTL, Scopes::ADMIN, now).unwrap();
        let second = pairing.create(DEFAULT_TTL, Scopes::ADMIN, now).unwrap();
        assert!(pairing.cancel(&first.id, now));
        assert!(!pairing.cancel(&first.id, now), "already retired");
        assert_eq!(
            code(pairing.redeem(first.secret.expose(), PEER, now)),
            codes::PAIRING_INVALID
        );
        assert_eq!(pairing.pending(now), 1);
        spend(&pairing, second.secret.expose(), OTHER, now);
        assert!(
            !pairing.cancel(&second.id, now),
            "a used offer is not pending"
        );
    }

    #[test]
    fn an_expired_offer_cannot_be_cancelled_and_all_counts_pending_ones() {
        let pairing = Pairing::default();
        let now = Instant::now();
        let short = pairing
            .create(Duration::from_secs(1), Scopes::ADMIN, now)
            .unwrap();
        pairing.create(DEFAULT_TTL, Scopes::ADMIN, now).unwrap();
        pairing.create(DEFAULT_TTL, Scopes::ADMIN, now).unwrap();
        let later = now + Duration::from_secs(2);
        assert!(!pairing.cancel(&short.id, later));
        assert_eq!(pairing.cancel_all(later), 2);
        assert_eq!(pairing.pending(later), 0);
        assert_eq!(pairing.cancel_all(later), 0);
    }

    #[test]
    fn a_pending_offer_lists_its_id_expiry_and_scopes_but_no_secret() {
        let pairing = Pairing::default();
        let now = Instant::now();
        let before = now_ms();
        let minted = pairing.create(DEFAULT_TTL, Scopes::ADMIN, now).unwrap();
        let listed = pairing.pending_offers(now);
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].offer_id, minted.id);
        assert_eq!(listed[0].expires_at_ms, minted.expires_at_ms);
        assert_eq!(listed[0].scopes, Scopes::ADMIN);
        let ttl_ms = i64::try_from(DEFAULT_TTL.as_millis()).unwrap();
        assert!(
            (before + ttl_ms..=now_ms() + ttl_ms).contains(&minted.expires_at_ms),
            "the wall-clock expiry is the creation time plus the ttl"
        );
        assert!(!format!("{listed:?}").contains(minted.secret.expose()));
        assert!(
            pairing.pending_offers(now + DEFAULT_TTL).is_empty(),
            "the Instant decides expiry, so an expired offer is not listed"
        );
    }

    #[test]
    fn secrets_are_fresh_and_long() {
        let pairing = Pairing::default();
        let now = Instant::now();
        let a = pairing
            .create(DEFAULT_TTL, Scopes::STANDARD, now)
            .unwrap()
            .secret;
        let b = pairing
            .create(DEFAULT_TTL, Scopes::STANDARD, now)
            .unwrap()
            .secret;
        assert_ne!(a, b);
        assert_eq!(a.expose().len(), 43);
    }
}
