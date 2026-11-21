//! Registries and permissions: duplicate keys, drop handles, owners, sorted names and the local owner's permissions.

use massorca_server::{
    CapabilityRegistry, Owner, Permission, Principal, RegistrationError, Registry, authorize,
};

#[test]
fn a_registry_refuses_duplicate_keys_and_unregisters_on_drop() {
    let registry: Registry<u32> = Registry::default();
    let first = registry
        .register(Owner::Core, "alpha", 1)
        .expect("the first registration succeeds");

    let duplicate = registry
        .register(Owner::Core, "alpha", 2)
        .expect_err("the key is taken");
    assert_eq!(
        duplicate,
        RegistrationError::Duplicate {
            key: "alpha".to_owned()
        }
    );
    assert_eq!(registry.get("alpha").map(|entry| entry.value), Some(1));

    drop(first);
    assert!(
        registry.get("alpha").is_none(),
        "dropping the handle unregisters"
    );
    let _again = registry
        .register(Owner::Core, "alpha", 3)
        .expect("the key is free again");
}

#[test]
fn a_registry_lists_entries_in_key_order_with_their_owners() {
    let registry: Registry<&'static str> = Registry::default();
    let _beta = registry
        .register(Owner::Extension("demo".to_owned()), "beta", "b")
        .expect("beta registers");
    let _alpha = registry
        .register(Owner::Core, "alpha", "a")
        .expect("alpha registers");

    let listed: Vec<(String, Owner)> = registry
        .list()
        .iter()
        .map(|entry| (entry.key.clone(), entry.owner.clone()))
        .collect();
    assert_eq!(
        listed,
        vec![
            ("alpha".to_owned(), Owner::Core),
            ("beta".to_owned(), Owner::Extension("demo".to_owned())),
        ]
    );
}

#[test]
fn owners_display_and_parse_their_wire_form() {
    assert_eq!(Owner::Core.to_string(), "core");
    assert_eq!(Owner::Extension("demo".to_owned()).to_string(), "ext:demo");
    assert_eq!(Owner::parse("core"), Some(Owner::Core));
    assert_eq!(
        Owner::parse("ext:demo"),
        Some(Owner::Extension("demo".to_owned()))
    );
    assert_eq!(Owner::parse("ext:"), None);
    assert_eq!(Owner::parse("someone"), None);
}

#[test]
fn the_local_owner_holds_every_permission() {
    for permission in [
        Permission::Read,
        Permission::Workspace,
        Permission::HostAdmin,
    ] {
        assert!(
            authorize(&Principal::Owner, permission).is_ok(),
            "the owner may use {permission:?}"
        );
    }
}

#[test]
fn capability_names_are_sorted_and_dropping_a_handle_withdraws_it() {
    let capabilities = CapabilityRegistry::default();
    let events = capabilities
        .register("events/1", Owner::Core)
        .expect("events/1 registers");
    let _service = capabilities
        .register("service/1", Owner::Core)
        .expect("service/1 registers");
    assert_eq!(
        capabilities.names(),
        vec!["events/1".to_owned(), "service/1".to_owned()]
    );

    drop(events);
    assert_eq!(capabilities.names(), vec!["service/1".to_owned()]);
}
