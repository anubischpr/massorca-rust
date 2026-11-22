use massorca_core_vault::{Aad, MasterKey, NONCE_LEN, Scope, SecretValue, open, seal};

fn key() -> MasterKey {
    MasterKey::generate().expect("OS randomness")
}

fn project() -> Scope {
    Scope::Project("p1".to_owned())
}

#[test]
fn sealed_value_opens_with_the_same_key_and_aad() {
    let key = key();
    let aad = Aad::new("row-1", &project(), "API_KEY").expect("valid aad");
    let value = SecretValue::new("s3cret".to_owned()).expect("value");
    let sealed = seal(&key, &aad, &value).expect("seal");
    assert_eq!(sealed.nonce.len(), NONCE_LEN);
    assert!(!sealed.ciphertext.windows(6).any(|w| w == b"s3cret"));
    assert!(!format!("{sealed:?}").contains("s3cret"));
    assert_eq!(format!("{key:?}"), "MasterKey([redacted])");
    let opened = open(&key, &aad, &sealed.nonce, &sealed.ciphertext).expect("open");
    assert_eq!(opened.expose(), "s3cret");
}

#[test]
fn each_seal_uses_a_fresh_nonce() {
    let key = key();
    let aad = Aad::new("row-1", &Scope::Global, "A").expect("aad");
    let value = SecretValue::new("v".to_owned()).expect("value");
    let first = seal(&key, &aad, &value).expect("seal");
    let second = seal(&key, &aad, &value).expect("seal");
    assert_ne!(first.nonce, second.nonce);
}

#[test]
fn tampering_or_moving_the_ciphertext_fails_to_open() {
    let key = key();
    let aad = Aad::new("row-1", &project(), "API_KEY").expect("aad");
    let value = SecretValue::new("s3cret".to_owned()).expect("value");
    let sealed = seal(&key, &aad, &value).expect("seal");

    let mut tampered = sealed.ciphertext.clone();
    tampered[0] ^= 1;
    assert!(open(&key, &aad, &sealed.nonce, &tampered).is_err());

    let others = [
        Aad::new("row-2", &project(), "API_KEY").expect("aad"),
        Aad::new("row-1", &Scope::Team("p1".to_owned()), "API_KEY").expect("aad"),
        Aad::new("row-1", &project(), "API_KEY2").expect("aad"),
    ];
    for other in &others {
        assert!(open(&key, other, &sealed.nonce, &sealed.ciphertext).is_err());
    }
    let stranger = MasterKey::generate().expect("key");
    assert!(open(&stranger, &aad, &sealed.nonce, &sealed.ciphertext).is_err());
    assert!(open(&key, &aad, &sealed.nonce[..12], &sealed.ciphertext).is_err());
}

#[test]
fn aad_refuses_invalid_parts() {
    assert!(Aad::new("", &Scope::Global, "A").is_err());
    assert!(Aad::new("row\0", &Scope::Global, "A").is_err());
    assert!(Aad::new("row", &Scope::Global, "1A").is_err());
}
