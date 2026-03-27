# Payments System

This is a toy payments system where I show how to handle most common transaction operations

## System design

### Key Decision: Rounding Strategy

Monetary values are rounded to **4 decimal places** using `RoundingStrategy::MidpointNearestEven` (banker's rounding).

This policy is implemented in `MoneyAmount::new` (`src/models.rs`) and is used to reduce cumulative rounding bias across many transactions.

---

### AI Usage

Some parts of this README were created with IA assistance and reviewed by the project author.

