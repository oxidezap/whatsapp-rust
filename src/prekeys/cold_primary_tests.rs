use super::{authenticate_cold_primary_bundles, authenticated_primary_identity};
use wacore::libsignal::protocol::{IdentityKey, KeyPair, PreKeyBundle};
use wacore::prekeys::PreKeyFetchOutcome;
use wacore_binary::Jid;

fn bundle(valid: bool) -> PreKeyBundle {
    let mut rng = rand::make_rng::<rand::rngs::StdRng>();
    let identity = KeyPair::generate(&mut rng);
    let signed = KeyPair::generate(&mut rng);
    let mut signature = identity
        .private_key
        .calculate_signature(&signed.public_key.serialize(), &mut rng)
        .unwrap()
        .to_vec();
    if !valid {
        signature[0] ^= 1;
    }
    PreKeyBundle::new(
        1,
        0u32.into(),
        None,
        1u32.into(),
        signed.public_key,
        signature,
        IdentityKey::new(identity.public_key),
    )
    .unwrap()
}

#[test]
fn each_cold_primary_signature_is_verified_once_before_publication() {
    for devices in [1, 3] {
        let primary = Jid::pn("12025550124");
        let companions: Vec<_> = (1..=devices).map(|id| primary.with_device(id)).collect();
        let mut outcome = PreKeyFetchOutcome::default();
        let primary_bundle = bundle(true);
        let expected = *primary_bundle.identity_key().unwrap();
        outcome
            .bundles
            .insert(primary.clone(), primary_bundle.clone());
        for companion in &companions {
            outcome
                .bundles
                .insert(companion.clone(), primary_bundle.clone());
        }
        let mut checks = 0;
        authenticate_cold_primary_bundles(
            &mut outcome,
            &companions,
            std::slice::from_ref(&primary),
            |bundle| {
                checks += 1;
                authenticated_primary_identity(bundle)
            },
        )
        .unwrap();
        assert_eq!(checks, 1);
        assert_eq!(*outcome.bundles[&primary].identity_key().unwrap(), expected);
        for companion in &companions {
            assert!(outcome.bundles.contains_key(companion));
        }
    }
}

#[test]
fn different_accounts_each_require_their_own_valid_signature() {
    let primaries = [Jid::pn("12025550124"), Jid::lid("100000000000125")];
    let companions: Vec<_> = primaries
        .iter()
        .map(|primary| primary.with_device(7))
        .collect();
    let mut outcome = PreKeyFetchOutcome::default();
    let mut expected = Vec::new();
    for primary in &primaries {
        let bundle = bundle(true);
        expected.push((primary.clone(), *bundle.identity_key().unwrap()));
        outcome.bundles.insert(primary.clone(), bundle);
    }
    let mut checks = 0;
    authenticate_cold_primary_bundles(&mut outcome, &companions, &primaries, |bundle| {
        checks += 1;
        authenticated_primary_identity(bundle)
    })
    .unwrap();
    assert_eq!(checks, 2);
    for (primary, identity) in expected {
        assert_eq!(*outcome.bundles[&primary].identity_key().unwrap(), identity);
    }
}

#[test]
fn an_invalid_later_primary_aborts_validation_before_publication() {
    let primaries = [Jid::pn("12025550124"), Jid::lid("100000000000125")];
    let companions: Vec<_> = primaries
        .iter()
        .map(|primary| primary.with_device(7))
        .collect();
    let mut outcome = PreKeyFetchOutcome::default();
    outcome.bundles.insert(primaries[0].clone(), bundle(true));
    outcome.bundles.insert(primaries[1].clone(), bundle(false));
    let mut checks = 0;
    let result =
        authenticate_cold_primary_bundles(&mut outcome, &companions, &primaries, |bundle| {
            checks += 1;
            authenticated_primary_identity(bundle)
        });
    assert_eq!(checks, 2);
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("invalid primary signed prekey signature")
    );
}

#[test]
fn missing_primaries_remove_every_unanchored_companion_without_trusting_its_bundle() {
    let primary = Jid::pn("12025550124");
    let companions = [primary.with_device(1), primary.with_device(7)];
    let mut outcome = PreKeyFetchOutcome::default();
    for companion in &companions {
        outcome.bundles.insert(companion.clone(), bundle(true));
    }
    authenticate_cold_primary_bundles(
        &mut outcome,
        &companions,
        std::slice::from_ref(&primary),
        |_| panic!("a companion must never authenticate its own missing primary"),
    )
    .unwrap();
    assert!(outcome.bundles.is_empty());
}

#[test]
fn unrelated_bundles_are_not_authenticated_when_no_account_is_cold() {
    let primary = Jid::pn("12025550124");
    let mut outcome = PreKeyFetchOutcome::default();
    outcome.bundles.insert(primary.clone(), bundle(false));
    authenticate_cold_primary_bundles(&mut outcome, &[], &[], |_| {
        panic!("no cold account requested validation")
    })
    .unwrap();
    assert!(outcome.bundles.contains_key(&primary));
}
