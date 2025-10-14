//! Tests for integer overflow vulnerabilities in fee calculations
//!
//! This module contains tests that demonstrate how integer overflow
//! vulnerabilities in fee calculations have been fixed.

#[cfg(test)]
mod overflow_attack_tests {
    use crate::types::safe_arithmetic::ArithmeticError;
    use crate::types::transaction::{Transaction, TransactionInput, TransactionOutput};
    use crate::types::transaction_safe::TransactionSafe;

    /// Create a transaction with specified output amounts
    fn create_transaction_with_outputs(amounts: Vec<u64>) -> Transaction {
        let inputs = vec![TransactionInput::new([0u8; 32], 0, vec![], 0xffffffff)];
        let outputs = amounts
            .into_iter()
            .map(|amount| TransactionOutput::new(amount, vec![]))
            .collect();
        Transaction::new(1, inputs, outputs, 0)
    }

    #[test]
    fn test_negative_fee_attack_prevented() {
        // Attack scenario: Create outputs that sum to more than u64::MAX
        // This would overflow and wrap around to a small number, allowing
        // an attacker to create money from nothing

        let tx = create_transaction_with_outputs(vec![
            u64::MAX - 1000, // Almost max
            2000,            // Add more than 1000
        ]);

        // Old vulnerable code would overflow here
        // let total = tx.total_output(); // Would wrap to ~1000

        // New safe code detects overflow
        let result = tx.total_output_safe();
        assert_eq!(result, Err(ArithmeticError::AdditionOverflow));
    }

    #[test]
    fn test_fee_calculation_overflow_prevented() {
        // Attack scenario: Huge fee rate * huge size = overflow
        let fee_rate = u64::MAX / 2;
        let size = 1000usize;

        // Old vulnerable code:
        // let fee = fee_rate * size as u64; // Overflow!

        // New safe code:
        use crate::types::safe_arithmetic::calculate_fee_safe;
        let result = calculate_fee_safe(fee_rate, size);
        assert_eq!(result, Err(ArithmeticError::MultiplicationOverflow));
    }

    #[test]
    fn test_multiple_output_overflow_attack() {
        // Attack scenario: Many outputs that individually seem fine
        // but together overflow
        let outputs = vec![
            u64::MAX / 4,
            u64::MAX / 4,
            u64::MAX / 4,
            u64::MAX / 4,
            100, // This pushes it over
        ];

        let tx = create_transaction_with_outputs(outputs);

        // Safe validation catches this
        let get_output = |_: &[u8; 32], _: u32| Some(TransactionOutput::new(1000, vec![]));

        assert!(!tx.validate_safe(&get_output));
    }

    #[test]
    fn test_rbf_fee_overflow_attack() {
        // Attack scenario: RBF replacement with overflowing fee calculation
        // Attacker tries to replace transaction with one that has overflow fee

        let original_fee_rate = 100u64;
        let original_size = 250usize;
        let original_fee = original_fee_rate * original_size as u64; // 25,000

        // Attacker creates replacement with overflow
        let attack_fee_rate = u64::MAX;
        let attack_size = 10usize;

        use crate::types::safe_arithmetic::calculate_fee_safe;

        // Old code would overflow and wrap to small number
        // let attack_fee = attack_fee_rate * attack_size as u64; // Overflow!

        // New safe code prevents this
        let result = calculate_fee_safe(attack_fee_rate, attack_size);
        assert_eq!(result, Err(ArithmeticError::MultiplicationOverflow));

        // RBF would be rejected because fee can't be calculated
    }

    #[test]
    fn test_mempool_total_fee_overflow() {
        // Attack scenario: Mempool with transactions that cause total fee overflow
        use crate::types::safe_arithmetic::sum_safe;

        let tx_fees = vec![
            u64::MAX / 2,
            u64::MAX / 2,
            1000, // This would cause overflow
        ];

        // Old vulnerable code:
        // let total: u64 = tx_fees.iter().sum(); // Overflow!

        // New safe code:
        let result = sum_safe(tx_fees.into_iter());
        assert_eq!(result, Err(ArithmeticError::AdditionOverflow));
    }

    #[test]
    fn test_percentage_fee_overflow() {
        // Attack scenario: Percentage-based fee calculation overflow
        use crate::types::safe_arithmetic::percentage_safe;

        let base_value = u64::MAX / 50;
        let percentage = 200u64; // 200%

        // Old code might do: (base_value * percentage) / 100
        // This would overflow in the multiplication step

        let result = percentage_safe(base_value, percentage);
        assert_eq!(result, Err(ArithmeticError::MultiplicationOverflow));
    }

    #[test]
    fn test_valid_large_transactions_still_work() {
        // Ensure legitimate large transactions still work
        let tx = create_transaction_with_outputs(vec![
            1_000_000_000_000, // 1 trillion satoshis (10,000 NOVA)
            500_000_000_000,   // 500 billion satoshis (5,000 NOVA)
            250_000_000_000,   // 250 billion satoshis (2,500 NOVA)
        ]);

        // This should work fine
        let result = tx.total_output_safe();
        assert_eq!(result, Ok(1_750_000_000_000));

        // Fee calculation should also work
        let get_output =
            |_: &[u8; 32], _: u32| Some(TransactionOutput::new(2_000_000_000_000, vec![]));

        let fee = tx.calculate_fee_safe(&get_output);
        assert_eq!(fee, Ok(250_000_000_000)); // 2T - 1.75T = 0.25T
    }

    #[test]
    fn test_edge_case_exactly_max() {
        // Test edge case where sum equals exactly u64::MAX
        let tx = create_transaction_with_outputs(vec![u64::MAX - 100, 50, 50]);

        let result = tx.total_output_safe();
        assert_eq!(result, Ok(u64::MAX));
    }
}
