use payments_system::csv::{TransactionRecord, TransactionType};
use payments_system::generate_accounts_report;
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
fn generate_report_single_deposit() {
    let (_, payment_db) = apply_records(vec![record(
        TransactionType::Deposit,
        1,
        1,
        Some(MoneyAmount(dec!(100.0))),
    )]);

    let report = generate_accounts_report(payment_db);
    let status = report.get(&ClientId(1)).unwrap();

    assert_eq!(status.available, MoneyAmount(dec!(100.0)));
    assert_eq!(status.total, MoneyAmount(dec!(100.0)));
    assert_eq!(status.held, MoneyAmount(dec!(0.0)));
    assert!(!status.locked);
}

#[test]
fn generate_report_single_withdrawal() {
    let (_, payment_db) = apply_records(vec![record(
        TransactionType::Withdrawal,
        1,
        1,
        Some(MoneyAmount(dec!(50.0))),
    )]);

    let report = generate_accounts_report(payment_db);
    let status = report.get(&ClientId(1)).unwrap();

    assert_eq!(status.available, MoneyAmount(dec!(-50.0)));
    assert_eq!(status.total, MoneyAmount(dec!(-50.0)));
    assert_eq!(status.held, MoneyAmount(dec!(0.0)));
    assert!(!status.locked);
}

#[test]
fn generate_report_deposit_and_withdrawal() {
    let (_, payment_db) = apply_records(vec![
        record(
            TransactionType::Deposit,
            1,
            1,
            Some(MoneyAmount(dec!(100.0))),
        ),
        record(
            TransactionType::Withdrawal,
            1,
            2,
            Some(MoneyAmount(dec!(30.0))),
        ),
    ]);

    let report = generate_accounts_report(payment_db);
    let status = report.get(&ClientId(1)).unwrap();

    assert_eq!(status.available, MoneyAmount(dec!(70.0)));
    assert_eq!(status.total, MoneyAmount(dec!(70.0)));
    assert_eq!(status.held, MoneyAmount(dec!(0.0)));
    assert!(!status.locked);
}

#[test]
fn generate_report_deposit_dispute_and_resolve() {
    let (_, payment_db) = apply_records(vec![
        record(
            TransactionType::Deposit,
            1,
            1,
            Some(MoneyAmount(dec!(100.0))),
        ),
        record(
            TransactionType::Deposit,
            1,
            2,
            Some(MoneyAmount(dec!(50.0))),
        ),
        record(TransactionType::Dispute, 1, 2, None),
        record(
            TransactionType::Deposit,
            1,
            3,
            Some(MoneyAmount(dec!(25.0))),
        ),
        record(TransactionType::Dispute, 1, 3, None),
        record(TransactionType::Resolve, 1, 3, None),
    ]);

    let report = generate_accounts_report(payment_db);
    let status = report.get(&ClientId(1)).unwrap();

    assert_eq!(status.available, MoneyAmount(dec!(125.0)));
    assert_eq!(status.held, MoneyAmount(dec!(50.0)));
    assert_eq!(status.total, MoneyAmount(dec!(175.0)));
    assert!(!status.locked);
}

#[test]
fn generate_report_deposit_then_chargeback() {
    let (_, payment_db) = apply_records(vec![
        record(
            TransactionType::Deposit,
            1,
            1,
            Some(MoneyAmount(dec!(100.0))),
        ),
        record(
            TransactionType::Deposit,
            1,
            2,
            Some(MoneyAmount(dec!(50.0))),
        ),
        record(TransactionType::Dispute, 1, 2, None),
        record(TransactionType::Chargeback, 1, 2, None),
    ]);

    let report = generate_accounts_report(payment_db);
    let status = report.get(&ClientId(1)).unwrap();

    assert_eq!(status.available, MoneyAmount(dec!(50.0)));
    assert_eq!(status.held, MoneyAmount(dec!(0.0)));
    assert_eq!(status.total, MoneyAmount(dec!(50.0)));
    assert!(status.locked);
}

#[test]
fn generate_report_complex_scenario() {
    let (_, payment_db) = apply_records(vec![
        record(
            TransactionType::Deposit,
            1,
            1,
            Some(MoneyAmount(dec!(100.0))),
        ),
        record(
            TransactionType::Withdrawal,
            1,
            2,
            Some(MoneyAmount(dec!(30.0))),
        ),
        record(
            TransactionType::Deposit,
            1,
            3,
            Some(MoneyAmount(dec!(40.0))),
        ),
        record(TransactionType::Dispute, 1, 3, None),
        record(TransactionType::Resolve, 1, 3, None),
        record(
            TransactionType::Deposit,
            2,
            4,
            Some(MoneyAmount(dec!(250.0))),
        ),
        record(
            TransactionType::Deposit,
            2,
            5,
            Some(MoneyAmount(dec!(50.0))),
        ),
        record(TransactionType::Dispute, 2, 5, None),
        record(TransactionType::Chargeback, 2, 5, None),
    ]);

    let report = generate_accounts_report(payment_db);

    let status1 = report.get(&ClientId(1)).unwrap();
    assert_eq!(status1.available, MoneyAmount(dec!(110.0)));
    assert_eq!(status1.held, MoneyAmount(dec!(0.0)));
    assert_eq!(status1.total, MoneyAmount(dec!(110.0)));
    assert!(!status1.locked);

    let status2 = report.get(&ClientId(2)).unwrap();
    assert_eq!(status2.available, MoneyAmount(dec!(200.0)));
    assert_eq!(status2.held, MoneyAmount(dec!(0.0)));
    assert_eq!(status2.total, MoneyAmount(dec!(200.0)));
    assert!(status2.locked);
}

#[test]
fn complex_workflow_with_withdrawal_deposit_dispute_chargeback() {
    let (_, payment_db) = apply_records(vec![
        record(
            TransactionType::Deposit,
            1,
            1,
            Some(MoneyAmount(dec!(500.0))),
        ),
        record(
            TransactionType::Withdrawal,
            1,
            2,
            Some(MoneyAmount(dec!(200.0))),
        ),
        record(
            TransactionType::Deposit,
            1,
            3,
            Some(MoneyAmount(dec!(300.0))),
        ),
        record(TransactionType::Dispute, 1, 3, None),
        record(
            TransactionType::Deposit,
            1,
            4,
            Some(MoneyAmount(dec!(150.0))),
        ),
        record(TransactionType::Dispute, 1, 4, None),
        record(TransactionType::Chargeback, 1, 4, None),
    ]);

    let report = generate_accounts_report(payment_db);
    let status = report.get(&ClientId(1)).unwrap();

    assert_eq!(status.available, MoneyAmount(dec!(150.0)));
    assert_eq!(status.held, MoneyAmount(dec!(300.0)));
    assert_eq!(status.total, MoneyAmount(dec!(450.0)));
    assert!(status.locked);
    assert_eq!(status.available + status.held, status.total);
}
