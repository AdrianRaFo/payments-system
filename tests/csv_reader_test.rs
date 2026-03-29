use payments_system::csv::{TransactionRecord, TransactionType, create_csv_reader};
use payments_system::models::MoneyAmount;
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

    let records = csv
        .deserialize::<TransactionRecord>()
        .map(|record| record.unwrap())
        .collect::<Vec<_>>();

    assert_eq!(expected, records);
}
