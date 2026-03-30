use payments_system::generate_accounts_report;
use payments_system::models::{
    ChargedBack, ClientId, ClientPayment, Done, MoneyAmount, OnDispute, Payment, PaymentState,
    PaymentType, Resolved, TransactionId,
};
use rust_decimal::dec;
use std::collections::HashMap;

fn payment<S: PaymentState>(
    client_id: ClientId,
    tx_id: TransactionId,
    payment_type: PaymentType,
    amount: MoneyAmount,
    state: S,
) -> Payment<S> {
    Payment {
        client_id,
        tx_id,
        payment_type,
        amount,
        _state: state,
    }
}

#[test]
fn generate_report_single_deposit() {
    let mut payment_db: HashMap<(ClientId, TransactionId), ClientPayment> = HashMap::new();
    payment_db.insert(
        (ClientId(1), TransactionId(1)),
        ClientPayment::Done(payment(
            ClientId(1),
            TransactionId(1),
            PaymentType::Deposit,
            MoneyAmount(dec!(100.0)),
            Done,
        )),
    );

    let report = generate_accounts_report(payment_db);
    let status = report.get(&ClientId(1)).unwrap();

    assert_eq!(status.available, MoneyAmount(dec!(100.0)));
    assert_eq!(status.total, MoneyAmount(dec!(100.0)));
    assert_eq!(status.held, MoneyAmount(dec!(0.0)));
    assert!(!status.locked);
}

#[test]
fn generate_report_single_withdrawal() {
    let mut payment_db: HashMap<(ClientId, TransactionId), ClientPayment> = HashMap::new();
    payment_db.insert(
        (ClientId(1), TransactionId(1)),
        ClientPayment::Done(payment(
            ClientId(1),
            TransactionId(1),
            PaymentType::Withdrawal,
            MoneyAmount(dec!(50.0)),
            Done,
        )),
    );

    let report = generate_accounts_report(payment_db);
    let status = report.get(&ClientId(1)).unwrap();

    assert_eq!(status.available, MoneyAmount(dec!(-50.0)));
    assert_eq!(status.total, MoneyAmount(dec!(-50.0)));
    assert_eq!(status.held, MoneyAmount(dec!(0.0)));
    assert!(!status.locked);
}

#[test]
fn generate_report_deposit_and_withdrawal() {
    let mut payment_db: HashMap<(ClientId, TransactionId), ClientPayment> = HashMap::new();
    payment_db.insert(
        (ClientId(1), TransactionId(1)),
        ClientPayment::Done(payment(
            ClientId(1),
            TransactionId(1),
            PaymentType::Deposit,
            MoneyAmount(dec!(100.0)),
            Done,
        )),
    );
    payment_db.insert(
        (ClientId(1), TransactionId(2)),
        ClientPayment::Done(payment(
            ClientId(1),
            TransactionId(2),
            PaymentType::Withdrawal,
            MoneyAmount(dec!(30.0)),
            Done,
        )),
    );

    let report = generate_accounts_report(payment_db);
    let status = report.get(&ClientId(1)).unwrap();

    assert_eq!(status.available, MoneyAmount(dec!(70.0)));
    assert_eq!(status.total, MoneyAmount(dec!(70.0)));
    assert_eq!(status.held, MoneyAmount(dec!(0.0)));
    assert!(!status.locked);
}

#[test]
fn generate_report_deposit_dispute_and_resolve() {
    let mut payment_db: HashMap<(ClientId, TransactionId), ClientPayment> = HashMap::new();
    payment_db.insert(
        (ClientId(1), TransactionId(1)),
        ClientPayment::Done(payment(
            ClientId(1),
            TransactionId(1),
            PaymentType::Deposit,
            MoneyAmount(dec!(100.0)),
            Done,
        )),
    );
    payment_db.insert(
        (ClientId(1), TransactionId(2)),
        ClientPayment::OnDispute(payment(
            ClientId(1),
            TransactionId(2),
            PaymentType::Deposit,
            MoneyAmount(dec!(50.0)),
            OnDispute,
        )),
    );
    payment_db.insert(
        (ClientId(1), TransactionId(3)),
        ClientPayment::Resolved(payment(
            ClientId(1),
            TransactionId(3),
            PaymentType::Deposit,
            MoneyAmount(dec!(25.0)),
            Resolved,
        )),
    );

    let report = generate_accounts_report(payment_db);
    let status = report.get(&ClientId(1)).unwrap();

    assert_eq!(status.available, MoneyAmount(dec!(125.0)));
    assert_eq!(status.held, MoneyAmount(dec!(50.0)));
    assert_eq!(status.total, MoneyAmount(dec!(175.0)));
    assert!(!status.locked);
}

#[test]
fn generate_report_deposit_then_chargeback() {
    let mut payment_db: HashMap<(ClientId, TransactionId), ClientPayment> = HashMap::new();
    payment_db.insert(
        (ClientId(1), TransactionId(1)),
        ClientPayment::Done(payment(
            ClientId(1),
            TransactionId(1),
            PaymentType::Deposit,
            MoneyAmount(dec!(100.0)),
            Done,
        )),
    );
    payment_db.insert(
        (ClientId(1), TransactionId(2)),
        ClientPayment::ChargedBack(payment(
            ClientId(1),
            TransactionId(2),
            PaymentType::Deposit,
            MoneyAmount(dec!(50.0)),
            ChargedBack,
        )),
    );

    let report = generate_accounts_report(payment_db);
    let status = report.get(&ClientId(1)).unwrap();

    assert_eq!(status.available, MoneyAmount(dec!(100.0)));
    assert_eq!(status.held, MoneyAmount(dec!(0.0)));
    assert_eq!(status.total, MoneyAmount(dec!(100.0)));
    assert!(status.locked);
}

#[test]
fn generate_report_complex_scenario() {
    let mut payment_db: HashMap<(ClientId, TransactionId), ClientPayment> = HashMap::new();

    payment_db.insert(
        (ClientId(1), TransactionId(1)),
        ClientPayment::Done(payment(
            ClientId(1),
            TransactionId(1),
            PaymentType::Deposit,
            MoneyAmount(dec!(100.0)),
            Done,
        )),
    );
    payment_db.insert(
        (ClientId(1), TransactionId(2)),
        ClientPayment::Done(payment(
            ClientId(1),
            TransactionId(2),
            PaymentType::Withdrawal,
            MoneyAmount(dec!(30.0)),
            Done,
        )),
    );
    payment_db.insert(
        (ClientId(1), TransactionId(3)),
        ClientPayment::Resolved(payment(
            ClientId(1),
            TransactionId(3),
            PaymentType::Deposit,
            MoneyAmount(dec!(40.0)),
            Resolved,
        )),
    );

    payment_db.insert(
        (ClientId(2), TransactionId(4)),
        ClientPayment::Done(payment(
            ClientId(2),
            TransactionId(4),
            PaymentType::Deposit,
            MoneyAmount(dec!(250.0)),
            Done,
        )),
    );
    payment_db.insert(
        (ClientId(2), TransactionId(5)),
        ClientPayment::ChargedBack(payment(
            ClientId(2),
            TransactionId(5),
            PaymentType::Deposit,
            MoneyAmount(dec!(50.0)),
            ChargedBack,
        )),
    );

    let report = generate_accounts_report(payment_db);

    let status1 = report.get(&ClientId(1)).unwrap();
    assert_eq!(status1.available, MoneyAmount(dec!(110.0)));
    assert_eq!(status1.held, MoneyAmount(dec!(0.0)));
    assert_eq!(status1.total, MoneyAmount(dec!(110.0)));
    assert!(!status1.locked);

    let status2 = report.get(&ClientId(2)).unwrap();
    assert_eq!(status2.available, MoneyAmount(dec!(250.0)));
    assert_eq!(status2.held, MoneyAmount(dec!(0.0)));
    assert_eq!(status2.total, MoneyAmount(dec!(250.0)));
    assert!(status2.locked);
}

#[test]
fn complex_workflow_with_withdrawal_deposit_dispute_chargeback() {
    let mut payment_db: HashMap<(ClientId, TransactionId), ClientPayment> = HashMap::new();

    payment_db.insert(
        (ClientId(1), TransactionId(1)),
        ClientPayment::Done(payment(
            ClientId(1),
            TransactionId(1),
            PaymentType::Deposit,
            MoneyAmount(dec!(500.0)),
            Done,
        )),
    );
    payment_db.insert(
        (ClientId(1), TransactionId(2)),
        ClientPayment::Done(payment(
            ClientId(1),
            TransactionId(2),
            PaymentType::Withdrawal,
            MoneyAmount(dec!(200.0)),
            Done,
        )),
    );
    payment_db.insert(
        (ClientId(1), TransactionId(3)),
        ClientPayment::OnDispute(payment(
            ClientId(1),
            TransactionId(3),
            PaymentType::Deposit,
            MoneyAmount(dec!(300.0)),
            OnDispute,
        )),
    );
    payment_db.insert(
        (ClientId(1), TransactionId(4)),
        ClientPayment::ChargedBack(payment(
            ClientId(1),
            TransactionId(4),
            PaymentType::Deposit,
            MoneyAmount(dec!(150.0)),
            ChargedBack,
        )),
    );

    let report = generate_accounts_report(payment_db);
    let status = report.get(&ClientId(1)).unwrap();

    assert_eq!(status.available, MoneyAmount(dec!(300.0)));
    assert_eq!(status.held, MoneyAmount(dec!(300.0)));
    assert_eq!(status.total, MoneyAmount(dec!(600.0)));
    assert!(status.locked);
    assert_eq!(status.available + status.held, status.total);
}
