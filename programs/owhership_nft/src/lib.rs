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

    pub fn init_ownership_nft(
        ctx: Context<InitOwnershipNft>,
        args: InitLotteryTokenArgs,
    ) -> Result<()> {
        let payer = &ctx.accounts.payer;
        let admin = &ctx.accounts.admin;
        let update_authority = &ctx.accounts.update_authority;
        let token_program = &ctx.accounts.token_program;
        let system_program = &ctx.accounts.system_program;
        let rent = &ctx.accounts.rent;
        let ownership_nft_mint = &ctx.accounts.ownership_nft_mint;
        let ownership_nft_metadata = &ctx.accounts.ownership_nft_metadata;
        let ownership_nft_master_edition = &ctx.accounts.ownership_nft_master_edition;
        let ownership_nft_token_account = &ctx.accounts.ownership_nft_token_account;
    
        let ticket_id_bytes = args.ticket_id.as_ref();

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

        let (expected_nft_mint_pda, nft_mint_bump) = Pubkey::find_program_address(
            &[b"lottery_nft_mint", ticket_id_bytes],
            ctx.program_id
        );
        require_keys_eq!(ownership_nft_mint.key(), expected_nft_mint_pda, ErrorCode::InvalidProgram);
        let nft_mint_bump_bytes = [nft_mint_bump];
        let nft_mint_seeds = &[b"lottery_nft_mint".as_ref(), ticket_id_bytes, &nft_mint_bump_bytes[..]][..];
        let nft_mint_signer_seeds = &[nft_mint_seeds];
        msg!("[init_ownership_nft]:2");
        create_ownership_nft_mint_account(
            payer,
            ownership_nft_mint,
            system_program,
            token_program,
            nft_mint_signer_seeds,
        )?;
        msg!("[init_ownership_nft]:3");
        initialize_metadata_and_group_pointer(
            token_program,
            ownership_nft_mint,
            admin,
            ownership_nft_metadata,
            nft_mint_signer_seeds,
        )?;
        msg!("[init_ownership_nft]:4");
        initialize_ownership_nft_mint_data(
            token_program,
            ownership_nft_mint,
            rent,
            admin,
            nft_mint_signer_seeds,
        )?;
        msg!("[init_ownership_nft]:5");
        create_ownership_nft_ata(
            payer,
            ownership_nft_token_account,
            ownership_nft_mint,
            system_program,
            token_program,
            &ctx.accounts.associated_token_program,
        )?;
        msg!("[init_ownership_nft]:6");
        
        let ownership_nft_symbol = OWNERSHIP_NFT_SYMBOL.to_string();
        let ownership_token_name = generate_lottery_token_name(&args.ticket_id);
        let ownership_token_uri = generate_lottery_metadata_uri(&args.ticket_id);
        
        msg!("[init_ownership_nft]:7");
        let create_v1_ix = build_create_v1_instruction(
            ownership_nft_metadata.key(),
            ownership_nft_master_edition.key(),
            ownership_nft_mint.key(),
            admin.key(),
            payer.key(),
            update_authority.key(),
            system_program.key(),
            ctx.accounts.instructions.key(),
            ctx.accounts.spl_token_program.key(),
            ownership_token_name,
            ownership_nft_symbol,
            ownership_token_uri,
        );
        msg!("[init_ownership_nft]:8");
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
                ctx.accounts.instructions.to_account_info(),
                ctx.accounts.spl_token_program.to_account_info(),
            ],
            &[],
        )?;
        msg!("[init_ownership_nft]:9");
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

    /// CHECK: Sysvar instructions account (обязателен для Metaplex CPI)
    #[account(address = solana_program::sysvar::instructions::ID)]
    pub instructions: AccountInfo<'info>,
}

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

fn create_ownership_nft_mint_account<'info>(
    payer: &Signer<'info>,
    ownership_nft_mint: &UncheckedAccount<'info>,
    system_program: &Program<'info, System>,
    token_program: &Program<'info, Token2022>,
    nft_mint_signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    let extensions = [ExtensionType::MetadataPointer, ExtensionType::GroupPointer];
    let space = ExtensionType::try_calculate_account_len::<MintState>(&extensions)?;
    let lamports = Rent::get()?.minimum_balance(space);
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
    Ok(())
}

fn initialize_metadata_and_group_pointer<'info>(
    token_program: &Program<'info, Token2022>,
    ownership_nft_mint: &UncheckedAccount<'info>,
    admin: &Signer<'info>,
    ownership_nft_metadata: &UncheckedAccount<'info>,
    nft_mint_signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    msg!("[initialize_metadata_and_group_pointer]:0");
    let init_nft_meta_ptr_ix = metadata_pointer::instruction::initialize(
        &token_program.key(),
        &ownership_nft_mint.key(),
        Some(admin.key()),
        Some(ownership_nft_metadata.key())
    )?;
    msg!("[initialize_metadata_and_group_pointer]:1");
    anchor_lang::solana_program::program::invoke_signed(
        &init_nft_meta_ptr_ix,
        &[
            ownership_nft_mint.to_account_info(),
            admin.to_account_info(),
        ],
        nft_mint_signer_seeds,
    )?;
    msg!("[initialize_metadata_and_group_pointer]:2");
    let init_group_ptr_ix = group_pointer::instruction::initialize(
        &token_program.key(),
        &ownership_nft_mint.key(),
        Some(admin.key()),
        Some(ownership_nft_mint.key())
    )?;
    msg!("[initialize_metadata_and_group_pointer]:3");
    anchor_lang::solana_program::program::invoke_signed(
        &init_group_ptr_ix,
        &[
            ownership_nft_mint.to_account_info(),
            admin.to_account_info(),
        ],
        nft_mint_signer_seeds,
    )?;
    msg!("[initialize_metadata_and_group_pointer]:4");
    Ok(())
}

fn initialize_ownership_nft_mint_data<'info>(
    token_program: &Program<'info, Token2022>,
    ownership_nft_mint: &UncheckedAccount<'info>,
    rent: &Sysvar<'info, Rent>,
    admin: &Signer<'info>,
    nft_mint_signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    msg!("[initialize_ownership_nft_mint_data]:0");
    let init_mint_nft_accounts = InitializeMint {
        mint: ownership_nft_mint.to_account_info(),
        rent: rent.to_account_info(),
    };
    let init_mint_nft_ctx = CpiContext::new_with_signer(
        token_program.to_account_info(),
        init_mint_nft_accounts,
        nft_mint_signer_seeds,
    );
    msg!("[initialize_ownership_nft_mint_data]:1");
    token_2022_program::initialize_mint(
        init_mint_nft_ctx,
        0,
        &admin.key(),
        Some(&admin.key()),
    )?;
    msg!("[initialize_ownership_nft_mint_data]:2");
    Ok(())
}

fn create_ownership_nft_ata<'info>(
    payer: &Signer<'info>,
    ownership_nft_token_account: &UncheckedAccount<'info>,
    ownership_nft_mint: &UncheckedAccount<'info>,
    system_program: &Program<'info, System>,
    token_program: &Program<'info, Token2022>,
    associated_token_program: &Program<'info, AssociatedToken>,
) -> Result<()> {
    msg!("[create_ownership_nft_ata]:0");
    let cpi_accounts = anchor_spl::associated_token::Create {
        payer: payer.to_account_info(),
        associated_token: ownership_nft_token_account.to_account_info(),
        authority: payer.to_account_info(),
        mint: ownership_nft_mint.to_account_info(),
        system_program: system_program.to_account_info(),
        token_program: token_program.to_account_info(),
    };
    let cpi_ctx = CpiContext::new(associated_token_program.to_account_info(), cpi_accounts);
    anchor_spl::associated_token::create(cpi_ctx)?;
    msg!("[create_ownership_nft_ata]:1");
    Ok(())
}

fn build_create_v1_instruction(
    metadata: Pubkey,
    master_edition: Pubkey,
    mint: Pubkey,
    admin: Pubkey,
    payer: Pubkey,
    update_authority: Pubkey,
    system_program: Pubkey,
    sysvar_instructions: Pubkey,
    spl_token_program: Pubkey,
    name: String,
    symbol: String,
    uri: String,
) -> solana_program::instruction::Instruction {
    msg!("[build_create_v1_instruction]:0");
    let nft_creators = vec![
        Creator { address: admin, verified: true, share: 100 },
    ];
    let mut create_v1_builder = CreateV1Builder::new();
    create_v1_builder
        .metadata(metadata)
        .master_edition(Some(master_edition))
        .mint(mint, false)
        .authority(admin)
        .payer(payer)
        .update_authority(update_authority, true)
        .system_program(system_program)
        .sysvar_instructions(sysvar_instructions)
        .spl_token_program(Some(spl_token_program))
        .name(name)
        .symbol(symbol)
        .uri(uri)
        .seller_fee_basis_points(0)
        .creators(nft_creators)
        .primary_sale_happened(false)
        .is_mutable(true)
        .token_standard(TokenStandard::NonFungible)
        .collection_details(CollectionDetails::V1 { size: 0 })
        .decimals(0);
    msg!("[build_create_v1_instruction]:1");
    create_v1_builder.instruction()
}
