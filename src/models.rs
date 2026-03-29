use crate::models::PaymentType::Deposit;
use rust_decimal::{Decimal, RoundingStrategy};
use serde::{Deserialize, Serialize, Serializer};
use std::fmt::Debug;
use std::ops::{Add, AddAssign, SubAssign};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
// This struct is a wrapper around Decimal to have a custom deserializer that keeps the decimals consistent
pub struct MoneyAmount(pub Decimal);

impl MoneyAmount {
    pub fn abs(&self) -> Self {
        MoneyAmount(self.0.abs())
    }
}

impl Add for MoneyAmount {
    type Output = MoneyAmount;

    fn add(self, second: Self) -> MoneyAmount {
        MoneyAmount(self.0 + second.0)
    }
}

impl AddAssign for MoneyAmount {
    fn add_assign(&mut self, second: Self) {
        *self = MoneyAmount(self.0 + second.0);
    }
}

impl SubAssign for MoneyAmount {
    fn sub_assign(&mut self, second: Self) {
        *self = MoneyAmount(self.0 - second.0)
    }
}

impl Serialize for MoneyAmount {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        // This avoids conflicts on method resolution with rust_decimal methods
        <Decimal as Serialize>::serialize(&self.0, serializer)
    }
}

impl<'de> Deserialize<'de> for MoneyAmount {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        // This avoids conflicts on method resolution with rust_decimal methods
        <Decimal as Deserialize>::deserialize(deserializer).map(|decimal| {
            MoneyAmount(
                decimal
                    .normalize()
                    .round_dp_with_strategy(4, RoundingStrategy::MidpointNearestEven),
            )
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
pub struct TransactionId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Default)]
pub struct ClientId(pub u16);

// Defining the enum like this will allow to define Payment with a Type Parameter
// and then define in the models which state changes are allowed
pub trait PaymentState {}

#[derive(Debug, Serialize)]
pub struct Done;
impl PaymentState for Done {}

#[derive(Debug, Serialize)]
pub struct OnDispute;
impl PaymentState for OnDispute {}

#[derive(Debug, Serialize)]
pub struct Resolved;
impl PaymentState for Resolved {}

#[derive(Debug, Serialize)]
pub struct ChargedBack;
impl PaymentState for ChargedBack {}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum PaymentType {
    Deposit,
    Withdrawal,
}

#[derive(Debug, Serialize)]
pub struct Payment<S: PaymentState> {
    pub(crate) client_id: ClientId,
    pub(crate) tx_id: TransactionId,
    pub(crate) payment_type: PaymentType,
    pub(crate) amount: MoneyAmount,
    pub(crate) _state: S,
}

// Only payments that has not been disputed before can be disputed
impl Payment<Done> {
    // according to the definition of Dispute only the client with the payment as Deposit will get the dispute
    pub(crate) fn disputed(&self) -> Result<Payment<OnDispute>, String> {
        if self.payment_type == Deposit {
            Ok(Payment {
                tx_id: self.tx_id,
                payment_type: self.payment_type,
                client_id: self.client_id,
                amount: self.amount,
                _state: OnDispute,
            })
        } else {
            Err(format!(
                "Warning: Only Deposit transactions can be disputed. Transaction {self:?} was skipped"
            ))
        }
    }
}

// A disputed payment can have two resolutions: Resolved or ChargedBack
impl Payment<OnDispute> {
    pub(crate) fn resolved(&self) -> Payment<Resolved> {
        Payment {
            tx_id: self.tx_id,
            payment_type: self.payment_type,
            client_id: self.client_id,
            amount: self.amount,
            _state: Resolved,
        }
    }
    pub(crate) fn charge_back(&self) -> Payment<ChargedBack> {
        Payment {
            tx_id: self.tx_id,
            payment_type: self.payment_type,
            client_id: self.client_id,
            amount: self.amount,
            _state: ChargedBack,
        }
    }
}

#[derive(Debug)]
pub enum ClientPayment {
    Done(Payment<Done>),
    OnDispute(Payment<OnDispute>),
    Resolved(Payment<Resolved>),
    ChargedBack(Payment<ChargedBack>),
}

impl ClientPayment {
    pub(crate) fn state(&self) -> String {
        match self {
            ClientPayment::Done(_) => "Done".to_string(),
            ClientPayment::OnDispute(_) => "OnDispute".to_string(),
            ClientPayment::Resolved(_) => "Resolved".to_string(),
            ClientPayment::ChargedBack(_) => "ChargedBack".to_string(),
        }
    }
}
