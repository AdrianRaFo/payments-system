use payments_system::csv::{TransactionRecord, TransactionType};
use payments_system::models::{ClientId, ClientPayment, MoneyAmount, TransactionId};
use payments_system::process_csv_record;
use rust_decimal::dec;
use std::collections::{HashMap, HashSet};

fn record(
    kind: TransactionType,
    client_id: u16,
    tx_id: u32,
    amount: Option<MoneyAmount>,
) -> TransactionRecord {
    TransactionRecord {
        transaction_type: kind,
        client_id,
        tx_id,
        amount,
    }
}

fn apply_records(
    records: Vec<TransactionRecord>,
) -> (
    HashSet<ClientId>,
    HashMap<(ClientId, TransactionId), ClientPayment>,
) {
    let mut locked_clients: HashSet<ClientId> = HashSet::new();
    let mut payment_db: HashMap<(ClientId, TransactionId), ClientPayment> = HashMap::new();

    for tx in records {
        process_csv_record(&mut locked_clients, &mut payment_db, tx).unwrap();
    }

    (locked_clients, payment_db)
}

#[test]
fn deposit_and_withdrawal_are_stored_as_done() {
    let (locked_clients, payment_db) = apply_records(vec![
        record(
            TransactionType::Deposit,
            1,
            1,
            Some(MoneyAmount(dec!(10.0))),
        ),
        record(
            TransactionType::Withdrawal,
            2,
            2,
            Some(MoneyAmount(dec!(11.0))),
        ),
    ]);

    assert!(locked_clients.is_empty());
    assert!(matches!(
        payment_db.get(&(ClientId(1), TransactionId(1))),
        Some(ClientPayment::Done(_))
    ));
    assert!(matches!(
        payment_db.get(&(ClientId(2), TransactionId(2))),
        Some(ClientPayment::Done(_))
    ));
}

#[test]
fn dispute_then_resolve_transitions_payment_state() {
    let (locked_clients, payment_db) = apply_records(vec![
        record(
            TransactionType::Deposit,
            1,
            1,
            Some(MoneyAmount(dec!(10.0))),
        ),
        record(TransactionType::Dispute, 1, 1, None),
        record(TransactionType::Resolve, 1, 1, None),
    ]);

    assert!(locked_clients.is_empty());
    assert!(matches!(
        payment_db.get(&(ClientId(1), TransactionId(1))),
        Some(ClientPayment::Resolved(_))
    ));
}

#[test]
fn chargeback_locks_client_and_ignores_future_transactions() {
    let (locked_clients, payment_db) = apply_records(vec![
        record(
            TransactionType::Deposit,
            2,
            7,
            Some(MoneyAmount(dec!(20.0))),
        ),
        record(TransactionType::Dispute, 2, 7, None),
        record(TransactionType::Chargeback, 2, 7, None),
        record(TransactionType::Deposit, 2, 8, None),
    ]);

    assert!(locked_clients.contains(&ClientId(2)));
    assert!(matches!(
        payment_db.get(&(ClientId(2), TransactionId(7))),
        Some(ClientPayment::ChargedBack(_))
    ));
    assert!(!payment_db.contains_key(&(ClientId(2), TransactionId(8))));
}

#[test]
fn chargeback_does_not_lock_client_when_payment_state_does_not_change() {
    let (locked_clients, payment_db) = apply_records(vec![
        record(
            TransactionType::Deposit,
            3,
            9,
            Some(MoneyAmount(dec!(30.0))),
        ),
        record(TransactionType::Chargeback, 3, 9, None),
    ]);

    assert!(locked_clients.is_empty());
    assert!(matches!(
        payment_db.get(&(ClientId(3), TransactionId(9))),
        Some(ClientPayment::Done(_))
    ));
}
