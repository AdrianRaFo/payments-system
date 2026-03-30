use crate::csv::{
    ClientStatus, TransactionRecord, TransactionType, create_csv_reader, save_accounts_csv,
    save_payments_csv,
};
use crate::models::{ClientId, ClientPayment, Done, Payment, PaymentType, TransactionId};
use std::collections::{HashMap, HashSet};

pub mod csv;
pub mod models;

pub fn process(path: String) -> Result<(), Box<dyn std::error::Error>> {
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

pub fn process_csv_record(
    locked_clients: &mut HashSet<ClientId>,
    payment_db: &mut HashMap<(ClientId, TransactionId), ClientPayment>,
    tx: TransactionRecord,
) -> Result<(), Box<dyn std::error::Error>> {
    if locked_clients.get(&ClientId(tx.client_id)).is_some() {
        return Ok(());
    }
    let key = (ClientId(tx.client_id), TransactionId(tx.tx_id));

    match tx.transaction_type {
        TransactionType::Deposit | TransactionType::Withdrawal => {
            let payment: Payment<Done> = tx.try_into()?;
            let client_payment = ClientPayment::Done(payment);
            payment_db.entry(key).or_insert_with(|| client_payment);
        }
        TransactionType::Dispute => {
            if !payment_db.contains_key(&key) {
                eprintln!("Warning: No payment found. Transaction {tx:?} was skipped");
                return Ok(());
            }

            payment_db.entry(key).and_modify(|client_payment| match client_payment {
                ClientPayment::Done(payment) => match payment.disputed().map(ClientPayment::OnDispute)
                {
                    Ok(disputed) => *client_payment = disputed,
                    Err(e) => eprintln!("{e}"),
                },
                _ => {
                    eprintln!(
                        "Warning: Dispute can't be opened on a {:?} payment. Transaction {key:?} was skipped",
                        client_payment.state()
                    );
                }
            });
        }
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
                        *client_payment = ClientPayment::ChargedBack(payment.charge_back());
                        locked_clients.insert(ClientId(tx.client_id));
                    }
                }
                _ => {
                    eprintln!(
                        "Warning: {:?} can't be done on a {:?} payment. Transaction {tx:?} was skipped",
                        tx.transaction_type,
                        client_payment.state()
                    );
                }
            });
        }
    }

    Ok(())
}

pub fn generate_accounts_report(
    payment_db: HashMap<(ClientId, TransactionId), ClientPayment>,
) -> HashMap<ClientId, ClientStatus> {
    let mut report: HashMap<ClientId, ClientStatus> = HashMap::new();
    for ((client_id, _), payment) in payment_db {
        let client_status = report
            .entry(client_id)
            .or_insert(ClientStatus::new(client_id));

        match payment {
            ClientPayment::Done(payment) => {
                if payment.payment_type == PaymentType::Deposit {
                    // Deposit payment on client's account, available and total amounts increases
                    client_status.available += payment.amount;
                    client_status.total += payment.amount;
                } else {
                    // Withdrawal payment on client's account, available and total amounts decreases
                    client_status.available -= payment.amount;
                    client_status.total -= payment.amount;
                }
            }
            // A deposit payment is present but under an open dispute,
            // the total amount increases but money is held instead of available
            ClientPayment::OnDispute(payment) => {
                client_status.held += payment.amount;
                client_status.total += payment.amount;
            }
            // The payment went through a dispute and was successfully resolved,
            // the available and total amounts increases like a regular Deposit
            ClientPayment::Resolved(payment) => {
                client_status.available += payment.amount;
                client_status.total += payment.amount;
            }
            // The original deposit went through a dispute and was charged back,
            // the amounts remain the same but the client's account is locked
            ClientPayment::ChargedBack(_) => {
                client_status.locked = true;
            }
        }
    }
    report
}
