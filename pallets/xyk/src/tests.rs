use crate::{mock::*, Error};
use frame_support::{assert_noop, assert_ok};
use frame_system::EventRecord;

type Assets = pallet_assets::Pallet<Test>;
type Xyk = crate::Pallet<Test>;

#[test]
fn create_lp() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Xyk::create_lp(Origin::signed(ALICE), X, Balance::MAX, Y, 100_000),
            Error::<Test>::NotEnoughTokens
        );

        assert_noop!(
            Xyk::create_lp(Origin::signed(ALICE), X, Balance::MAX, Y, 100_000),
            Error::<Test>::NotEnoughTokens
        );

        assert_ok!(Xyk::create_lp(Origin::signed(ALICE), X, 100_000, Y, 100_000));

        assert_eq!(Assets::balance(X, ALICE), MINT - 100_000);
        assert_eq!(Assets::balance(Y, ALICE), MINT - 100_000);
        assert_eq!(Assets::balance(Y, ALICE), MINT - 100_000);

        assert_noop!(
            Xyk::create_lp(Origin::signed(ALICE), X, 100_000, Y, 100_000),
            Error::<Test>::LiquidityAlreadyExists
        );
    });
}

#[test]
fn create_lp_and_swap() {
    let balances = new_test_ext().execute_with(|| {
        assert_ok!(Xyk::create_lp(Origin::signed(ALICE), X, 100_000, Y, 1_000_000_000));

        for _ in 0..10 {
            assert_ok!(Xyk::swap(Origin::signed(ALICE), X, 1000, Y));
        }

        (Assets::balance(X, ALICE), Assets::balance(X, DEXAddr::get()))
    });

    let new_balances = new_test_ext().execute_with(|| {
        assert_ok!(Xyk::create_lp(Origin::signed(ALICE), X, 100_000, Y, 1_000_000_000));
        assert_ok!(Xyk::swap(Origin::signed(ALICE), X, 10_000, Y));

        (Assets::balance(X, ALICE), Assets::balance(X, DEXAddr::get()))
    });

    assert_eq!(balances, new_balances);
}

#[test]
fn create_lock_lp_and_unlock() {
    new_test_ext().execute_with(|| {
        assert_ok!(Xyk::create_lp(Origin::signed(ALICE), X, 10_000, Y, 10_000));

        let (lp, lp_amount) = frame_system::Pallet::<Test>::events()
            .into_iter()
            .find_map(|EventRecord { event, .. }| match event {
                Event::Xyk(crate::Event::LPMinted { lp, .. }) => Some(lp),
                _ => None,
            })
            .unwrap();

        assert_ok!(Xyk::lock(Origin::signed(ALICE), X, 10_000, Y));

        assert_eq!(Assets::balance(X, ALICE), MINT - 20_000);
        assert_eq!(Assets::balance(Y, ALICE), MINT - 20_000);
        assert_eq!(Assets::balance(lp, ALICE), 2 * lp_amount);
    })
}
