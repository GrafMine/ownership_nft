#![allow(unexpected_cfgs)]
use spl_token_2022::{ extension::{metadata_pointer, ExtensionType, group_pointer}, state::Mint as MintState};
use anchor_spl::token_2022::{self as token_2022_program, InitializeMint, Token2022};
use anchor_spl::associated_token::AssociatedToken;
use solana_program::system_instruction;
use anchor_lang::solana_program;
use anchor_spl::token::Token;
use anchor_lang::prelude::*;
use mpl_token_metadata::{
    types::{Creator, TokenStandard, CollectionDetails},
    instructions::CreateV1Builder,
    ID as MPL_TOKEN_METADATA_PROGRAM_ID
};
use std::fmt::Write;
use hex;

declare_id!("6HJN3E7nkbExcwfw8YkztMFC2vcfPBQwmDLrkEMJqnqM");

#[error_code]
pub enum ErrorCode {

    #[msg("Invalid server signer")]
    InvalidServerSigner,

    #[msg("Invalid program ID")]
    InvalidProgram,
}

#[constant]
pub const BASE_METADATA_URL: &str = "http://localhost:3000";

#[constant]
pub static ADMIN_TICKET_VALIDATOR: Pubkey = Pubkey::new_from_array([
    196,49,119,241,84,72,174,21,39,203,148,43,111,97,189,117,219,157,187,242,107,205,96,30,175,144,175,16,189,127,73,85
]);

#[constant]
pub const OWNERSHIP_NFT_TRANSFER_HOOK_PROGRAM_ID: Pubkey = Pubkey::new_from_array([1, 2, 3, 4, 5, 130, 19, 173, 21, 58, 108, 43, 179, 33, 211, 237, 222, 201, 145, 188, 175, 181, 142, 126, 0, 68, 162, 19, 143, 142, 77, 119]);

#[constant]
pub const OWNERSHIP_NFT_SYMBOL: &str = "OWNER-TEST-NFT";

#[program]
pub mod owhership_nft {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        msg!("Greetings from: {:?}", ctx.program_id);
        Ok(())
    }

    pub fn init_ownership_nft(
        ctx: Context<InitOwnershipNft>,
        args: InitLotteryTokenArgs,
    ) -> Result<()> {
        let payer = &ctx.accounts.payer;
        let admin = &ctx.accounts.admin;
        let update_authority = &ctx.accounts.update_authority;
        let token_program = &ctx.accounts.token_program;
        let token_metadata_program = &ctx.accounts.token_metadata_program;
        let system_program = &ctx.accounts.system_program;
        let rent = &ctx.accounts.rent;
        let ownership_nft_mint = &ctx.accounts.ownership_nft_mint;
        let ownership_nft_metadata = &ctx.accounts.ownership_nft_metadata;
        let ownership_nft_master_edition = &ctx.accounts.ownership_nft_master_edition;
        let ownership_nft_token_account = &ctx.accounts.ownership_nft_token_account;
    
        let ticket_id_bytes = args.ticket_id.as_ref();

        // --- ДЕБАГ: выводим сиды и PDA ---
        msg!("ticketIdBytes (hex): {}", hex::encode(ticket_id_bytes));
        msg!("ownershipNftMintPda: {}", ownership_nft_mint.key());
        let expected_metadata_pda = Pubkey::find_program_address(
            &[b"metadata", MPL_TOKEN_METADATA_PROGRAM_ID.as_ref(), ownership_nft_mint.key().as_ref()],
            &MPL_TOKEN_METADATA_PROGRAM_ID
        ).0;
        msg!("ownershipNftMetadataPda (calculated): {}", expected_metadata_pda);
        msg!("ownershipNftMetadataPda (from ctx): {}", ownership_nft_metadata.key());
        msg!("ADMIN_TICKET_VALIDATOR: {}", ADMIN_TICKET_VALIDATOR);
        msg!("admin.key(): {}", admin.key());
        // --- конец дебага ---

        // Find the bump seed for the ownership_nft_mint PDA
        let (expected_nft_mint_pda, nft_mint_bump) = Pubkey::find_program_address(
            &[b"lottery_nft_mint", ticket_id_bytes],
            ctx.program_id
        );

        // Assert that the provided account key matches the derived PDA
        // This is crucial security check since we removed the seeds constraint
        require_keys_eq!(ownership_nft_mint.key(), expected_nft_mint_pda, ErrorCode::InvalidProgram); // Or a more specific error

        let nft_mint_bump_bytes = [nft_mint_bump]; // Use the found bump
        let nft_mint_seeds = &[b"lottery_nft_mint".as_ref(), ticket_id_bytes, &nft_mint_bump_bytes[..]][..];
        let nft_mint_signer_seeds = &[nft_mint_seeds];
    
        msg!("Calculating size and rent for Ownership NFT Mint...");
        let extensions = [ExtensionType::MetadataPointer, ExtensionType::GroupPointer];
        let space = ExtensionType::try_calculate_account_len::<MintState>(&extensions)?;
        let lamports = Rent::get()?.minimum_balance(space);

        msg!("Creating Ownership NFT Mint Account (PDA)...");
        let create_nft_mint_account_ix = system_instruction::create_account(
            &payer.key(),
            &ownership_nft_mint.key(),
            lamports,
            space as u64,
            &token_program.key(),
        );
        anchor_lang::solana_program::program::invoke_signed(
            &create_nft_mint_account_ix,
            &[
                payer.to_account_info(),
                ownership_nft_mint.to_account_info(),
                system_program.to_account_info(),
            ],
            nft_mint_signer_seeds,
        )?;
        msg!("Ownership NFT Mint Account created.");
    
        msg!("Initializing Metadata Pointer for Ownership NFT...");
        let init_nft_meta_ptr_ix = metadata_pointer::instruction::initialize(
            &token_program.key(),
            &ownership_nft_mint.key(),
            Some(admin.key()),
            Some(ownership_nft_metadata.key())
        )?;
        anchor_lang::solana_program::program::invoke_signed(
            &init_nft_meta_ptr_ix,
            &[
                ownership_nft_mint.to_account_info(),
                admin.to_account_info(),
            ],
            nft_mint_signer_seeds,
        )?;

        let init_group_ptr_ix = group_pointer::instruction::initialize(
            &token_program.key(),
            &ownership_nft_mint.key(),
            Some(admin.key()),
            Some(ownership_nft_mint.key())
        )?;
        anchor_lang::solana_program::program::invoke_signed(
            &init_group_ptr_ix,
            &[
                ownership_nft_mint.to_account_info(),
                admin.to_account_info(),
            ],
            nft_mint_signer_seeds,
        )?;
    
        msg!("Initializing Ownership NFT Mint data (decimals, authorities)...");
        let init_mint_nft_accounts = InitializeMint {
            mint: ownership_nft_mint.to_account_info(),
            rent: rent.to_account_info(),
        };
        let init_mint_nft_ctx = CpiContext::new_with_signer(
            token_program.to_account_info(),
            init_mint_nft_accounts,
            nft_mint_signer_seeds,
        );
        token_2022_program::initialize_mint(
            init_mint_nft_ctx,
            0,
            &admin.key(),
            Some(&admin.key()),
        )?;
    
        msg!("Creating Ownership NFT ATA via anchor_spl::associated_token::create (Token-2022)...");
        let cpi_accounts = anchor_spl::associated_token::Create {
            payer: payer.to_account_info(),
            associated_token: ownership_nft_token_account.to_account_info(),
            authority: payer.to_account_info(),
            mint: ownership_nft_mint.to_account_info(),
            system_program: system_program.to_account_info(),
            token_program: token_program.to_account_info(), // Token-2022!
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.associated_token_program.to_account_info(), cpi_accounts);
        anchor_spl::associated_token::create(cpi_ctx)?;
        msg!("Ownership NFT ATA created via associated_token::create.");
    
        msg!("Creating Metaplex Metadata for Ownership NFT...");
        let ownership_nft_symbol = OWNERSHIP_NFT_SYMBOL.to_string();
        let ownership_token_name = generate_lottery_token_name(&args.ticket_id);
        let ownership_token_uri = generate_lottery_metadata_uri(&args.ticket_id);
    
        let nft_creators = vec![
            Creator { address: admin.key(), verified: true, share: 100 },
        ];
    
        let mut create_v1_builder = CreateV1Builder::new();
        create_v1_builder
            .metadata(ownership_nft_metadata.key())
            .master_edition(Some(ownership_nft_master_edition.key()))
            .mint(ownership_nft_mint.key(), false)
            .authority(admin.key())
            .payer(payer.key())
            .update_authority(update_authority.key(), true)
            .system_program(system_program.key())
            .sysvar_instructions(solana_program::sysvar::instructions::ID)
            .spl_token_program(Some(ctx.accounts.spl_token_program.key()))
            .name(ownership_token_name.clone())
            .symbol(ownership_nft_symbol.clone())
            .uri(ownership_token_uri.clone())
            .seller_fee_basis_points(0)
            .creators(nft_creators)
            .primary_sale_happened(false)
            .is_mutable(true)
            .token_standard(TokenStandard::NonFungible)
            .collection_details(CollectionDetails::V1 { size: 0 })
            .decimals(0);
        let create_v1_ix = create_v1_builder.instruction();

        anchor_lang::solana_program::program::invoke_signed(
            &create_v1_ix,
            &[
                ownership_nft_metadata.to_account_info(),
                ownership_nft_master_edition.to_account_info(),
                ownership_nft_mint.to_account_info(),
                admin.to_account_info(),
                payer.to_account_info(),
                update_authority.to_account_info(),
                system_program.to_account_info(),
                ctx.accounts.spl_token_program.to_account_info(),
                token_metadata_program.to_account_info(),
                ctx.accounts.rent.to_account_info(),
                ctx.accounts.associated_token_program.to_account_info(),
                ctx.accounts.token_program.to_account_info(),
                ctx.accounts.ownership_nft_token_account.to_account_info(),
                ctx.accounts.ownership_nft_mint.to_account_info(),
                ctx.accounts.ownership_nft_metadata.to_account_info(),
                ctx.accounts.ownership_nft_master_edition.to_account_info(),
                ctx.accounts.update_authority.to_account_info(),
                ctx.accounts.admin.to_account_info(),
                ctx.accounts.payer.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
                ctx.accounts.rent.to_account_info(),
                ctx.accounts.token_metadata_program.to_account_info(),
                ctx.accounts.spl_token_program.to_account_info(),
            ],
            &[],
        )?;
    
        Ok(())
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct InitLotteryTokenArgs {
    pub ticket_id: [u8; 16],
}

#[derive(Accounts)]
#[instruction(args: InitLotteryTokenArgs)]
pub struct InitOwnershipNft<'info> {
    /// CHECK: Mint account PDA for the Ownership NFT
    #[account(mut)]
    pub ownership_nft_mint: UncheckedAccount<'info>,

    // #[account(
    //     mut,
    //     seeds = [b"metadata", TOKEN_METADATA_PROGRAM_ID.as_ref(), ownership_nft_mint.key().as_ref()],
    //     bump,
    // )]
    /// CHECK: Metadata account PDA for the Ownership NFT. Checked via CPI.
    #[account(
        mut
    )]
    pub ownership_nft_metadata: UncheckedAccount<'info>,

    /// CHECK: Master Edition account PDA for the Ownership NFT. Checked via CPI.
    #[account(mut)]
    pub ownership_nft_master_edition: UncheckedAccount<'info>,

    /// CHECK: Associated Token Account for the Payer to receive the Ownership NFT.
    #[account(mut)]
    pub ownership_nft_token_account: UncheckedAccount<'info>,

    /// Funding account for token creation
    #[account(mut)]
    pub payer: Signer<'info>,

    /// Admin signer with authority for mint & metadata update
    #[account(
        signer,
        constraint = admin.key() == ADMIN_TICKET_VALIDATOR @ ErrorCode::InvalidServerSigner
    )]
    pub admin: Signer<'info>,

    /// Authority signer specifically for the update_authority field in metadata
    #[account(
        signer,
        constraint = update_authority.key() == admin.key() @ ErrorCode::InvalidServerSigner
    )]
    pub update_authority: Signer<'info>,

    /// System program
    pub system_program: Program<'info, System>,

    /// Token program (Token-2022)
    pub token_program: Program<'info, Token2022>,

    /// CHECK: CPI call
    #[account(
        address = MPL_TOKEN_METADATA_PROGRAM_ID @ ErrorCode::InvalidProgram
    )]
    pub token_metadata_program: AccountInfo<'info>,

    /// Solana Rent program
    pub rent: Sysvar<'info, Rent>,

    /// Associated Token program
    pub associated_token_program: Program<'info, AssociatedToken>,

    /// --- ADDED: Standard token program for CPI to Metaplex ---
    pub spl_token_program: Program<'info, Token>,
}


#[derive(Accounts)]
pub struct Initialize {}

fn generate_lottery_metadata_uri(
    ticket_id_bytes: &[u8; 16]
) -> String {
    let ticket_id_str = array_to_uuid_string(ticket_id_bytes);
    
    format!(
        "{}/api/metadata/test/{}", 
        BASE_METADATA_URL,
        ticket_id_str
    )
}

fn array_to_uuid_string(uuid_bytes: &[u8; 16]) -> String {
    let mut uuid_string = String::with_capacity(36);
    
    for (i, byte) in uuid_bytes.iter().enumerate() {
        if i == 4 || i == 6 || i == 8 || i == 10 {
            uuid_string.push('-');
        }
        
        write!(uuid_string, "{:02x}", byte).unwrap();
    }
    
    uuid_string
}

fn generate_lottery_token_name(uuid_bytes: &[u8; 16]) -> String {
    format!("Test #{}", array_to_uuid_string(uuid_bytes))
}
