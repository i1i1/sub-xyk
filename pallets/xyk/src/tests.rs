use crate::{mock::*, pallet::AssetId, Error};
use frame_support::{assert_noop, assert_ok};

type Assets = pallet_assets::Pallet<Test>;
type Xyk = crate::Pallet<Test>;

const ADDR: AccountId = 0x1;

fn create_pair_of_tokens() -> (AssetId<Test>, AssetId<Test>) {
    dbg!(Assets::create(Origin::signed(1), 0x1337, ADDR, 1)).unwrap();
    dbg!(Assets::create(Origin::signed(1), 0x1338, ADDR, 1)).unwrap();
    dbg!(Assets::mint(Origin::signed(1), 0x1337, ADDR, 1_000_000)).unwrap();
    dbg!(Assets::mint(Origin::signed(1), 0x1338, ADDR, 1_000_000)).unwrap();
    (0x1337, 0x1338)
}

#[test]
fn create_lp() {
    new_test_ext().execute_with(|| {
        let (x, y) = create_pair_of_tokens();
        // Xyk::create_lp(Origin::signed(1), (x, 1000), (y, 100_000)).unwrap();
        // Dispatch a signed extrinsic.
        // assert_ok!(Xyk::do_something(Origin::signed(1), 42));
        // Read pallet storage and assert an expected result.
        // assert_eq!(TemplateModule::something(), Some(42));
    });
}

#[test]
fn correct_error_for_none_value() {
    new_test_ext().execute_with(|| {
        // Ensure the expected error is thrown when no value is present.
        // assert_noop!(TemplateModule::cause_error(Origin::signed(1)), Error::<Test>::NoneValue);
    });
}
