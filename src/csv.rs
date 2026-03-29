use crate::models::{
    ClientId, ClientPayment, Done, MoneyAmount, Payment, PaymentType, TransactionId,
};
use csv::{Reader, ReaderBuilder, Trim, Writer, WriterBuilder};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::Stdout;

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TransactionType {
    Deposit,
    Withdrawal,
    Dispute,
    Resolve,
    Chargeback,
}

impl TryInto<PaymentType> for TransactionType {
    type Error = String;

    fn try_into(self) -> Result<PaymentType, Self::Error> {
        match self {
            TransactionType::Deposit => Ok(PaymentType::Deposit),
            TransactionType::Withdrawal => Ok(PaymentType::Withdrawal),
            _ => Err(format!(
                "Transaction type {self:?} cannot be converted to PaymentType"
            )),
        }
    }
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
pub struct TransactionRecord {
    #[serde(rename = "type")]
    pub transaction_type: TransactionType,
    #[serde(rename = "client")]
    pub client_id: u16,
    #[serde(rename = "tx")]
    pub tx_id: u32,
    pub amount: Option<MoneyAmount>,
}

impl TryInto<Payment<Done>> for TransactionRecord {
    type Error = String;

    fn try_into(self) -> Result<Payment<Done>, Self::Error> {
        let money_amount = self.amount.ok_or(format!(
            "Missing amount for transaction type {:?}",
            self.transaction_type
        ))?;

        let payment_type: PaymentType = self.transaction_type.try_into()?;

        Ok(Payment {
            tx_id: TransactionId(self.tx_id),
            payment_type,
            client_id: ClientId(self.client_id),
            amount: money_amount.abs(),
            _state: Done,
        })
    }
}

#[derive(Serialize, Default)]
pub struct ClientStatus {
    pub(crate) client: ClientId,
    pub(crate) available: MoneyAmount,
    pub(crate) held: MoneyAmount,
    pub(crate) total: MoneyAmount,
    pub(crate) locked: bool,
}

impl ClientStatus {
    pub(crate) fn new(client: ClientId) -> Self {
        ClientStatus {
            client,
            ..Default::default()
        }
    }
}

pub fn create_csv_reader(path: String) -> csv::Result<Reader<File>> {
    ReaderBuilder::new()
        .trim(Trim::All)
        .has_headers(true)
        .from_path(path)
}

pub fn create_csv_output_writer() -> Writer<Stdout> {
    WriterBuilder::new()
        .has_headers(true)
        .from_writer(std::io::stdout())
}

pub fn save_payments_csv(
    payment_db: &HashMap<(ClientId, TransactionId), ClientPayment>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut csv_writer = WriterBuilder::new()
        .has_headers(true)
        .from_path("./payments.csv")?;

    for client_payment in payment_db.values() {
        // this pattern matching may look redundant, but it is necessary to serialize the inner payment properly
        // this also allows serializing multiple payment in multiple states at once
        match client_payment {
            ClientPayment::Done(payment) => {
                csv_writer.serialize(payment)?;
            }
            ClientPayment::OnDispute(payment) => {
                csv_writer.serialize(payment)?;
            }
            ClientPayment::Resolved(payment) => {
                csv_writer.serialize(payment)?;
            }
            ClientPayment::ChargedBack(payment) => {
                csv_writer.serialize(payment)?;
            }
        }
    }

    Ok(())
}

pub fn save_accounts_csv(
    accounts: HashMap<ClientId, ClientStatus>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut csv_writer = create_csv_output_writer();

    for client_status in accounts.values() {
        csv_writer.serialize(client_status)?
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::dec;

    #[test]
    fn deserializes_transaction_record() {
        let test_file_path = "./resources/transactions.csv".to_owned();
        let mut csv = create_csv_reader(test_file_path).unwrap();

        let expected = vec![
            TransactionRecord {
                transaction_type: TransactionType::Deposit,
                client_id: 1,
                tx_id: 1,
                amount: Some(MoneyAmount(dec!(100.0))),
            },
            TransactionRecord {
                transaction_type: TransactionType::Deposit,
                client_id: 2,
                tx_id: 2,
                amount: Some(MoneyAmount(dec!(250.0))),
            },
            TransactionRecord {
                transaction_type: TransactionType::Withdrawal,
                client_id: 1,
                tx_id: 3,
                amount: Some(MoneyAmount(dec!(40))),
            },
            TransactionRecord {
                transaction_type: TransactionType::Withdrawal,
                client_id: 2,
                tx_id: 4,
                amount: Some(MoneyAmount(dec!(75))),
            },
            TransactionRecord {
                transaction_type: TransactionType::Deposit,
                client_id: 3,
                tx_id: 5,
                amount: Some(MoneyAmount(dec!(250.0))),
            },
            TransactionRecord {
                transaction_type: TransactionType::Dispute,
                client_id: 1,
                tx_id: 1,
                amount: None,
            },
            TransactionRecord {
                transaction_type: TransactionType::Dispute,
                client_id: 2,
                tx_id: 2,
                amount: None,
            },
            TransactionRecord {
                transaction_type: TransactionType::Dispute,
                client_id: 3,
                tx_id: 5,
                amount: None,
            },
            TransactionRecord {
                transaction_type: TransactionType::Resolve,
                client_id: 1,
                tx_id: 1,
                amount: None,
            },
            TransactionRecord {
                transaction_type: TransactionType::Chargeback,
                client_id: 2,
                tx_id: 2,
                amount: None,
            },
            TransactionRecord {
                transaction_type: TransactionType::Dispute,
                client_id: 2,
                tx_id: 4,
                amount: None,
            },
            TransactionRecord {
                transaction_type: TransactionType::Resolve,
                client_id: 1,
                tx_id: 3,
                amount: None,
            },
            TransactionRecord {
                transaction_type: TransactionType::Chargeback,
                client_id: 1,
                tx_id: 3,
                amount: None,
            },
            TransactionRecord {
                transaction_type: TransactionType::Dispute,
                client_id: 1,
                tx_id: 9,
                amount: None,
            },
        ];

        let mut records = Vec::<TransactionRecord>::new();
        for record in csv.deserialize::<TransactionRecord>() {
            records.push(record.unwrap());
        }

        assert_eq!(expected.len(), records.len());

        for idx in 0..expected.len() {
            assert_eq!(expected[idx], records[idx]);
        }
    }
}
