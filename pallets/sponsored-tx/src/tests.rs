use crate::{mock::*, Error, Event};
use frame::{prelude::Get, testing_prelude::*};
use polkadot_sdk::{
	frame_support::dispatch::{Pays, PostDispatchInfo},
	pallet_balances,
	sp_runtime::{
		traits::{DispatchTransaction, TransactionExtension},
		transaction_validity::{InvalidTransaction, TransactionSource},
	},
};

fn sponsored_ext(
	tip: Balance,
	sponsor: Option<AccountId>,
) -> crate::SponsoredChargeTransactionPayment<Test> {
	// Centralize extension construction so test cases stay focused on the lifecycle they assert.
	crate::SponsoredChargeTransactionPayment::<Test>::new(tip, sponsor)
}

fn sponsored_post_dispatch_weight() -> polkadot_sdk::frame_support::weights::Weight {
	// Mirror the current placeholder settlement weight so accidental drift is explicit in tests.
	let db_weight: polkadot_sdk::frame_support::weights::RuntimeDbWeight =
		<Test as frame_system::Config>::DbWeight::get();
	db_weight.reads_writes(7, 7)
}

#[test]
fn register_sponsor_holds_budget() {
	new_test_ext().execute_with(|| {
		assert_ok!(SponsoredTx::register_sponsor(
			RuntimeOrigin::signed(1),
			200,
			policy(vec![2], 50)
		));
		assert_eq!(SponsoredTx::budget_on_hold(&1), 200);
		assert_eq!(SponsoredTx::pending_on_hold(&1), 0);
	});
}

#[test]
fn register_requires_non_empty_unique_allowlist() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			SponsoredTx::register_sponsor(RuntimeOrigin::signed(1), 200, policy(vec![], 50)),
			Error::<Test>::EmptyAllowlist
		);

		let dup_policy = crate::SponsorPolicy::<Test> {
			allowed_callers: vec![2, 2].try_into().unwrap(),
			max_fee_per_tx: 50,
		};
		assert_noop!(
			SponsoredTx::register_sponsor(RuntimeOrigin::signed(1), 200, dup_policy),
			Error::<Test>::DuplicateAllowedCaller
		);
	});
}

#[test]
fn register_requires_non_zero_budget() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			SponsoredTx::register_sponsor(RuntimeOrigin::signed(1), 0, policy(vec![2], 50)),
			Error::<Test>::ZeroBudget
		);
	});
}

#[test]
fn can_increase_decrease_pause_resume_and_unregister() {
	new_test_ext().execute_with(|| {
		assert_ok!(SponsoredTx::register_sponsor(
			RuntimeOrigin::signed(1),
			200,
			policy(vec![2], 50)
		));
		assert_ok!(SponsoredTx::increase_budget(RuntimeOrigin::signed(1), 25));
		assert_eq!(SponsoredTx::budget_on_hold(&1), 225);

		assert_ok!(SponsoredTx::decrease_budget(RuntimeOrigin::signed(1), 20));
		assert_eq!(SponsoredTx::budget_on_hold(&1), 205);

		assert_ok!(SponsoredTx::pause(RuntimeOrigin::signed(1)));
		assert_ok!(SponsoredTx::resume(RuntimeOrigin::signed(1)));
		assert_ok!(SponsoredTx::unregister(RuntimeOrigin::signed(1)));
		assert!(SponsoredTx::sponsor_state(&1).is_none());
		assert_eq!(SponsoredTx::budget_on_hold(&1), 0);
	});
}

#[test]
fn set_policy_replaces_allowlist_and_fee_cap() {
	new_test_ext().execute_with(|| {
		assert_ok!(SponsoredTx::register_sponsor(
			RuntimeOrigin::signed(1),
			200,
			policy(vec![2], 50)
		));
		let updated = policy(vec![3], 120);

		assert_ok!(SponsoredTx::set_policy(RuntimeOrigin::signed(1), updated.clone()));
		assert_eq!(SponsoredTx::sponsor_state(&1).unwrap().policy, updated);

		let call = remark_call();
		let info = call.get_dispatch_info();
		let len = call.encoded_size();
		let ext = sponsored_ext(0, Some(1));

		assert_eq!(
			ext.validate_only(
				RuntimeOrigin::signed(2),
				&call,
				&info,
				len,
				TransactionSource::External,
				EXT_VERSION,
			)
			.unwrap_err(),
			InvalidTransaction::Custom(3).into()
		);
		assert_ok!(ext.validate_only(
			RuntimeOrigin::signed(3),
			&call,
			&info,
			len,
			TransactionSource::External,
			EXT_VERSION,
		));
	});
}

#[test]
fn sponsored_validation_accepts_allowlisted_signer() {
	new_test_ext().execute_with(|| {
		assert_ok!(SponsoredTx::register_sponsor(
			RuntimeOrigin::signed(1),
			200,
			policy(vec![2], 50)
		));
		let call = remark_call();
		let info = call.get_dispatch_info();
		let len = call.encoded_size();
		let ext = sponsored_ext(0, Some(1));

		assert_ok!(ext.validate_only(
			RuntimeOrigin::signed(2),
			&call,
			&info,
			len,
			TransactionSource::External,
			EXT_VERSION,
		));
	});
}

#[test]
fn sponsored_validation_rejects_paused_sponsor() {
	new_test_ext().execute_with(|| {
		assert_ok!(SponsoredTx::register_sponsor(
			RuntimeOrigin::signed(1),
			200,
			policy(vec![2], 50)
		));
		assert_ok!(SponsoredTx::pause(RuntimeOrigin::signed(1)));

		let call = remark_call();
		let info = call.get_dispatch_info();
		let len = call.encoded_size();
		let ext = sponsored_ext(0, Some(1));

		assert_eq!(
			ext.validate_only(
				RuntimeOrigin::signed(2),
				&call,
				&info,
				len,
				TransactionSource::External,
				EXT_VERSION,
			)
			.unwrap_err(),
			InvalidTransaction::Custom(2).into()
		);
	});
}

#[test]
fn sponsored_validation_rejects_non_allowlisted_signer() {
	new_test_ext().execute_with(|| {
		assert_ok!(SponsoredTx::register_sponsor(
			RuntimeOrigin::signed(1),
			200,
			policy(vec![2], 50)
		));
		let call = remark_call();
		let info = call.get_dispatch_info();
		let len = call.encoded_size();
		let ext = sponsored_ext(0, Some(1));

		let err = ext
			.validate_only(
				RuntimeOrigin::signed(3),
				&call,
				&info,
				len,
				TransactionSource::External,
				EXT_VERSION,
			)
			.unwrap_err();
		assert_eq!(err, InvalidTransaction::Custom(3).into());
	});
}

#[test]
fn sponsored_validation_rejects_fee_cap_exceeded() {
	new_test_ext().execute_with(|| {
		let call = remark_call();
		let info = call.get_dispatch_info();
		let len = call.encoded_size();
		let estimated_fee = polkadot_sdk::pallet_transaction_payment::Pallet::<Test>::compute_fee(
			len as u32, &info, 0,
		);

		assert_ok!(SponsoredTx::register_sponsor(
			RuntimeOrigin::signed(1),
			200,
			policy(vec![2], estimated_fee - 1)
		));
		let ext = sponsored_ext(0, Some(1));

		assert_eq!(
			ext.validate_only(
				RuntimeOrigin::signed(2),
				&call,
				&info,
				len,
				TransactionSource::External,
				EXT_VERSION,
			)
			.unwrap_err(),
			InvalidTransaction::Custom(4).into()
		);
	});
}

#[test]
fn sponsored_validation_rejects_insufficient_budget() {
	new_test_ext().execute_with(|| {
		let call = remark_call();
		let info = call.get_dispatch_info();
		let len = call.encoded_size();
		let estimated_fee = polkadot_sdk::pallet_transaction_payment::Pallet::<Test>::compute_fee(
			len as u32, &info, 0,
		);
		assert!(estimated_fee > 1);

		assert_ok!(SponsoredTx::register_sponsor(
			RuntimeOrigin::signed(1),
			estimated_fee - 1,
			policy(vec![2], estimated_fee + 10)
		));
		let ext = sponsored_ext(0, Some(1));

		assert_eq!(
			ext.validate_only(
				RuntimeOrigin::signed(2),
				&call,
				&info,
				len,
				TransactionSource::External,
				EXT_VERSION,
			)
			.unwrap_err(),
			InvalidTransaction::Custom(5).into()
		);
	});
}

#[test]
fn sponsored_prepare_and_post_dispatch_exactly_settle_refund_and_report_weight() {
	new_test_ext().execute_with(|| {
		assert_ok!(SponsoredTx::register_sponsor(
			RuntimeOrigin::signed(1),
			200,
			policy(vec![2], 50)
		));
		let call = remark_call();
		let info = call.get_dispatch_info();
		let len = call.encoded_size();
		let ext = sponsored_ext(0, Some(1));

		let estimated_fee = polkadot_sdk::pallet_transaction_payment::Pallet::<Test>::compute_fee(
			len as u32, &info, 0,
		);
		let (pre, _) = ext
			.validate_and_prepare(RuntimeOrigin::signed(2), &call, &info, len, EXT_VERSION)
			.unwrap();

		assert_eq!(SponsoredTx::budget_on_hold(&1), 200 - estimated_fee);
		assert_eq!(SponsoredTx::pending_on_hold(&1), estimated_fee);

		let post_info =
			PostDispatchInfo { actual_weight: Some(info.call_weight / 2), pays_fee: Pays::Yes };
		let actual_fee =
			polkadot_sdk::pallet_transaction_payment::Pallet::<Test>::compute_actual_fee(
				len as u32, &info, &post_info, 0,
			);
		let settlement_weight =
			crate::SponsoredChargeTransactionPayment::<Test>::post_dispatch_details(
				pre,
				&info,
				&post_info,
				len,
				&Ok(()),
			)
			.unwrap();

		assert_eq!(SponsoredTx::pending_on_hold(&1), 0);
		assert_eq!(SponsoredTx::budget_on_hold(&1), 200 - actual_fee);
		assert_eq!(settlement_weight, sponsored_post_dispatch_weight());
		System::assert_has_event(
			Event::SponsoredTransactionFeePaid { sponsor: 1, signer: 2, actual_fee, tip: 0 }.into(),
		);
	});
}

#[test]
fn sponsored_post_dispatch_splits_tip_and_fee_in_event() {
	new_test_ext().execute_with(|| {
		let tip = 5;
		assert_ok!(SponsoredTx::register_sponsor(
			RuntimeOrigin::signed(1),
			300,
			policy(vec![2], 120)
		));
		let call = remark_call();
		let info = call.get_dispatch_info();
		let len = call.encoded_size();
		let ext = sponsored_ext(tip, Some(1));

		let estimated_fee_with_tip =
			polkadot_sdk::pallet_transaction_payment::Pallet::<Test>::compute_fee(
				len as u32, &info, tip,
			);
		let (pre, _) = ext
			.validate_and_prepare(RuntimeOrigin::signed(2), &call, &info, len, EXT_VERSION)
			.unwrap();

		assert_eq!(SponsoredTx::budget_on_hold(&1), 300 - estimated_fee_with_tip);
		assert_eq!(SponsoredTx::pending_on_hold(&1), estimated_fee_with_tip);

		let post_info =
			PostDispatchInfo { actual_weight: Some(info.call_weight / 2), pays_fee: Pays::Yes };
		let actual_fee_with_tip =
			polkadot_sdk::pallet_transaction_payment::Pallet::<Test>::compute_actual_fee(
				len as u32, &info, &post_info, tip,
			);
		let actual_tip = tip.min(actual_fee_with_tip);
		let settlement_weight =
			crate::SponsoredChargeTransactionPayment::<Test>::post_dispatch_details(
				pre,
				&info,
				&post_info,
				len,
				&Ok(()),
			)
			.unwrap();

		assert_eq!(SponsoredTx::pending_on_hold(&1), 0);
		assert_eq!(SponsoredTx::budget_on_hold(&1), 300 - actual_fee_with_tip);
		assert_eq!(settlement_weight, sponsored_post_dispatch_weight());
		System::assert_has_event(
			Event::SponsoredTransactionFeePaid {
				sponsor: 1,
				signer: 2,
				actual_fee: actual_fee_with_tip,
				tip: actual_tip,
			}
			.into(),
		);
	});
}

#[test]
fn unregister_rejects_when_pending_budget_is_not_empty() {
	new_test_ext().execute_with(|| {
		assert_ok!(SponsoredTx::register_sponsor(
			RuntimeOrigin::signed(1),
			200,
			policy(vec![2], 50)
		));
		let call = remark_call();
		let info = call.get_dispatch_info();
		let len = call.encoded_size();

		// Prepare without post-dispatch leaves the per-tx reservation in pending on purpose.
		let (_pre, _) = sponsored_ext(0, Some(1))
			.validate_and_prepare(RuntimeOrigin::signed(2), &call, &info, len, EXT_VERSION)
			.unwrap();

		assert!(SponsoredTx::pending_on_hold(&1) > 0);
		assert_noop!(
			SponsoredTx::unregister(RuntimeOrigin::signed(1)),
			Error::<Test>::PendingBudgetNotEmpty
		);
	});
}

#[test]
fn multiple_sponsored_transactions_preserve_budget_accounting() {
	new_test_ext().execute_with(|| {
		assert_ok!(SponsoredTx::register_sponsor(
			RuntimeOrigin::signed(1),
			400,
			policy(vec![2, 3], 120)
		));
		let call = remark_call();
		let info = call.get_dispatch_info();
		let len = call.encoded_size();
		let ext = sponsored_ext(0, Some(1));

		// The mock runtime cannot model true parallel execution, so we assert the same sponsor
		// invariant with back-to-back settlements against one budget.
		let (pre_one, _) = ext
			.clone()
			.validate_and_prepare(RuntimeOrigin::signed(2), &call, &info, len, EXT_VERSION)
			.unwrap();
		let post_info_one =
			PostDispatchInfo { actual_weight: Some(info.call_weight / 2), pays_fee: Pays::Yes };
		let actual_fee_one =
			polkadot_sdk::pallet_transaction_payment::Pallet::<Test>::compute_actual_fee(
				len as u32,
				&info,
				&post_info_one,
				0,
			);
		let weight_one = crate::SponsoredChargeTransactionPayment::<Test>::post_dispatch_details(
			pre_one,
			&info,
			&post_info_one,
			len,
			&Ok(()),
		)
		.unwrap();

		assert_eq!(SponsoredTx::pending_on_hold(&1), 0);
		assert_eq!(SponsoredTx::budget_on_hold(&1), 400 - actual_fee_one);
		assert_eq!(weight_one, sponsored_post_dispatch_weight());

		let (pre_two, _) = ext
			.validate_and_prepare(RuntimeOrigin::signed(3), &call, &info, len, EXT_VERSION)
			.unwrap();
		let post_info_two =
			PostDispatchInfo { actual_weight: Some(info.call_weight), pays_fee: Pays::Yes };
		let actual_fee_two =
			polkadot_sdk::pallet_transaction_payment::Pallet::<Test>::compute_actual_fee(
				len as u32,
				&info,
				&post_info_two,
				0,
			);
		let weight_two = crate::SponsoredChargeTransactionPayment::<Test>::post_dispatch_details(
			pre_two,
			&info,
			&post_info_two,
			len,
			&Ok(()),
		)
		.unwrap();

		assert_eq!(SponsoredTx::pending_on_hold(&1), 0);
		assert_eq!(SponsoredTx::budget_on_hold(&1), 400 - actual_fee_one - actual_fee_two);
		assert_eq!(weight_two, sponsored_post_dispatch_weight());
	});
}

#[test]
fn unsponsored_path_keeps_signer_payment_behavior() {
	new_test_ext().execute_with(|| {
		let call = remark_call();
		let info = call.get_dispatch_info();
		let len = call.encoded_size();
		let signer_before = pallet_balances::Pallet::<Test>::free_balance(&2);
		let ext = sponsored_ext(0, None);

		let result =
			ext.dispatch_transaction(RuntimeOrigin::signed(2), call, &info, len, EXT_VERSION);
		assert!(result.is_ok());
		assert!(pallet_balances::Pallet::<Test>::free_balance(&2) < signer_before);
	});
}
