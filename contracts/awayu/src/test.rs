#![cfg(test)]

mod tests {
    use soroban_sdk::testutils::Address as _;
    use soroban_sdk::{
        vec, 
        Env, 
        Address, 
        String, 
        Symbol, 
        TryFromVal, 
        ConversionError,
    };
    use crate::{GigPayContract, Invoice, EscrowStatus};

    // Helper: Generate a dummy 32-byte hash
    fn dummy_hash(n: u8) -> [u8; 32] {
        [n; 32]
    }

    // Helper: Generate a dummy address
    fn dummy_address(n: u8) -> Address {
        let mut bytes = [0u8; 32];
        bytes[0] = n;
        Address::from_array(bytes)
    }

    // ===== TEST 1: Happy Path - Create Invoice, Fund, Submit, Release =====

    #[test]
    fn test_full_escrow_flow() {
        let env = Env::default();
        
        // Setup addresses
        let freelancer = dummy_address(1);
        let client = dummy_address(2);
        
        let contract = GigPayContractClient::new(&env, &dummy_address(0));
        
        // Step 1: Create invoice
        let invoice_id = contract.create_invoice(
            &freelancer,
            &client,
            &1000_0000i128, // 1000 USDC
            &1700000000u64, // Future deadline
            &dummy_hash(42),
        );
        
        assert_eq!(invoice_id, 1);
        
        // Step 2: Client funds escrow
        client.require_auth();
        contract.fund_escrow(&invoice_id, &client);
        
        let invoice = contract.get_invoice(&invoice_id);
        assert_eq!(invoice.status, EscrowStatus::Funded);
        
        // Step 3: Freelancer submits proof
        freelancer.require_auth();
        contract.submit_proof(&invoice_id, &dummy_hash(42));
        
        let invoice = contract.get_invoice(&invoice_id);
        assert_eq!(invoice.status, EscrowStatus::Delivered);
        
        // Step 4: Client approves and release
        client.require_auth();
        contract.approve_and_release(&invoice_id);
        
        let invoice = contract.get_invoice(&invoice_id);
        assert_eq!(invoice.status, EscrowStatus::Completed);
    }

    // ===== TEST 2: Edge Case - Unauthorized Caller =====

    #[test]
    #[should_panic(expected = "Not the client")]
    fn test_fund_with_wrong_client() {
        let env = Env::default();
        
        let freelancer = dummy_address(1);
        let client = dummy_address(2