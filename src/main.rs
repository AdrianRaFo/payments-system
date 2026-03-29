use crate::csv::{
    ClientStatus, TransactionRecord, TransactionType, create_csv_reader, save_accounts_csv,
    save_payments_csv,
};
use crate::models::{ClientId, ClientPayment, Done, Payment, PaymentType, TransactionId};
use clap::Parser;
use std::collections::{HashMap, HashSet};

pub mod csv;
pub mod models;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Path to CSV file
    path: String,
}

fn main() {
    let args = Args::parse();

    process(args.path).expect("Error processing transactions");
}

fn process(path: String) -> Result<(), Box<dyn std::error::Error>> {
    let mut csv_reader = create_csv_reader(path)?;

    let mut locked_clients: HashSet<ClientId> = HashSet::new();

    let mut payment_db: HashMap<(ClientId, TransactionId), ClientPayment> = HashMap::new();

    for (idx, record) in csv_reader.deserialize::<TransactionRecord>().enumerate() {
        match record {
            Ok(transaction_record) => {
                process_csv_record(&mut locked_clients, &mut payment_db, transaction_record)?
            }
            Err(e) => return Err(format!("Error deserializing record {idx}: {e}").into()),
        }
    }

    save_payments_csv(&payment_db)?;

    let accounts = generate_accounts_report(payment_db);

    save_accounts_csv(accounts)
}

fn process_csv_record(
    locked_clients: &mut HashSet<ClientId>,
    payment_db: &mut HashMap<(ClientId, TransactionId), ClientPayment>,
    tx: TransactionRecord,
) -> Result<(), Box<dyn std::error::Error>> {
    if locked_clients.get(&ClientId(tx.client_id)).is_some() {
        return Ok(());
    }
    let key = (ClientId(tx.client_id), TransactionId(tx.tx_id));

    match tx.transaction_type {
        // if transaction_record is Deposit or Withdrawal, we store it in the db
        TransactionType::Deposit | TransactionType::Withdrawal => {
            let payment: Payment<Done> = tx.try_into()?;
            let client_payment = ClientPayment::Done(payment);
            payment_db.entry(key).or_insert_with(|| client_payment);
        }
        // if transaction_record is Dispute, find the done payment in the db and update it (if possible)
        TransactionType::Dispute => {
            if !payment_db.contains_key(&key) {
                eprintln!("Warning: No payment found. Transaction {tx:?} was skipped");
                return Ok(());
            }

            payment_db.entry(key).and_modify(|client_payment| match client_payment {
            ClientPayment::Done(payment) => {
              // if payment is not a deposit, it can't be disputed
              match payment.disputed().map(ClientPayment::OnDispute) {
                Ok(disputed) => *client_payment = disputed,
                Err(e) => eprintln!("{e}"),
              }
            }
            _ => {
              eprintln!("Warning: Dispute can't be opened on a {:?} payment. Transaction {key:?} was skipped", client_payment.state());
            }
          });
        }
        // if transaction_record is Resolve or Chargeback, find the disputed payment in the db and update it (if possible)
        TransactionType::Resolve | TransactionType::Chargeback => {
            if !payment_db.contains_key(&key) {
                eprintln!("Warning: No payment found. Transaction {tx:?} was skipped");
                return Ok(());
            }
            payment_db.entry(key).and_modify(|client_payment| match client_payment {
              ClientPayment::OnDispute(payment) => {
                if tx.transaction_type == TransactionType::Resolve {
                  *client_payment = ClientPayment::Resolved(payment.resolved())
                } else {
                  // if ChargeBack happened lock account and ignore other transactions
                  *client_payment = ClientPayment::ChargedBack(payment.charge_back());
                  locked_clients.insert(ClientId(tx.client_id));
                }
              }
              _ => {
                eprintln!("Warning: {:?} can't be done on a {:?} payment. Transaction {tx:?} was skipped", tx.transaction_type, client_payment.state());
              }
            });
        }
    }

    Ok(())
}

fn generate_accounts_report(
    payment_db: HashMap<(ClientId, TransactionId), ClientPayment>,
) -> HashMap<ClientId, ClientStatus> {
    let mut report: HashMap<ClientId, ClientStatus> = HashMap::new();
    for ((client_id, _), payment) in payment_db {
        // for each payment, we calculate the available, held and total amounts for the client
        let client_status = report
            .entry(client_id)
            .or_insert(ClientStatus::new(client_id));

        match payment {
            ClientPayment::Done(payment) => {
                if payment.payment_type == PaymentType::Deposit {
                    client_status.available += payment.amount;
                    client_status.total += payment.amount;
                } else {
                    // withdrawal
                    client_status.available -= payment.amount;
                    client_status.total -= payment.amount;
                }
            }
            ClientPayment::OnDispute(payment) => {
                // the deposit payment is still on dispute, amount is held
                client_status.held += payment.amount;
                client_status.total += payment.amount;
            }
            ClientPayment::Resolved(payment) => {
                // the client kept the deposit
                client_status.available += payment.amount;
                client_status.total += payment.amount;
            }
            ClientPayment::ChargedBack(payment) => {
                // the client lost the deposit and the account is locked
                client_status.available -= payment.amount;
                client_status.total -= payment.amount;
                client_status.locked = true;
            }
        }
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{ChargedBack, MoneyAmount, OnDispute, PaymentType, Resolved};
    use rust_decimal::dec;

    #[test]
    fn deposit_and_withdrawal_are_stored_as_done() {
        let mut locked_clients: HashSet<ClientId> = HashSet::new();
        let mut payment_db: HashMap<(ClientId, TransactionId), ClientPayment> = HashMap::new();

        let tx = TransactionRecord {
            transaction_type: TransactionType::Deposit,
            client_id: 1,
            tx_id: 1,
            amount: Some(MoneyAmount(dec!(10.0))),
        };
        process_csv_record(&mut locked_clients, &mut payment_db, tx).unwrap();

        let tx = TransactionRecord {
            transaction_type: TransactionType::Withdrawal,
            client_id: 2,
            tx_id: 2,
            amount: Some(MoneyAmount(dec!(11.0))),
        };
        process_csv_record(&mut locked_clients, &mut payment_db, tx).unwrap();

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
        let mut locked_clients: HashSet<ClientId> = HashSet::new();
        let mut payment_db: HashMap<(ClientId, TransactionId), ClientPayment> = HashMap::new();
        let key = (ClientId(1), TransactionId(1));

        payment_db.insert(
            key,
            ClientPayment::Done(Payment {
                client_id: ClientId(1),
                tx_id: TransactionId(1),
                payment_type: PaymentType::Deposit,
                amount: MoneyAmount(dec!(10.0)),
                _state: Done,
            }),
        );

        process_csv_record(
            &mut locked_clients,
            &mut payment_db,
            TransactionRecord {
                transaction_type: TransactionType::Dispute,
                client_id: 1,
                tx_id: 1,
                amount: None,
            },
        )
        .unwrap();

        assert!(locked_clients.is_empty());
        assert!(matches!(
            payment_db.get(&key),
            Some(ClientPayment::OnDispute(_))
        ));

        process_csv_record(
            &mut locked_clients,
            &mut payment_db,
            TransactionRecord {
                transaction_type: TransactionType::Resolve,
                client_id: 1,
                tx_id: 1,
                amount: None,
            },
        )
        .unwrap();

        assert!(locked_clients.is_empty());
        assert!(matches!(
            payment_db.get(&key),
            Some(ClientPayment::Resolved(_))
        ));
    }

    #[test]
    fn chargeback_locks_client_and_ignores_future_transactions() {
        let mut locked_clients: HashSet<ClientId> = HashSet::new();
        let mut payment_db: HashMap<(ClientId, TransactionId), ClientPayment> = HashMap::new();
        let key = (ClientId(2), TransactionId(7));

        payment_db.insert(
            key,
            ClientPayment::OnDispute(Payment {
                client_id: ClientId(2),
                tx_id: TransactionId(7),
                payment_type: PaymentType::Deposit,
                amount: MoneyAmount(dec!(20.0)),
                _state: OnDispute,
            }),
        );

        process_csv_record(
            &mut locked_clients,
            &mut payment_db,
            TransactionRecord {
                transaction_type: TransactionType::Chargeback,
                client_id: 2,
                tx_id: 7,
                amount: None,
            },
        )
        .unwrap();

        assert!(locked_clients.contains(&ClientId(2)));
        assert!(matches!(
            payment_db.get(&key),
            Some(ClientPayment::ChargedBack(_))
        ));

        process_csv_record(
            &mut locked_clients,
            &mut payment_db,
            TransactionRecord {
                transaction_type: TransactionType::Deposit,
                client_id: 2,
                tx_id: 8,
                amount: None,
            },
        )
        .unwrap();

        assert!(locked_clients.contains(&ClientId(2)));
        assert!(payment_db.get(&(ClientId(2), TransactionId(8))).is_none());
    }

    #[test]
    fn chargeback_does_not_lock_client_when_payment_state_does_not_change() {
        let mut locked_clients: HashSet<ClientId> = HashSet::new();
        let mut payment_db: HashMap<(ClientId, TransactionId), ClientPayment> = HashMap::new();
        let key = (ClientId(3), TransactionId(9));

        payment_db.insert(
            key,
            ClientPayment::Done(Payment {
                client_id: ClientId(3),
                tx_id: TransactionId(9),
                payment_type: PaymentType::Deposit,
                amount: MoneyAmount(dec!(30.0)),
                _state: Done,
            }),
        );

        process_csv_record(
            &mut locked_clients,
            &mut payment_db,
            TransactionRecord {
                transaction_type: TransactionType::Chargeback,
                client_id: 3,
                tx_id: 9,
                amount: None,
            },
        )
        .unwrap();

        assert!(locked_clients.is_empty());
        assert!(matches!(payment_db.get(&key), Some(ClientPayment::Done(_))));
    }

    #[test]
    fn generate_report_single_deposit() {
        let mut payment_db: HashMap<(ClientId, TransactionId), ClientPayment> = HashMap::new();
        payment_db.insert(
            (ClientId(1), TransactionId(1)),
            ClientPayment::Done(Payment {
                client_id: ClientId(1),
                tx_id: TransactionId(1),
                payment_type: PaymentType::Deposit,
                amount: MoneyAmount(dec!(100.0)),
                _state: Done,
            }),
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
            ClientPayment::Done(Payment {
                client_id: ClientId(1),
                tx_id: TransactionId(1),
                payment_type: PaymentType::Withdrawal,
                amount: MoneyAmount(dec!(50.0)),
                _state: Done,
            }),
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
            ClientPayment::Done(Payment {
                client_id: ClientId(1),
                tx_id: TransactionId(1),
                payment_type: PaymentType::Deposit,
                amount: MoneyAmount(dec!(100.0)),
                _state: Done,
            }),
        );
        payment_db.insert(
            (ClientId(1), TransactionId(2)),
            ClientPayment::Done(Payment {
                client_id: ClientId(1),
                tx_id: TransactionId(2),
                payment_type: PaymentType::Withdrawal,
                amount: MoneyAmount(dec!(30.0)),
                _state: Done,
            }),
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
            ClientPayment::Done(Payment {
                client_id: ClientId(1),
                tx_id: TransactionId(1),
                payment_type: PaymentType::Deposit,
                amount: MoneyAmount(dec!(100.0)),
                _state: Done,
            }),
        );
        payment_db.insert(
            (ClientId(1), TransactionId(2)),
            ClientPayment::OnDispute(Payment {
                client_id: ClientId(1),
                tx_id: TransactionId(2),
                payment_type: PaymentType::Deposit,
                amount: MoneyAmount(dec!(50.0)),
                _state: OnDispute,
            }),
        );
        payment_db.insert(
            (ClientId(1), TransactionId(3)),
            ClientPayment::Resolved(Payment {
                client_id: ClientId(1),
                tx_id: TransactionId(3),
                payment_type: PaymentType::Deposit,
                amount: MoneyAmount(dec!(25.0)),
                _state: Resolved,
            }),
        );

        let report = generate_accounts_report(payment_db);

        let status = report.get(&ClientId(1)).unwrap();
        assert_eq!(status.available, MoneyAmount(dec!(125.0))); // 100 + 25
        assert_eq!(status.held, MoneyAmount(dec!(50.0)));
        assert_eq!(status.total, MoneyAmount(dec!(175.0))); // 100 + 50 + 25
        assert!(!status.locked);
    }

    #[test]
    fn generate_report_deposit_then_chargeback() {
        let mut payment_db: HashMap<(ClientId, TransactionId), ClientPayment> = HashMap::new();
        payment_db.insert(
            (ClientId(1), TransactionId(1)),
            ClientPayment::Done(Payment {
                client_id: ClientId(1),
                tx_id: TransactionId(1),
                payment_type: PaymentType::Deposit,
                amount: MoneyAmount(dec!(100.0)),
                _state: Done,
            }),
        );
        payment_db.insert(
            (ClientId(1), TransactionId(2)),
            ClientPayment::ChargedBack(Payment {
                client_id: ClientId(1),
                tx_id: TransactionId(2),
                payment_type: PaymentType::Deposit,
                amount: MoneyAmount(dec!(50.0)),
                _state: ChargedBack,
            }),
        );

        let report = generate_accounts_report(payment_db);

        let status = report.get(&ClientId(1)).unwrap();
        assert_eq!(status.available, MoneyAmount(dec!(50.0))); // 100 - 50
        assert_eq!(status.held, MoneyAmount(dec!(0.0)));
        assert_eq!(status.total, MoneyAmount(dec!(50.0)));
        assert!(status.locked);
    }

    #[test]
    fn generate_report_complex_scenario() {
        let mut payment_db: HashMap<(ClientId, TransactionId), ClientPayment> = HashMap::new();

        // Client 1: deposit 100, withdrawal 30, dispute 40, resolve with 40 more
        payment_db.insert(
            (ClientId(1), TransactionId(1)),
            ClientPayment::Done(Payment {
                client_id: ClientId(1),
                tx_id: TransactionId(1),
                payment_type: PaymentType::Deposit,
                amount: MoneyAmount(dec!(100.0)),
                _state: Done,
            }),
        );
        payment_db.insert(
            (ClientId(1), TransactionId(2)),
            ClientPayment::Done(Payment {
                client_id: ClientId(1),
                tx_id: TransactionId(2),
                payment_type: PaymentType::Withdrawal,
                amount: MoneyAmount(dec!(30.0)),
                _state: Done,
            }),
        );
        payment_db.insert(
            (ClientId(1), TransactionId(3)),
            ClientPayment::Resolved(Payment {
                client_id: ClientId(1),
                tx_id: TransactionId(3),
                payment_type: PaymentType::Deposit,
                amount: MoneyAmount(dec!(40.0)),
                _state: Resolved,
            }),
        );

        // Client 2: deposit 250, chargeback 50
        payment_db.insert(
            (ClientId(2), TransactionId(4)),
            ClientPayment::Done(Payment {
                client_id: ClientId(2),
                tx_id: TransactionId(4),
                payment_type: PaymentType::Deposit,
                amount: MoneyAmount(dec!(250.0)),
                _state: Done,
            }),
        );
        payment_db.insert(
            (ClientId(2), TransactionId(5)),
            ClientPayment::ChargedBack(Payment {
                client_id: ClientId(2),
                tx_id: TransactionId(5),
                payment_type: PaymentType::Deposit,
                amount: MoneyAmount(dec!(50.0)),
                _state: ChargedBack,
            }),
        );

        let report = generate_accounts_report(payment_db);

        // Client 1: available = 100 - 30 + 40 = 110, held = 0, total = 110, not locked
        let status1 = report.get(&ClientId(1)).unwrap();
        assert_eq!(status1.available, MoneyAmount(dec!(110.0)));
        assert_eq!(status1.held, MoneyAmount(dec!(0.0)));
        assert_eq!(status1.total, MoneyAmount(dec!(110.0)));
        assert!(!status1.locked);

        // Client 2: available = 250 - 50 = 200, held = 0, total = 200, locked
        let status2 = report.get(&ClientId(2)).unwrap();
        assert_eq!(status2.available, MoneyAmount(dec!(200.0)));
        assert_eq!(status2.held, MoneyAmount(dec!(0.0)));
        assert_eq!(status2.total, MoneyAmount(dec!(200.0)));
        assert!(status2.locked);
    }

    #[test]
    fn complex_workflow_with_withdrawal_deposit_dispute_chargeback() {
        let mut payment_db: HashMap<(ClientId, TransactionId), ClientPayment> = HashMap::new();

        // Deposit 500
        payment_db.insert(
            (ClientId(1), TransactionId(1)),
            ClientPayment::Done(Payment {
                client_id: ClientId(1),
                tx_id: TransactionId(1),
                payment_type: PaymentType::Deposit,
                amount: MoneyAmount(dec!(500.0)),
                _state: Done,
            }),
        );

        // Withdrawal 200
        payment_db.insert(
            (ClientId(1), TransactionId(2)),
            ClientPayment::Done(Payment {
                client_id: ClientId(1),
                tx_id: TransactionId(2),
                payment_type: PaymentType::Withdrawal,
                amount: MoneyAmount(dec!(200.0)),
                _state: Done,
            }),
        );

        // Deposit 300 (later disputed)
        payment_db.insert(
            (ClientId(1), TransactionId(3)),
            ClientPayment::OnDispute(Payment {
                client_id: ClientId(1),
                tx_id: TransactionId(3),
                payment_type: PaymentType::Deposit,
                amount: MoneyAmount(dec!(300.0)),
                _state: OnDispute,
            }),
        );

        // Deposit 150 (later charged back)
        payment_db.insert(
            (ClientId(1), TransactionId(4)),
            ClientPayment::ChargedBack(Payment {
                client_id: ClientId(1),
                tx_id: TransactionId(4),
                payment_type: PaymentType::Deposit,
                amount: MoneyAmount(dec!(150.0)),
                _state: ChargedBack,
            }),
        );

        let report = generate_accounts_report(payment_db);
        let status = report.get(&ClientId(1)).unwrap();

        // Calculate expected values:
        // available = 500 - 200 - 150 = 150
        // held = 300 (from dispute)
        // total = 150 + 300 = 450
        // locked = true (due to chargeback)

        assert_eq!(status.available, MoneyAmount(dec!(150.0)));
        assert_eq!(status.held, MoneyAmount(dec!(300.0)));
        assert_eq!(status.total, MoneyAmount(dec!(450.0)));
        assert!(status.locked);

        // Verify: total = available + held
        assert_eq!(status.available + status.held, status.total);
    }
}
