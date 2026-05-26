#![no_std]

use soroban_sdk::{ 
    contract, 
    contracttype, 
    contractimpl, 
    vec, 
    Vec, 
    Env, 
    Address, 
    String, 
    Symbol, 
    TryFromVal, 
    ConversionError 
};

// ===== DATA TYPES =====

// Escrow status: Pending, Funded, Delivered, Completed, Disputed, Refunded
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum EscrowStatus {
    Pending,    // Invoice created, awaiting client funding
    Funded,     // Client deposited USDC, escrow locked
    Delivered,  // Freelancer submitted proof of work
    Completed,  // Client approved, funds released
    Disputed,   // Client raised issue
    Refunded,   // Client refunded after dispute
}

// Invoice struct storing all escrow details
#[contracttype]
#[derive(Clone)]
pub struct Invoice {
    pub id: u32,
    pub freelancer: Address,
    pub client: Address,
    pub amount: i128,           // USDC amount (7 decimals like USDC)
    pub deadline: u64,          // UNIX timestamp deadline
    pub deliverable_hash: [u8; 32], // SHA-256 hash of deliverable
    pub status: EscrowStatus,
    pub created_at: u64,
}

// Storage keys for persistent data
#[contracttype]
pub enum StorageKey {
    Invoice(u32),           // Maps invoice_id -> Invoice
    InvoiceCount,         // Total invoices created
    UserInvoices(Address), // Maps user -> Vec<invoice_id>
}

// ===== CONTRACT STRUCT =====

#[contract]
pub struct GigPayContract;

// ===== CONTRACT IMPLEMENTATION =====

#[contractimpl]
impl GigPayContract {
    // Create a new invoice for a gig job
    // 
    // # Arguments
    // * `env` - Soroban environment
    // * `freelancer` - Address of the freelancer receiving payment
    // * `client` - Address of the client paying
    // * `amount` - USDC amount (7 decimals)
    // * `deadline` - UNIX timestamp when work must be delivered
    // * `deliverable_hash` - SHA-256 hash of expected deliverable file
    //
    // # Returns
    // * `u32` - The created invoice ID
    pub fn create_invoice(
        env: Env,
        freelancer: Address,
        client: Address,
        amount: i128,
        deadline: u64,
        deliverable_hash: [u8; 32],
    ) -> u32 {
        // Increment invoice counter
        let invoice_count: u32 = env
            .storage()
            .get(&StorageKey::InvoiceCount)
            .unwrap_or(Ok(0u32.into()))
            .unwrap();
        let new_id = invoice_count + 1;
        
        env.storage().set(&StorageKey::InvoiceCount, &new_id);

        // Create invoice record
        let invoice = Invoice {
            id: new_id,
            freelancer,
            client,
            amount,
            deadline,
            deliverable_hash,
            status: EscrowStatus::Pending,
            created_at: env.ledger().timestamp(),
        };

        env.storage().set(&StorageKey::Invoice(new_id), &invoice);

        // Track user's invoices
        let mut freelancer_invoices: Vec<u32> = env
            .storage()
            .get(&StorageKey::UserInvoices(invoice.freelancer.clone()))
            .unwrap_or(Ok(vec![&env]))
            .unwrap();
        freelancer_invoices.push_back(new_id);
        env.storage().set(
            &StorageKey::UserInvoices(invoice.freelancer.clone()),
            &freelancer_invoices,
        );

        new_id
    }

    // Client funds the escrow
    // 
    // Transfers USDC from client to contract hold.
    // 
    // # Arguments
    // * `env` - Soroban environment
    // * `invoice_id` - ID of invoice to fund
    // * `client` - Must match invoice.client
    pub fn fund_escrow(env: Env, invoice_id: u32, client: Address) {
        // Load invoice
        let key = StorageKey::Invoice(invoice_id);
        let mut invoice: Invoice = env
            .storage()
            .get(&key)
            .unwrap_or(Ok(Invoice {
                id: 0,
                freelancer: Address::from_array([0u8; 32]),
                client: Address::from_array([0u8; 32]),
                amount: 0,
                deadline: 0,
                deliverable_hash: [0u8; 32],
                status: EscrowStatus::Pending,
                created_at: 0,
            }))
            .unwrap();

        // Authorization: only client can fund
        client.require_auth();

        // Ensure invoice is in Pending status
        if invoice.status != EscrowStatus::Pending {
            panic!("Invoice not pending");
        }

        // Ensure caller is the client
        if client != invoice.client {
            panic!("Not the client");
        }

        // Transfer USDC from client to contract (simulated via token balance)
        // In production: call USDC token contract's transfer() here
        // For demo: we track it in storage
        let balance_key = format!("balance_{}", invoice_id);
        env.storage().set(&balance_key, &invoice.amount);

        // Update status
        invoice.status = EscrowStatus::Funded;
        env.storage().set(&key, &invoice);
    }

    // Freelancer submits proof of work
    // 
    // # Arguments
    // * `env` - Soroban environment
    // * `invoice_id` - ID of invoice
    // * `proof_hash` - SHA-256 hash of submitted deliverable
    pub fn submit_proof(env: Env, invoice_id: u32, proof_hash: [u8; 32]) {
        let key = StorageKey::Invoice(invoice_id);
        let mut invoice: Invoice = env.storage().get(&key).unwrap();

        // Authorization
        invoice.freelancer.require_auth();

        // Verify status is Funded
        if invoice.status != EscrowStatus::Funded {
            panic!("Escrow not funded");
        }

        // Update status to Delivered (doesn't verify hash - for demo)
        invoice.status = EscrowStatus::Delivered;
        env.storage().set(&key, &invoice);
    }

    // Client approves and releases funds
    // 
    // # Arguments
    // * `env` - Soroban environment
    // * `invoice_id` - ID of invoice
    pub fn approve_and_release(env: Env, invoice_id: u32) {
        let key = StorageKey::Invoice(invoice_id);
        let mut invoice: Invoice = env.storage().get(&key).unwrap();

        // Authorization
        invoice.client.require_auth();

        // Verify status is Delivered
        if invoice.status != EscrowStatus::Delivered {
            panic!("Work not delivered");
        }

        // Transfer USDC to freelancer
        let balance_key = format!("balance_{}", invoice_id);
        let amount: i128 = env.storage().get(&balance_key).unwrap();

        // In production: call USDC token contract's transfer() to freelancer
        // For demo: we just record the release
        invoice.status = EscrowStatus::Completed;
        env.storage().set(&key, &invoice);
    }

    // Auto-release funds after deadline (for demo: simplified)
    pub fn auto_release_after_deadline(env: Env, invoice_id: u32) {
        let key = StorageKey::Invoice(invoice_id);
        let mut invoice: Invoice = env.storage().get(&key).unwrap();

        // Check deadline passed
        if invoice.deadline > env.ledger().timestamp() {
            panic!("Deadline not passed");
        }

        // Only if delivered
        if invoice.status == EscrowStatus::Delivered {
            invoice.status = EscrowStatus::Completed;
            env.storage().set(&key, &invoice);
        }
    }

    // Get invoice details
    pub fn get_invoice(env: Env, invoice_id: u32) -> Invoice {
        env.storage()
            .get(&StorageKey::Invoice(invoice_id))
            .unwrap()
    }
} 