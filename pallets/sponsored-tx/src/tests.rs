use crate::{mock::*, Error, Event};
use frame::testing_prelude::*;
use polkadot_sdk::{
	frame_support::dispatch::{Pays, PostDispatchInfo},
	pallet_balances,
	sp_runtime::{
		traits::{DispatchTransaction, TransactionExtension},
		transaction_validity::{InvalidTransaction, TransactionSource},
	},
};

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
		let ext = crate::SponsoredChargeTransactionPayment::<Test>::new(0, Some(1));

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
		let ext = crate::SponsoredChargeTransactionPayment::<Test>::new(0, Some(1));

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
fn sponsored_prepare_moves_budget_to_pending_and_post_dispatch_restores_refund() {
	new_test_ext().execute_with(|| {
		assert_ok!(SponsoredTx::register_sponsor(
			RuntimeOrigin::signed(1),
			200,
			policy(vec![2], 50)
		));
		let call = remark_call();
		let info = call.get_dispatch_info();
		let len = call.encoded_size();
		let ext = crate::SponsoredChargeTransactionPayment::<Test>::new(0, Some(1));

		let estimated_fee = polkadot_sdk::pallet_transaction_payment::Pallet::<Test>::compute_fee(
			len as u32, &info, 0,
		);
		let (pre, _) = ext
			.validate_and_prepare(RuntimeOrigin::signed(2), &call, &info, len, EXT_VERSION)
			.unwrap();

		assert_eq!(SponsoredTx::budget_on_hold(&1), 200 - estimated_fee);
		assert_eq!(SponsoredTx::pending_on_hold(&1), estimated_fee);

		let mut post_info =
			PostDispatchInfo { actual_weight: Some(info.call_weight / 2), pays_fee: Pays::Yes };
		let actual_fee =
			polkadot_sdk::pallet_transaction_payment::Pallet::<Test>::compute_actual_fee(
				len as u32, &info, &post_info, 0,
			);
		assert_ok!(crate::SponsoredChargeTransactionPayment::<Test>::post_dispatch(
			pre,
			&info,
			&mut post_info,
			len,
			&Ok(()),
		));

		assert_eq!(SponsoredTx::pending_on_hold(&1), 0);
		assert!(SponsoredTx::budget_on_hold(&1) < 200);
		System::assert_has_event(
			Event::SponsoredTransactionFeePaid { sponsor: 1, signer: 2, actual_fee, tip: 0 }.into(),
		);
	});
}

#[test]
fn unsponsored_path_keeps_signer_payment_behavior() {
	new_test_ext().execute_with(|| {
		let call = remark_call();
		let info = call.get_dispatch_info();
		let len = call.encoded_size();
		let signer_before = pallet_balances::Pallet::<Test>::free_balance(&2);
		let ext = crate::SponsoredChargeTransactionPayment::<Test>::new(0, None);

		let result =
			ext.dispatch_transaction(RuntimeOrigin::signed(2), call, &info, len, EXT_VERSION);
		assert!(result.is_ok());
		assert!(pallet_balances::Pallet::<Test>::free_balance(&2) < signer_before);
	});
}

#[test]
fn sponsored_validation_rejects_inactive_sponsor() {
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
		let ext = crate::SponsoredChargeTransactionPayment::<Test>::new(0, Some(1));

		let err = ext
			.validate_only(
				RuntimeOrigin::signed(2),
				&call,
				&info,
				len,
				TransactionSource::External,
				EXT_VERSION,
			)
			.unwrap_err();
		assert_eq!(err, InvalidTransaction::Custom(2).into());
	});
}

#[test]
fn sponsored_validation_rejects_fee_cap_exceeded() {
	new_test_ext().execute_with(|| {
		// Register with a very low max_fee_per_tx (1) so any real call exceeds the cap.
		assert_ok!(SponsoredTx::register_sponsor(
			RuntimeOrigin::signed(1),
			200,
			policy(vec![2], 1)
		));

		let call = remark_call();
		let info = call.get_dispatch_info();
		let len = call.encoded_size();
		let ext = crate::SponsoredChargeTransactionPayment::<Test>::new(0, Some(1));

		let fee = polkadot_sdk::pallet_transaction_payment::Pallet::<Test>::compute_fee(
			len as u32, &info, 0,
		);
		assert!(fee > 1, "fee {fee} must exceed the cap for this test to be meaningful");

		let err = ext
			.validate_only(
				RuntimeOrigin::signed(2),
				&call,
				&info,
				len,
				TransactionSource::External,
				EXT_VERSION,
			)
			.unwrap_err();
		assert_eq!(err, InvalidTransaction::Custom(4).into());
	});
}

#[test]
fn sponsored_validation_rejects_insufficient_budget() {
	new_test_ext().execute_with(|| {
		// Register with budget of 1 — far too small for any real fee.
		assert_ok!(SponsoredTx::register_sponsor(
			RuntimeOrigin::signed(1),
			1,
			policy(vec![2], 500)
		));

		let call = remark_call();
		let info = call.get_dispatch_info();
		let len = call.encoded_size();
		let ext = crate::SponsoredChargeTransactionPayment::<Test>::new(0, Some(1));

		let fee = polkadot_sdk::pallet_transaction_payment::Pallet::<Test>::compute_fee(
			len as u32, &info, 0,
		);
		assert!(fee > 1, "fee {fee} must exceed budget for this test to be meaningful");

		let err = ext
			.validate_only(
				RuntimeOrigin::signed(2),
				&call,
				&info,
				len,
				TransactionSource::External,
				EXT_VERSION,
			)
			.unwrap_err();
		assert_eq!(err, InvalidTransaction::Custom(5).into());
	});
}

#[test]
fn unregister_blocked_while_pending_hold_exists() {
	new_test_ext().execute_with(|| {
		assert_ok!(SponsoredTx::register_sponsor(
			RuntimeOrigin::signed(1),
			200,
			policy(vec![2], 50)
		));

		// Simulate an in-flight tx by moving budget to pending.
		let call = remark_call();
		let info = call.get_dispatch_info();
		let len = call.encoded_size();
		let ext = crate::SponsoredChargeTransactionPayment::<Test>::new(0, Some(1));

		let (_pre, _) = ext
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
fn sponsored_tx_with_tip_splits_correctly() {
	new_test_ext().execute_with(|| {
		assert_ok!(SponsoredTx::register_sponsor(
			RuntimeOrigin::signed(1),
			500,
			policy(vec![2], 200)
		));

		let call = remark_call();
		let info = call.get_dispatch_info();
		let len = call.encoded_size();
		let tip = 5u64;
		let ext = crate::SponsoredChargeTransactionPayment::<Test>::new(tip, Some(1));

		let (pre, _) = ext
			.validate_and_prepare(RuntimeOrigin::signed(2), &call, &info, len, EXT_VERSION)
			.unwrap();

		let mut post_info =
			PostDispatchInfo { actual_weight: Some(info.call_weight / 2), pays_fee: Pays::Yes };
		assert_ok!(crate::SponsoredChargeTransactionPayment::<Test>::post_dispatch(
			pre,
			&info,
			&mut post_info,
			len,
			&Ok(()),
		));

		assert_eq!(SponsoredTx::pending_on_hold(&1), 0);
		// The event should record the tip separately.
		let events: Vec<_> = System::events()
			.into_iter()
			.filter_map(|r| {
				if let RuntimeEvent::SponsoredTx(Event::SponsoredTransactionFeePaid {
					tip: event_tip,
					..
				}) = r.event
				{
					Some(event_tip)
				} else {
					None
				}
			})
			.collect();
		assert_eq!(events.len(), 1);
		assert_eq!(events[0], tip);
	});
}
