use super::*;
use crate::deposit::DepositError;
use crate::flash_loan::FlashLoanError;
use crate::withdraw::WithdrawError;
use soroban_sdk::{testutils::Address as _, Address, Env};

#[test]
fn test_emergency_state_machine_complete_flow() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(LendingContract, ());
    let client = LendingContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let guardian = Address::generate(&env);

    client.initialize(&admin, &1_000_000_000, &1000);
    assert_eq!(client.get_emergency_state(), EmergencyState::Normal);

    client.set_guardian(&admin, &guardian);
    assert_eq!(client.get_guardian(), Some(guardian.clone()));

    client.emergency_shutdown(&guardian);
    assert_eq!(client.get_emergency_state(), EmergencyState::Shutdown);

    client.start_recovery(&admin);
    assert_eq!(client.get_emergency_state(), EmergencyState::Recovery);

    client.complete_recovery(&admin);
    assert_eq!(client.get_emergency_state(), EmergencyState::Normal);
}

#[test]
fn test_emergency_shutdown_authorization() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(LendingContract, ());
    let client = LendingContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let guardian = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin, &1_000_000_000, &1000);
    client.set_guardian(&admin, &guardian);

    assert_eq!(
        client.try_emergency_shutdown(&user),
        Err(Ok(BorrowError::Unauthorized))
    );

    client.emergency_shutdown(&admin);
    assert_eq!(client.get_emergency_state(), EmergencyState::Shutdown);

    client.start_recovery(&admin);
    client.complete_recovery(&admin);

    client.emergency_shutdown(&guardian);
    assert_eq!(client.get_emergency_state(), EmergencyState::Shutdown);
}

#[test]
fn test_recovery_transition_authorization() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(LendingContract, ());
    let client = LendingContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let guardian = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin, &1_000_000_000, &1000);
    client.set_guardian(&admin, &guardian);

    assert_eq!(
        client.try_start_recovery(&admin),
        Err(Ok(BorrowError::ProtocolPaused))
    );

    client.emergency_shutdown(&guardian);

    assert_eq!(
        client.try_start_recovery(&user),
        Err(Ok(BorrowError::Unauthorized))
    );

    assert_eq!(
        client.try_start_recovery(&guardian),
        Err(Ok(BorrowError::Unauthorized))
    );

    client.start_recovery(&admin);
    assert_eq!(client.get_emergency_state(), EmergencyState::Recovery);
}

#[test]
fn test_complete_recovery_authorization() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(LendingContract, ());
    let client = LendingContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let guardian = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin, &1_000_000_000, &1000);
    client.set_guardian(&admin, &guardian);

    assert_eq!(
        client.try_complete_recovery(&admin),
        Err(Ok(BorrowError::ProtocolPaused))
    );

    client.emergency_shutdown(&guardian);

    assert_eq!(
        client.try_complete_recovery(&admin),
        Err(Ok(BorrowError::ProtocolPaused))
    );

    client.start_recovery(&admin);

    assert_eq!(
        client.try_complete_recovery(&user),
        Err(Ok(BorrowError::Unauthorized))
    );

    assert_eq!(
        client.try_complete_recovery(&guardian),
        Err(Ok(BorrowError::Unauthorized))
    );

    client.complete_recovery(&admin);
    assert_eq!(client.get_emergency_state(), EmergencyState::Normal);
}

#[test]
fn test_operation_permissions_normal_state() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(LendingContract, ());
    let client = LendingContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    let asset = Address::generate(&env);
    let collateral_asset = Address::generate(&env);

    client.initialize(&admin, &1_000_000_000, &1000);
    client.initialize_deposit_settings(&1_000_000_000, &100);
    client.initialize_withdraw_settings(&100);

    assert_eq!(client.get_emergency_state(), EmergencyState::Normal);

    client.deposit(&user, &asset, &50_000);
    client.deposit_collateral(&user, &collateral_asset, &20_000);
    client.borrow(&user, &asset, &10_000, &collateral_asset, &20_000);
    client.repay(&user, &asset, &1_000);
    client.withdraw(&user, &asset, &1_000);
    client.flash_loan(&user, &asset, &1_000, &soroban_sdk::Bytes::new(&env));
}

#[test]
fn test_operation_permissions_shutdown_state() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(LendingContract, ());
    let client = LendingContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let guardian = Address::generate(&env);
    let user = Address::generate(&env);
    let asset = Address::generate(&env);
    let collateral_asset = Address::generate(&env);

    client.initialize(&admin, &1_000_000_000, &1000);
    client.set_guardian(&admin, &guardian);
    client.initialize_deposit_settings(&1_000_000_000, &100);
    client.initialize_withdraw_settings(&100);

    client.deposit(&user, &asset, &50_000);
    client.borrow(&user, &asset, &10_000, &collateral_asset, &20_000);

    client.emergency_shutdown(&guardian);
    assert_eq!(client.get_emergency_state(), EmergencyState::Shutdown);

    assert_eq!(
        client.try_deposit(&user, &asset, &1000),
        Err(Ok(DepositError::DepositPaused))
    );
    assert_eq!(
        client.try_deposit_collateral(&user, &collateral_asset, &1000),
        Err(Ok(BorrowError::ProtocolPaused))
    );
    assert_eq!(
        client.try_borrow(&user, &asset, &1000, &collateral_asset, &2000),
        Err(Ok(BorrowError::ProtocolPaused))
    );
    assert_eq!(
        client.try_liquidate(&user, &user, &asset, &collateral_asset, &1000),
        Err(Ok(BorrowError::ProtocolPaused))
    );
    assert_eq!(
        client.try_flash_loan(&user, &asset, &1000, &soroban_sdk::Bytes::new(&env)),
        Err(Ok(FlashLoanError::ProtocolPaused))
    );
    assert_eq!(
        client.try_repay(&user, &asset, &1000),
        Err(Ok(BorrowError::ProtocolPaused))
    );
    assert_eq!(
        client.try_withdraw(&user, &asset, &1000),
        Err(Ok(WithdrawError::WithdrawPaused))
    );
}

#[test]
fn test_operation_permissions_recovery_state() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(LendingContract, ());
    let client = LendingContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let guardian = Address::generate(&env);
    let user = Address::generate(&env);
    let asset = Address::generate(&env);
    let collateral_asset = Address::generate(&env);

    client.initialize(&admin, &1_000_000_000, &1000);
    client.set_guardian(&admin, &guardian);
    client.initialize_deposit_settings(&1_000_000_000, &100);
    client.initialize_withdraw_settings(&100);

    client.deposit(&user, &asset, &50_000);
    client.borrow(&user, &asset, &10_000, &collateral_asset, &20_000);

    client.emergency_shutdown(&guardian);
    client.start_recovery(&admin);
    assert_eq!(client.get_emergency_state(), EmergencyState::Recovery);

    assert_eq!(
        client.try_deposit(&user, &asset, &1000),
        Err(Ok(DepositError::DepositPaused))
    );
    assert_eq!(
        client.try_deposit_collateral(&user, &collateral_asset, &1000),
        Err(Ok(BorrowError::ProtocolPaused))
    );
    assert_eq!(
        client.try_borrow(&user, &asset, &1000, &collateral_asset, &2000),
        Err(Ok(BorrowError::ProtocolPaused))
    );
    assert_eq!(
        client.try_liquidate(&user, &user, &asset, &collateral_asset, &1000),
        Err(Ok(BorrowError::ProtocolPaused))
    );
    assert_eq!(
        client.try_flash_loan(&user, &asset, &1000, &soroban_sdk::Bytes::new(&env)),
        Err(Ok(FlashLoanError::ProtocolPaused))
    );

    client.repay(&user, &asset, &1_000);
    client.withdraw(&user, &asset, &1_000);
}

#[test]
fn test_forbidden_state_transitions() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(LendingContract, ());
    let client = LendingContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let guardian = Address::generate(&env);

    client.initialize(&admin, &1_000_000_000, &1000);
    client.set_guardian(&admin, &guardian);

    assert_eq!(
        client.try_start_recovery(&admin),
        Err(Ok(BorrowError::ProtocolPaused))
    );

    assert_eq!(
        client.try_complete_recovery(&admin),
        Err(Ok(BorrowError::ProtocolPaused))
    );

    client.emergency_shutdown(&guardian);

    assert_eq!(
        client.try_complete_recovery(&admin),
        Err(Ok(BorrowError::ProtocolPaused))
    );

    client.start_recovery(&admin);
    client.emergency_shutdown(&admin);
    assert_eq!(client.get_emergency_state(), EmergencyState::Shutdown);
}

#[test]
fn test_guardian_configuration_authorization() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(LendingContract, ());
    let client = LendingContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let guardian = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin, &1_000_000_000, &1000);

    assert_eq!(
        client.try_set_guardian(&user, &guardian),
        Err(Ok(BorrowError::Unauthorized))
    );

    client.set_guardian(&admin, &guardian);
    assert_eq!(client.get_guardian(), Some(guardian.clone()));

    let new_guardian = Address::generate(&env);
    client.set_guardian(&admin, &new_guardian);
    assert_eq!(client.get_guardian(), Some(new_guardian));
}

#[test]
fn test_multiple_emergency_cycles() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(LendingContract, ());
    let client = LendingContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let guardian = Address::generate(&env);
    let user = Address::generate(&env);
    let asset = Address::generate(&env);
    let collateral_asset = Address::generate(&env);

    client.initialize(&admin, &1_000_000_000, &1000);
    client.set_guardian(&admin, &guardian);
    client.initialize_deposit_settings(&1_000_000_000, &100);
    client.initialize_withdraw_settings(&100);

    client.deposit(&user, &asset, &50_000);
    client.borrow(&user, &asset, &10_000, &collateral_asset, &20_000);

    client.emergency_shutdown(&guardian);
    client.start_recovery(&admin);
    client.repay(&user, &asset, &5_000);
    client.withdraw(&user, &asset, &5_000);
    client.complete_recovery(&admin);

    client.deposit(&user, &asset, &30_000);
    client.borrow(&user, &asset, &5_000, &collateral_asset, &10_000);

    client.emergency_shutdown(&admin);
    client.start_recovery(&admin);
    client.repay(&user, &asset, &2_000);
    client.withdraw(&user, &asset, &2_000);
    client.complete_recovery(&admin);

    assert_eq!(client.get_emergency_state(), EmergencyState::Normal);
}
