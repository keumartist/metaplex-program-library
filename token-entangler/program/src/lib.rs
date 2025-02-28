pub mod utils;

use crate::utils::*;
use anchor_lang::{
    prelude::*,
    solana_program::program::{invoke, invoke_signed},
    AnchorDeserialize, AnchorSerialize,
};
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{Mint, Token, TokenAccount},
};

anchor_lang::declare_id!("qntmGodpGkrM42mN68VCZHXnKqDCT8rdY23wFcXCLPd");

const PREFIX: &str = "token_entangler";
const ESCROW: &str = "escrow";
const A_NAME: &str = "A";
const B_NAME: &str = "B";
#[program]
pub mod token_entangler {
    use spl_token::amount_to_ui_amount;

    use super::*;

    pub fn create_entangled_pair<'info>(
        ctx: Context<'_, '_, '_, 'info, CreateEntangledPair<'info>>,
        bump: u8,
        _reverse_bump: u8,
        token_a_escrow_bump: u8,
        token_b_escrow_bump: u8,
        price: u64,
        pays_every_time: bool,
    ) -> Result<()> {
        let treasury_mint = &ctx.accounts.treasury_mint;
        let payer = &ctx.accounts.payer;
        let transfer_authority = &ctx.accounts.transfer_authority;
        let authority = &ctx.accounts.authority;
        let mint_a = &ctx.accounts.mint_a;
        let metadata_a = &ctx.accounts.metadata_a;
        let edition_a = &ctx.accounts.edition_a;
        let mint_b = &ctx.accounts.mint_b;
        let metadata_b = &ctx.accounts.metadata_b;
        let edition_b = &ctx.accounts.edition_b;
        let token_a_escrow = &ctx.accounts.token_a_escrow;
        let token_b_escrow = &ctx.accounts.token_b_escrow;
        let token_b = &ctx.accounts.token_b;
        let entangled_pair = &mut ctx.accounts.entangled_pair;
        let reverse_entangled_pair = &ctx.accounts.reverse_entangled_pair;
        let token_program = &ctx.accounts.token_program;
        let system_program = &ctx.accounts.system_program;
        let rent = &ctx.accounts.rent;

        if !reverse_entangled_pair.data_is_empty() {
            return Err(ErrorCode::EntangledPairExists.into());
        }

        entangled_pair.bump = bump;
        entangled_pair.token_a_escrow_bump = token_a_escrow_bump;
        entangled_pair.token_b_escrow_bump = token_b_escrow_bump;
        entangled_pair.price = price;
        entangled_pair.pays_every_time = pays_every_time;
        entangled_pair.authority = authority.key();
        entangled_pair.mint_b = mint_b.key();
        entangled_pair.token_a_escrow = token_a_escrow.key();
        entangled_pair.token_b_escrow = token_b_escrow.key();
        entangled_pair.treasury_mint = treasury_mint.key();
        entangled_pair.mint_a = mint_a.key();

        let edition_option_a = if edition_a.data_len() > 0 {
            Some(edition_a)
        } else {
            None
        };

        let edition_option_b = if edition_b.data_len() > 0 {
            Some(edition_b)
        } else {
            None
        };

        let (mint_a_supply, mint_a_decimals) = get_mint_details(&mint_a.to_account_info())?;
        let mint_a_ui_supply = amount_to_ui_amount(mint_a_supply, mint_a_decimals);
        require!(
            mint_a_supply == 1 || mint_a_ui_supply == 1.0,
            ErrorCode::MustHaveSupplyOne
        );

        let (mint_b_supply, mint_b_decimals) = get_mint_details(&mint_b.to_account_info())?;
        let mint_b_ui_supply = amount_to_ui_amount(mint_b_supply, mint_b_decimals);
        require!(
            mint_b_supply == 1 || mint_b_ui_supply == 1.0,
            ErrorCode::MustHaveSupplyOne
        );

        assert_metadata_valid(metadata_a, edition_option_a, &mint_a.key())?;
        assert_metadata_valid(metadata_b, edition_option_b, &mint_b.key())?;

        assert_is_ata(&token_b.to_account_info(), &payer.key(), &mint_b.key())?;

        let mint_a_key = mint_a.key();
        let mint_b_key = mint_b.key();
        let token_a_escrow_seeds = [
            PREFIX.as_bytes(),
            &mint_a_key.as_ref(),
            &mint_b_key.as_ref(),
            ESCROW.as_bytes(),
            A_NAME.as_bytes(),
            &[token_a_escrow_bump],
        ];
        let token_b_escrow_seeds = [
            PREFIX.as_bytes(),
            &mint_a_key.as_ref(),
            &mint_b_key.as_ref(),
            ESCROW.as_bytes(),
            B_NAME.as_bytes(),
            &[token_b_escrow_bump],
        ];

        create_program_token_account_if_not_present(
            token_a_escrow,
            system_program,
            &payer,
            token_program,
            &mint_a.to_account_info(),
            &entangled_pair.to_account_info(),
            rent,
            &token_a_escrow_seeds,
            &[],
        )?;

        create_program_token_account_if_not_present(
            token_b_escrow,
            system_program,
            &payer,
            token_program,
            &mint_b.to_account_info(),
            &entangled_pair.to_account_info(),
            rent,
            &token_b_escrow_seeds,
            &[],
        )?;

        invoke(
            &spl_token::instruction::transfer(
                token_program.key,
                &token_b.key(),
                &token_b_escrow.key(),
                &transfer_authority.key(),
                &[],
                mint_b_supply,
            )?,
            &[
                token_b.to_account_info(),
                token_b_escrow.to_account_info(),
                token_program.to_account_info(),
                transfer_authority.to_account_info(),
            ],
        )?;
        Ok(())
    }

    pub fn update_entangled_pair<'info>(
        ctx: Context<'_, '_, '_, 'info, UpdateEntangledPair<'info>>,
        price: u64,
        pays_every_time: bool,
    ) -> Result<()> {
        let new_authority = &ctx.accounts.new_authority;
        let entangled_pair = &mut ctx.accounts.entangled_pair;

        entangled_pair.authority = new_authority.key();
        entangled_pair.pays_every_time = pays_every_time;
        entangled_pair.price = price;
        Ok(())
    }

    pub fn swap<'info>(ctx: Context<'_, '_, '_, 'info, Swap<'info>>) -> Result<()> {
        let treasury_mint = &ctx.accounts.treasury_mint;
        let payer = &ctx.accounts.payer;
        let payment_account = &ctx.accounts.payment_account;
        let payment_transfer_authority = &ctx.accounts.payment_transfer_authority;
        let token = &ctx.accounts.token;
        let token_mint = &ctx.accounts.token_mint;
        let replacement_token_metadata = &ctx.accounts.replacement_token_metadata;
        let replacement_token = &ctx.accounts.replacement_token;
        let replacement_token_mint = &ctx.accounts.replacement_token_mint;
        let transfer_authority = &ctx.accounts.transfer_authority;
        let token_a_escrow = &ctx.accounts.token_a_escrow;
        let token_b_escrow = &ctx.accounts.token_b_escrow;
        let entangled_pair = &mut ctx.accounts.entangled_pair;
        let token_program = &ctx.accounts.token_program;
        let system_program = &ctx.accounts.system_program;
        let ata_program = &ctx.accounts.ata_program;
        let rent = &ctx.accounts.rent;

        require!(token.mint == token_mint.key(), ErrorCode::InvalidMint);
        let token_mint_supply = token_mint.supply;
        if token.amount != token_mint_supply {
            return Err(ErrorCode::InvalidTokenAmount.into());
        }

        if replacement_token.data_is_empty() {
            make_ata(
                replacement_token.to_account_info(),
                payer.to_account_info(),
                replacement_token_mint.to_account_info(),
                payer.to_account_info(),
                ata_program.to_account_info(),
                token_program.to_account_info(),
                system_program.to_account_info(),
                rent.to_account_info(),
                &[],
            )?;
        }

        assert_is_ata(
            &replacement_token.to_account_info(),
            &payer.key(),
            &replacement_token_mint.key(),
        )?;

        let signer_seeds = [
            PREFIX.as_bytes(),
            &entangled_pair.mint_a.as_ref(),
            &entangled_pair.mint_b.as_ref(),
            &[entangled_pair.bump],
        ];

        let swap_from_escrow;
        let swap_to_escrow;
        if token.mint == entangled_pair.mint_a {
            swap_from_escrow = token_a_escrow;
            swap_to_escrow = token_b_escrow;
            assert_metadata_valid(replacement_token_metadata, None, &entangled_pair.mint_b)?;
        } else if token.mint == entangled_pair.mint_b {
            swap_from_escrow = token_b_escrow;
            swap_to_escrow = token_a_escrow;
            assert_metadata_valid(replacement_token_metadata, None, &entangled_pair.mint_a)?;
        } else {
            return Err(ErrorCode::InvalidMint.into());
        }

        if replacement_token_mint.key() != entangled_pair.mint_a
            && replacement_token_mint.key() != entangled_pair.mint_b
        {
            return Err(ErrorCode::InvalidMint.into());
        }

        invoke(
            &spl_token::instruction::transfer(
                token_program.key,
                &token.key(),
                &swap_from_escrow.key(),
                &transfer_authority.key(),
                &[],
                token_mint_supply,
            )?,
            &[
                token.to_account_info(),
                swap_from_escrow.to_account_info(),
                token_program.to_account_info(),
                transfer_authority.to_account_info(),
            ],
        )?;

        let (replacement_token_mint_supply, _) =
            get_mint_details(&replacement_token_mint.to_account_info())?;
        invoke_signed(
            &spl_token::instruction::transfer(
                token_program.key,
                &swap_to_escrow.key(),
                &replacement_token.key(),
                &entangled_pair.key(),
                &[],
                replacement_token_mint_supply,
            )?,
            &[
                swap_to_escrow.to_account_info(),
                replacement_token.to_account_info(),
                token_program.to_account_info(),
                entangled_pair.to_account_info(),
            ],
            &[&signer_seeds],
        )?;

        let is_native = treasury_mint.key() == spl_token::native_mint::id();

        if !entangled_pair.paid || entangled_pair.pays_every_time {
            pay_creator_fees(
                &mut ctx.remaining_accounts.iter(),
                &replacement_token_metadata,
                &payment_account,
                &payment_transfer_authority,
                &payer,
                &treasury_mint.to_account_info(),
                &ata_program.to_account_info(),
                &token_program.to_account_info(),
                &system_program.to_account_info(),
                &rent.to_account_info(),
                entangled_pair.price,
                is_native,
            )?;
        }
        entangled_pair.paid = true;
        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(reverse_bump: u8, token_a_escrow_bump: u8, token_b_escrow_bump: u8)]
pub struct CreateEntangledPair<'info> {
    treasury_mint: Box<Account<'info, Mint>>,
    #[account(mut)]
    payer: Signer<'info>,
    transfer_authority: Signer<'info>,
    /// CHECK: Verified through CPI
    authority: UncheckedAccount<'info>,
    mint_a: Box<Account<'info, Mint>>,
    /// CHECK: Verified through CPI
    metadata_a: UncheckedAccount<'info>,
    /// CHECK: Verified through CPI
    edition_a: UncheckedAccount<'info>,
    mint_b: Box<Account<'info, Mint>>,
    /// CHECK: Verified through CPI
    metadata_b: UncheckedAccount<'info>,
    /// CHECK: Verified through CPI
    edition_b: UncheckedAccount<'info>,
    #[account(mut)]
    token_b: Box<Account<'info, TokenAccount>>,
    /// CHECK: Verified through CPI
    #[account(mut,seeds=[PREFIX.as_bytes(), mint_a.key().as_ref(), mint_b.key().as_ref(), ESCROW.as_bytes(), A_NAME.as_bytes()], bump=token_a_escrow_bump)]
    token_a_escrow: UncheckedAccount<'info>,
    /// CHECK: Not dangerous. Account seeds checked in constraint.
    #[account(mut,seeds=[PREFIX.as_bytes(), mint_a.key().as_ref(), mint_b.key().as_ref(), ESCROW.as_bytes(), B_NAME.as_bytes()], bump=token_b_escrow_bump)]
    token_b_escrow: UncheckedAccount<'info>,
    #[account(init, seeds=[PREFIX.as_bytes(), mint_a.key().as_ref(), mint_b.key().as_ref()], bump, space=ENTANGLED_PAIR_SIZE, payer=payer)]
    entangled_pair: Box<Account<'info, EntangledPair>>,
    /// CHECK: Not dangerous. Account seeds checked in constraint.
    #[account(mut, seeds=[PREFIX.as_bytes(), mint_b.key().as_ref(), mint_a.key().as_ref()], bump=reverse_bump)]
    reverse_entangled_pair: UncheckedAccount<'info>,
    token_program: Program<'info, Token>,
    system_program: Program<'info, System>,
    rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct UpdateEntangledPair<'info> {
    authority: Signer<'info>,
    /// CHECK: Verified through CPI
    new_authority: UncheckedAccount<'info>,
    #[account(mut, seeds=[PREFIX.as_bytes(), entangled_pair.mint_a.as_ref(), entangled_pair.mint_b.as_ref()], bump=entangled_pair.bump, has_one=authority)]
    entangled_pair: Account<'info, EntangledPair>,
}

#[derive(Accounts)]
pub struct Swap<'info> {
    treasury_mint: Box<Account<'info, Mint>>,
    payer: Signer<'info>,
    /// CHECK: Verified through CPI
    #[account(mut)]
    payment_account: UncheckedAccount<'info>,
    /// CHECK: Verified through CPI
    payment_transfer_authority: UncheckedAccount<'info>,
    #[account(mut)]
    token: Account<'info, TokenAccount>,
    token_mint: Box<Account<'info, Mint>>,
    /// CHECK: Verified through CPI
    replacement_token_metadata: UncheckedAccount<'info>,
    replacement_token_mint: Box<Account<'info, Mint>>,
    /// CHECK: Verified through CPI
    #[account(mut)]
    replacement_token: UncheckedAccount<'info>,
    transfer_authority: Signer<'info>,
    /// CHECK: Not dangerous. Account seeds checked in constraint.
    #[account(mut,seeds=[PREFIX.as_bytes(), entangled_pair.mint_a.as_ref(), entangled_pair.mint_b.as_ref(), ESCROW.as_bytes(), A_NAME.as_bytes()], bump=entangled_pair.token_a_escrow_bump)]
    token_a_escrow: UncheckedAccount<'info>,
    /// CHECK: Not dangerous. Account seeds checked in constraint.
    #[account(mut,seeds=[PREFIX.as_bytes(), entangled_pair.mint_a.as_ref(), entangled_pair.mint_b.as_ref(), ESCROW.as_bytes(), B_NAME.as_bytes()], bump=entangled_pair.token_b_escrow_bump)]
    token_b_escrow: UncheckedAccount<'info>,
    #[account(mut, seeds=[PREFIX.as_bytes(), entangled_pair.mint_a.as_ref(), entangled_pair.mint_b.as_ref()], bump=entangled_pair.bump, has_one=treasury_mint)]
    entangled_pair: Account<'info, EntangledPair>,
    token_program: Program<'info, Token>,
    system_program: Program<'info, System>,
    ata_program: Program<'info, AssociatedToken>,
    rent: Sysvar<'info, Rent>,
}

pub const ENTANGLED_PAIR_SIZE: usize = 8 +// key 
32 + // treasury mint
32 + // mint a
32 + // mint b
32 + // token a
32 + // token b
32 + // authority
1 + // bump
1 + // token a bump
1 + // token b bump
8 + // price
1 + // paid
200; // padding

#[account]
pub struct EntangledPair {
    pub treasury_mint: Pubkey,
    pub mint_a: Pubkey,
    pub mint_b: Pubkey,
    pub token_a_escrow: Pubkey,
    pub token_b_escrow: Pubkey,
    pub authority: Pubkey,
    pub bump: u8,
    pub token_a_escrow_bump: u8,
    pub token_b_escrow_bump: u8,
    pub price: u64,
    pub paid: bool,
    pub pays_every_time: bool,
}

#[error_code]
pub enum ErrorCode {
    #[msg("PublicKeyMismatch")]
    PublicKeyMismatch,
    #[msg("InvalidMintAuthority")]
    InvalidMintAuthority,
    #[msg("UninitializedAccount")]
    UninitializedAccount,
    #[msg("IncorrectOwner")]
    IncorrectOwner,
    #[msg("PublicKeysShouldBeUnique")]
    PublicKeysShouldBeUnique,
    #[msg("StatementFalse")]
    StatementFalse,
    #[msg("NotRentExempt")]
    NotRentExempt,
    #[msg("NumericalOverflow")]
    NumericalOverflow,
    #[msg("Derived key invalid")]
    DerivedKeyInvalid,
    #[msg("Metadata doesn't exist")]
    MetadataDoesntExist,
    #[msg("Edition doesn't exist")]
    EditionDoesntExist,
    #[msg("Invalid token amount")]
    InvalidTokenAmount,
    #[msg("This token is not a valid mint for this entangled pair")]
    InvalidMint,
    #[msg("This pair already exists as it's reverse")]
    EntangledPairExists,
    #[msg("Must have supply one!")]
    MustHaveSupplyOne,
}
