#![allow(unexpected_cfgs)]
use spl_token_2022::{ extension::{metadata_pointer, group_pointer, ExtensionType}, state::Mint as MintState};
use anchor_spl::token_2022::{self as token_2022_program, InitializeMint, Token2022};
use spl_token_metadata_interface::instruction as token_metadata_instruction;
use anchor_spl::associated_token::AssociatedToken;
use solana_program::system_instruction;
use anchor_lang::solana_program;
use anchor_lang::prelude::*;
use std::fmt::Write;

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
pub const OWNERSHIP_NFT_SYMBOL: &str = "OWNER-NFT";

#[program]
pub mod owhership_nft {
    use super::*;

    pub fn init_ownership_nft(
        ctx: Context<InitOwnershipNft>,
        args: InitLotteryTokenArgs,
    ) -> Result<()> {
        let payer = &ctx.accounts.payer;
        let admin = &ctx.accounts.admin;
        let token_program = &ctx.accounts.token_program;
        let system_program = &ctx.accounts.system_program;
        let rent = &ctx.accounts.rent;
        let ownership_nft_mint = &ctx.accounts.ownership_nft_mint;
        let ownership_nft_token_account = &ctx.accounts.ownership_nft_token_account;

        let ticket_id_bytes = args.ticket_id.as_ref();

        let (expected_nft_mint_pda, nft_mint_bump) = Pubkey::find_program_address(
            &[b"lottery_nft_mint", ticket_id_bytes],
            ctx.program_id
        );
        require_keys_eq!(ownership_nft_mint.key(), expected_nft_mint_pda, ErrorCode::InvalidProgram);
        let nft_mint_bump_bytes = [nft_mint_bump];
        let nft_mint_seeds = &[b"lottery_nft_mint".as_ref(), ticket_id_bytes, &nft_mint_bump_bytes[..]][..];
        let nft_mint_signer_seeds = &[nft_mint_seeds];

        // Создаём mint с расширениями MetadataPointer и GroupPointer
        create_ownership_nft_mint_account_with_extensions(
            payer,
            ownership_nft_mint,
            &ctx.accounts.ownership_nft_metadata,
            admin,
            system_program,
            token_program,
            nft_mint_signer_seeds,
        )?;

        // Инициализируем mint
        initialize_ownership_nft_mint_data(
            token_program,
            ownership_nft_mint,
            rent,
            admin,
            nft_mint_signer_seeds,
        )?;

        // Создаём ATA
        create_ownership_nft_ata(
            payer,
            ownership_nft_token_account,
            ownership_nft_mint,
            system_program,
            token_program,
            &ctx.accounts.associated_token_program,
        )?;

        // Инициализация метаданных через SPL Token Metadata Interface
        initialize_token2022_metadata(
            ctx.accounts,
            &args.ticket_id,
            nft_mint_signer_seeds,
        )?;

        // Mint 1 токен
        let mint_to_ctx = CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            anchor_spl::token_2022::MintTo {
                mint: ownership_nft_mint.to_account_info(),
                to: ownership_nft_token_account.to_account_info(),
                authority: admin.to_account_info(),
            },
            nft_mint_signer_seeds,
        );
        anchor_spl::token_2022::mint_to(mint_to_ctx, 1)?;

        msg!("NFT успешно создан через Token-2022 без Metaplex!");
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

    /// CHECK: Metadata account for the Ownership NFT (SPL Token-2022 Metadata Extension)
    #[account(mut)]
    pub ownership_nft_metadata: UncheckedAccount<'info>,

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

    /// System program
    pub system_program: Program<'info, System>,

    /// Token program (Token-2022)
    pub token_program: Program<'info, Token2022>,

    /// Solana Rent program
    pub rent: Sysvar<'info, Rent>,

    /// Associated Token program
    pub associated_token_program: Program<'info, AssociatedToken>,
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
    
    for byte in uuid_bytes.iter() {
        write!(uuid_string, "{:02x}", byte).unwrap();
    }
    
    uuid_string
}

fn generate_lottery_token_name(uuid_bytes: &[u8; 16]) -> String {
    array_to_uuid_string(uuid_bytes)
}

fn create_ownership_nft_mint_account_with_extensions<'info>(
    payer: &Signer<'info>,
    ownership_nft_mint: &UncheckedAccount<'info>,
    metadata_account: &UncheckedAccount<'info>,
    admin: &Signer<'info>,
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
    
    // Инициализируем MetadataPointer (метаданные будут указывать на отдельный аккаунт)
    let init_nft_meta_ptr_ix = metadata_pointer::instruction::initialize(
        &token_program.key(),
        &ownership_nft_mint.key(),
        None,
        Some(metadata_account.key())
    )?;
    anchor_lang::solana_program::program::invoke_signed(
        &init_nft_meta_ptr_ix,
        &[
            ownership_nft_mint.to_account_info(),
            admin.to_account_info(),
        ],
        nft_mint_signer_seeds,
    )?;
    
    // Инициализируем GroupPointer (указывает на себя — self-reference)
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
    
    Ok(())
}

fn initialize_ownership_nft_mint_data<'info>(
    token_program: &Program<'info, Token2022>,
    ownership_nft_mint: &UncheckedAccount<'info>,
    rent: &Sysvar<'info, Rent>,
    admin: &Signer<'info>,
    nft_mint_signer_seeds: &[&[&[u8]]],
) -> Result<()> {
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
    Ok(())
}

// Отдельная функция для инициализации метаданных через SPL Token-2022 Metadata Extension
fn initialize_token2022_metadata(
    accounts: &InitOwnershipNft<'_>,
    ticket_id: &[u8; 16],
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    // Генерируем данные для метаданных здесь
    let symbol = OWNERSHIP_NFT_SYMBOL.to_string();
    let name = generate_lottery_token_name(ticket_id);
    let uri = generate_lottery_metadata_uri(ticket_id);

    // По документации SPL Token Metadata Interface:
    // - Для создания инструкции инициализации метаданных требуется:
    // - Метаданные (сам аккаунт, где будут храниться метаданные)
    // - Mint (аккаунт mint, для которого создаются метаданные)
    // - Update Authority (обычно admin, который должен подписывать транзакцию)
    // - Payer (тот, кто оплачивает создание)
    let metadata_account = &accounts.ownership_nft_metadata;
    let mint = &accounts.ownership_nft_mint;
    let update_authority = &accounts.admin; // The admin is the update authority
    let payer = &accounts.payer; // The payer is the one funding the transaction

    // Вызываем функцию initialize с правильными аргументами согласно документации
    // Сигнатура из spl-token-metadata-interface/src/instruction.rs:
    // pub fn initialize(...) -> Instruction { ... }
    // Аргументы: program_id, metadata, update_authority, mint, mint_authority, name, symbol, uri
    let init_metadata_ix = token_metadata_instruction::initialize(
        &token_2022_program::ID,   // program_id: ID программы Token-2022
        &metadata_account.key(),   // metadata: адрес аккаунта метаданных
        &update_authority.key(),   // update_authority: кто может изменять метаданные (admin)
        &mint.key(),               // mint: адрес mint
        &update_authority.key(),   // mint_authority: кто может минтить (admin)
        name,                      // name: имя токена
        symbol,                    // symbol: символ токена
        uri,                       // uri: URI метаданных
    );

    // Вызываем инструкцию с необходимыми аккаунтами
    // Payer подписывает т.к. он платит за транзакцию, update_authority - т.к. он авторизует создание/изменение
    // Аккаунты, которые требуются для инструкции initialize:
    // 0. `[writable]` Metadata account
    // 1. `[signer]` Update authority
    // 2. `[]` Mint account
    // 3. `[signer]` Payer
    anchor_lang::solana_program::program::invoke_signed(
        &init_metadata_ix,
        &[
            metadata_account.to_account_info(), // 0. Metadata account
            update_authority.to_account_info(), // 1. Update authority (signer)
            mint.to_account_info(),             // 2. Mint account
            payer.to_account_info(),            // 3. Payer (signer)
        ],
        signer_seeds, // Используем PDA seeds для подписи от имени mint
    )?;

    msg!("Метаданные успешно инициализированы через SPL Token Metadata Interface");
    Ok(())
}

