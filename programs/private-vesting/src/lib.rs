#![allow(unexpected_cfgs)]

use anchor_lang::prelude::*;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token::{self, Mint, Token, TokenAccount, TransferChecked};
use pyth_solana_receiver_sdk::price_update::{get_feed_id_from_hex, PriceUpdateV2};

use std::str::FromStr;
declare_id!("8UKRtLxSCUwCwXBTyAt3oqsDBy8gFinr2Cm4C1iPLp8Z");

pub const MAXIMUM_AGE: u64 = 3600; // 1 hour
pub const PYTH_PRICE_UPDATE_ADDRESS: &str = "7UVimffxr9ow1uXYxsr4LHAcV58mLzhmwaeKvJ1pjLiE";
pub const FEED_ID: &str = "0xef0d8b6fda2ceba41da15d4095d1da392a0d2f8ed0c6c7bc0f4cfac8c280b56d";

/**************************************** Change These ==> Start ***********************************************/
// pub const ADMIN_WALLET_ADDRESS: &str = "6ubMCJm3AQNcmUG9gxe44VjhxwqMy8YWHhHbGqFc6jfC";
// pub const MINT_ADDRESS: &str = "EiGin8Xaf3uefW165oxjF23kGzaBKyoHjKNKuGpYYEXX";
// pub const USDT_MINT_ADDRESS: &str = "3xpEnFCpA73fxLQJNYCHrSbaj7R38smTCfzE3eGkykBn"; // Devnet Fake USDT

pub const ADMIN_WALLET_ADDRESS: &str = "2s419ZBoudi2iBG7TfGUsmS1jiX8FXcNJeSN8MBASzDq";
pub const MINT_ADDRESS: &str = "AJB8mJbn3G8uuS1sUvW5zTGwQAmYwW843UQfEd1E6YYA";
pub const USDT_MINT_ADDRESS: &str = "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB"; // Mainnet Tether USDT

/**************************************** Change These ==> End *************************************************/

#[program]
pub mod private_vesting {
    use solana_program::native_token::LAMPORTS_PER_SOL;

    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        require!(
            ctx.accounts.user.key() == Pubkey::from_str(ADMIN_WALLET_ADDRESS).unwrap(),
            ErrorCode::Unauthorized
        );
        msg!("Initialized!");
        Ok(())
    }

    pub fn list_token(ctx: Context<ListToken>) -> Result<()> {
        require!(
            ctx.accounts.user.key() == Pubkey::from_str(ADMIN_WALLET_ADDRESS).unwrap(),
            ErrorCode::Unauthorized
        );
        require!(
            ctx.accounts.vesting.listed_time == 0,
            ErrorCode::Unauthorized
        );
        msg!("Token listed now!");
        ctx.accounts.vesting.listed_time = Clock::get()?.unix_timestamp as u64;
        Ok(())
    }

    pub fn set_vesting(
        ctx: Context<SetVesting>,
        start_time: u64,
        sale_duration: u64,
        vesting_duration_x1: u64,
        amount: u64,
    ) -> Result<()> {
        // Only admin can deposit tokens
        require!(
            ctx.accounts.user.key() == Pubkey::from_str(ADMIN_WALLET_ADDRESS).unwrap(),
            ErrorCode::Unauthorized
        );
        // Check if there's no active vesting or if previous vesting has ended
        let current_time = Clock::get()?.unix_timestamp;
        require!(
            ctx.accounts.vesting.start_time == 0
                || current_time
                    >= ctx.accounts.vesting.start_time + ctx.accounts.vesting.sale_duration as i64,
            // >= ctx.accounts.vesting.start_time
            //     + (ctx.accounts.vesting.sale_duration
            //         + ctx.accounts.vesting.vesting_duration_x1 * 6)
            //         as i64,
            ErrorCode::ActiveVestingExists
        );

        // Set vesting parameters
        ctx.accounts.vesting.start_time = Clock::get()?.unix_timestamp + start_time as i64;
        ctx.accounts.vesting.sale_duration = sale_duration;
        ctx.accounts.vesting.listed_time = 0;
        ctx.accounts.vesting.vesting_duration_x1 = vesting_duration_x1;
        ctx.accounts.vesting.amount = amount;
        ctx.accounts.vesting.claimed_amount = 0;
        // ctx.accounts.vesting.refer_codes = vec![];
        // ctx.accounts.vesting.refer_amounts = vec![];

        // Transfer tokens from admin to vault
        let transfer_cpi_accounts = TransferChecked {
            from: ctx.accounts.admin_ata.clone().to_account_info(),
            to: ctx.accounts.pda_ata.clone().to_account_info(),
            authority: ctx.accounts.user.clone().to_account_info(),
            mint: ctx.accounts.mint.clone().to_account_info(),
        };

        let cpi_ctx = CpiContext::new(
            ctx.accounts.token_program.clone().to_account_info(),
            transfer_cpi_accounts,
        );
        token::transfer_checked(cpi_ctx, amount, ctx.accounts.mint.decimals)?;

        Ok(())
    }

    pub fn return_token(ctx: Context<SetVesting>) -> Result<()> {
        let pda_vesting_account = &mut ctx.accounts.vesting;
        let current_time = Clock::get()?.unix_timestamp;
        require!(
            ctx.accounts.user.key() == Pubkey::from_str(ADMIN_WALLET_ADDRESS).unwrap(),
            ErrorCode::Unauthorized
        );
        // Ensure sale has ended
        require!(
            current_time
                > pda_vesting_account.start_time + pda_vesting_account.sale_duration as i64,
            ErrorCode::SaleNotEnded
        );
        // Transfer tokens from vault to user
        let seeds = &["vesting".as_bytes(), &[ctx.bumps.vesting]];
        let signer = &[&seeds[..]];
        let amount = ctx.accounts.vesting.amount - ctx.accounts.vesting.claimed_amount;
        token::transfer_checked(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.clone().to_account_info(),
                TransferChecked {
                    from: ctx.accounts.pda_ata.clone().to_account_info(),
                    to: ctx.accounts.admin_ata.clone().to_account_info(),
                    mint: ctx.accounts.mint.clone().to_account_info(),
                    authority: ctx.accounts.vesting.clone().to_account_info(),
                },
                signer,
            ),
            amount,
            ctx.accounts.mint.decimals,
        )?;
        ctx.accounts.vesting.claimed_amount = ctx.accounts.vesting.amount;
        Ok(())
    }

    pub fn buy_token(
        ctx: Context<BuyToken>,
        amount: u64,
        // is_investor: bool,
        pay_sol: bool,
        refer_code: u32,
    ) -> Result<()> {
        let from_account = &ctx.accounts.user;
        let pda_vesting_account = &mut ctx.accounts.vesting;
        let current_time = Clock::get()?.unix_timestamp;

        // Check if sale is active
        require!(
            current_time >= pda_vesting_account.start_time,
            ErrorCode::SaleNotStarted
        );

        require!(
            current_time
                <= pda_vesting_account.start_time + pda_vesting_account.sale_duration as i64,
            ErrorCode::SaleEnded
        );

        let referrer_counts = pda_vesting_account.refer_codes.len();
        let mut refer_code = refer_code;
        if ctx.accounts.user_info.refer_code > 0 {
            refer_code = ctx.accounts.user_info.refer_code;
        } else {
            ctx.accounts.user_info.refer_code = refer_code;
        }
        if refer_code > 0 {
            let mut found_code = false;
            for i in 0..referrer_counts {
                let code = pda_vesting_account.refer_codes[i];
                let prev_amount = pda_vesting_account.refer_amounts[i];
                if code == refer_code {
                    pda_vesting_account.refer_amounts[i] = amount + prev_amount;
                    found_code = true;
                    break;
                }
            }
            if !found_code {
                require!(referrer_counts < 100, ErrorCode::CodeCountOverflow);
                pda_vesting_account.refer_codes.push(refer_code);
                pda_vesting_account.refer_amounts.push(amount);
            }
            msg!("refer_code: {}, amount:{}", refer_code, amount);
        }
        let price_in_usd = 0.025 * (if refer_code > 0 { 0.99 } else { 1.0 });
        require!(
            amount <= pda_vesting_account.amount - pda_vesting_account.claimed_amount,
            ErrorCode::AllocationAmountTooLarge
        );
        pda_vesting_account.claimed_amount = pda_vesting_account
            .claimed_amount
            .checked_add(amount)
            .unwrap();
        if pay_sol {
            let price_update = &mut ctx.accounts.price_update;
            let price = price_update.get_price_no_older_than(
                &Clock::get()?,
                MAXIMUM_AGE,
                &get_feed_id_from_hex(FEED_ID)?,
            )?;

            let amount_in_lamports = ((LAMPORTS_PER_SOL as f64)
                * (10_u64.pow(price.exponent.abs().try_into().unwrap()) as f64)
                * ((amount as f64) / 1000000.0)
                * price_in_usd
                / (price.price as f64)) as u64;

            let transfer_instruction = anchor_lang::solana_program::system_instruction::transfer(
                &from_account.key(),
                &ctx.accounts.admin.key(),
                amount_in_lamports,
            );
            anchor_lang::solana_program::program::invoke(
                &transfer_instruction,
                &[
                    from_account.to_account_info(),
                    ctx.accounts.admin.clone().to_account_info(),
                    ctx.accounts.system_program.to_account_info(),
                ],
            )?;
        } else {
            // let usdt_amount = (price_in_usd * 1000000.0 * (amount as f64) / 1000000000.0) as u64;
            let usdt_amount = (price_in_usd * (amount as f64)) as u64;
            let cpi_accounts = TransferChecked {
                from: ctx.accounts.user_usdt_ata.to_account_info().clone(),
                to: ctx.accounts.admin_usdt_ata.to_account_info(),
                authority: ctx.accounts.user.to_account_info(),
                mint: ctx.accounts.usdt_mint.to_account_info(),
            };
            let cpi_program = ctx.accounts.token_program.to_account_info();

            token::transfer_checked(
                CpiContext::new(cpi_program, cpi_accounts),
                usdt_amount,
                ctx.accounts.usdt_mint.decimals,
            )?;
        }
        ctx.accounts.user_info.total_allocation = ctx
            .accounts
            .user_info
            .total_allocation
            .checked_add(amount)
            .unwrap();
        Ok(())
    }

    pub fn give_token(ctx: Context<GiveToken>, amount: u64, refer_code: u32) -> Result<()> {
        let pda_vesting_account = &mut ctx.accounts.vesting;
        let current_time = Clock::get()?.unix_timestamp;
        require!(
            ctx.accounts.admin.key() == Pubkey::from_str(ADMIN_WALLET_ADDRESS).unwrap(),
            ErrorCode::Unauthorized
        );

        // Check if sale is active
        require!(
            current_time >= pda_vesting_account.start_time,
            ErrorCode::SaleNotStarted
        );

        require!(
            current_time
                <= pda_vesting_account.start_time + pda_vesting_account.sale_duration as i64,
            ErrorCode::SaleEnded
        );

        require!(
            amount <= pda_vesting_account.amount - pda_vesting_account.claimed_amount,
            ErrorCode::AllocationAmountTooLarge
        );
        pda_vesting_account.claimed_amount = pda_vesting_account
            .claimed_amount
            .checked_add(amount)
            .unwrap();
        ctx.accounts.user_info.total_allocation = ctx
            .accounts
            .user_info
            .total_allocation
            .checked_add(amount)
            .unwrap();
        let referrer_counts = pda_vesting_account.refer_codes.len();
        let mut refer_code = refer_code;
        if ctx.accounts.user_info.refer_code > 0 {
            refer_code = ctx.accounts.user_info.refer_code;
        } else {
            ctx.accounts.user_info.refer_code = refer_code
        }
        if refer_code > 0 {
            let mut found_code = false;
            for i in 0..referrer_counts {
                let code = pda_vesting_account.refer_codes[i];
                let prev_amount = pda_vesting_account.refer_amounts[i];
                if code == refer_code {
                    pda_vesting_account.refer_amounts[i] = amount + prev_amount;
                    found_code = true;
                    break;
                }
            }
            if !found_code {
                require!(referrer_counts < 100, ErrorCode::CodeCountOverflow);
                pda_vesting_account.refer_codes.push(refer_code);
                pda_vesting_account.refer_amounts.push(amount);
            }
            msg!("refer_code: {}, amount:{}", refer_code, amount);
        }
        Ok(())
    }

    pub fn claim_token(ctx: Context<ClaimToken>, amount: u64) -> Result<()> {
        let current_time = Clock::get()?.unix_timestamp;

        // Ensure Token is listed
        require!(
            ctx.accounts.vesting.listed_time > 0,
            ErrorCode::TokenNotListed
        );
        // Ensure sale has ended
        require!(
            current_time
                >= ctx.accounts.vesting.start_time + ctx.accounts.vesting.sale_duration as i64,
            ErrorCode::SaleNotEnded
        );

        let initial_unlock_rate: f64 = 0.05;
        let vesting_duration_coef: u64;
        let cliff_duration_coef: u64 = 0;
        if ctx.accounts.user_info.total_allocation / 1_000_000 < 500_000 {
            vesting_duration_coef = 2;
        } else if ctx.accounts.user_info.total_allocation / 1_000_000 < 1_000_000 {
            vesting_duration_coef = 3;
        } else {
            vesting_duration_coef = 6;
        };
        let vesting_duration: u64 =
            ctx.accounts.vesting.vesting_duration_x1 * vesting_duration_coef;
        let cliff_duration: u64 = ctx.accounts.vesting.vesting_duration_x1 * cliff_duration_coef;
        // Calculate initial 15% ? allocation
        let initial_allocation =
            (ctx.accounts.user_info.total_allocation as f64 * initial_unlock_rate) as u64;
        // Calculate vesting amount
        let time_since_listed = (current_time - (ctx.accounts.vesting.listed_time as i64)) as u64;
        let vesting_amount = if time_since_listed >= vesting_duration {
            // If vesting period is complete, allow claiming full amount
            ctx.accounts.user_info.total_allocation
        } else {
            // Linear vesting for remaining 85% ? over 6? months
            let remaining_allocation = (ctx.accounts.user_info.total_allocation as f64
                * (1.0 - initial_unlock_rate)) as u64;
            let vested_portion = if time_since_listed < cliff_duration {
                0
            } else {
                ((remaining_allocation as f64)
                    * (time_since_listed as f64 / vesting_duration as f64)) as u64
            };
            initial_allocation.checked_add(vested_portion).unwrap()
        };

        // Check if requested amount is available
        let available_to_claim = vesting_amount
            .checked_sub(ctx.accounts.user_info.claimed_amount)
            .unwrap();
        require!(
            amount <= available_to_claim,
            ErrorCode::AllocationAmountTooLarge
        );

        ctx.accounts.user_info.claimed_amount = ctx
            .accounts
            .user_info
            .claimed_amount
            .checked_add(amount)
            .unwrap();

        // Transfer tokens from vault to user
        let seeds = &["vesting".as_bytes(), &[ctx.bumps.vesting]];
        let signer = &[&seeds[..]];

        token::transfer_checked(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.clone().to_account_info(),
                TransferChecked {
                    from: ctx.accounts.pda_ata.clone().to_account_info(),
                    to: ctx.accounts.user_ata.clone().to_account_info(),
                    mint: ctx.accounts.mint.clone().to_account_info(),
                    authority: ctx.accounts.vesting.clone().to_account_info(),
                },
                signer,
            ),
            amount,
            ctx.accounts.mint.decimals,
        )?;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    #[account(
        init,
        payer = user,
        space = 20 + Vesting::INIT_SPACE,
        seeds = [b"vesting"],
        bump
    )]
    pub vesting: Account<'info, Vesting>,
    #[account(
        init,
        payer = user,
        associated_token::mint = mint,
        associated_token::authority = vesting,
    )]
    pub pda_ata: Account<'info, TokenAccount>,
    #[account(address = Pubkey::from_str(MINT_ADDRESS).unwrap())]
    pub mint: Account<'info, Mint>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}
#[derive(Accounts)]
pub struct ListToken<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    #[account(
        mut,
        seeds = [b"vesting"],
        bump
    )]
    pub vesting: Account<'info, Vesting>,
    pub system_program: Program<'info, System>,
}
#[derive(Accounts)]
pub struct SetVesting<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    #[account(
        mut,
        seeds = [b"vesting"],
        bump
    )]
    pub vesting: Account<'info, Vesting>,
    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = user,
    )]
    pub admin_ata: Account<'info, TokenAccount>,
    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = vesting,
    )]
    pub pda_ata: Account<'info, TokenAccount>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    #[account(address = Pubkey::from_str(MINT_ADDRESS).unwrap())]
    pub mint: Account<'info, Mint>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct BuyToken<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    #[account(
        mut,
        seeds = [b"vesting"],
        bump
    )]
    pub vesting: Account<'info, Vesting>,
    #[account(mut, address = Pubkey::from_str(ADMIN_WALLET_ADDRESS).unwrap())]
    /// CHECK: This is not dangerous because we don't read or write from this account
    pub admin: AccountInfo<'info>,
    #[account(
        init_if_needed,
        payer = user,
        space = 8 + 8 * 3 + 1,
        seeds = [b"user_info", user.key().as_ref()],
        bump
    )]
    pub user_info: Account<'info, UserInfo>,
    #[account(address = Pubkey::from_str(USDT_MINT_ADDRESS).unwrap())]
    pub usdt_mint: Account<'info, Mint>,
    #[account(
        mut,
        associated_token::mint = usdt_mint,
        associated_token::authority = admin,
    )]
    pub admin_usdt_ata: Account<'info, TokenAccount>,
    #[account(
        init_if_needed,
        payer = user,
        associated_token::mint = usdt_mint,
        associated_token::authority = user,
    )]
    pub user_usdt_ata: Account<'info, TokenAccount>,
    pub system_program: Program<'info, System>,
    #[account(address = Pubkey::from_str(PYTH_PRICE_UPDATE_ADDRESS).unwrap())]
    pub price_update: Account<'info, PriceUpdateV2>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
}
#[derive(Accounts)]
pub struct GiveToken<'info> {
    /// CHECK: This is not dangerous because we don't read or write from this account
    #[account(mut)]
    pub user: AccountInfo<'info>,
    #[account(
        mut,
        seeds = [b"vesting"],
        bump
    )]
    pub vesting: Account<'info, Vesting>,
    #[account(mut)]
    /// CHECK: This is not dangerous because we don't read or write from this account
    pub admin: Signer<'info>,
    #[account(
        init_if_needed,
        payer = admin,
        space = 8 + 8 * 3 + 1,
        seeds = [b"user_info", user.key().as_ref()],
        bump
    )]
    pub user_info: Account<'info, UserInfo>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ClaimToken<'info> {
    #[account(address = Pubkey::from_str(MINT_ADDRESS).unwrap())]
    pub mint: Account<'info, Mint>,
    #[account(mut)]
    pub user: Signer<'info>,
    #[account(
        mut,
        seeds = [b"vesting"],
        bump
    )]
    pub vesting: Account<'info, Vesting>,
    #[account(
        mut,
        seeds = [b"user_info", user.key().as_ref()],
        bump
    )]
    pub user_info: Account<'info, UserInfo>,
    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = vesting,
    )]
    pub pda_ata: Account<'info, TokenAccount>,
    #[account(
        init_if_needed,
        payer = user,
        associated_token::mint = mint,
        associated_token::authority = user,
    )]
    pub user_ata: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

#[account]
#[derive(InitSpace)]
pub struct Vesting {
    pub start_time: i64,
    pub sale_duration: u64,
    pub listed_time: u64,
    pub vesting_duration_x1: u64,
    pub amount: u64,
    pub claimed_amount: u64,
    #[max_len(100)]
    pub refer_codes: Vec<u32>,
    #[max_len(100)]
    pub refer_amounts: Vec<u64>,
}

#[account]
#[derive(InitSpace)]
pub struct UserInfo {
    pub total_allocation: u64,
    pub claimed_amount: u64,
    pub refer_code: u32,
}

#[error_code]
pub enum ErrorCode {
    #[msg("Unauthorized")]
    Unauthorized,
    #[msg("Active vesting period exists")]
    ActiveVestingExists,
    #[msg("Allocation amount too large")]
    AllocationAmountTooLarge,
    #[msg("Sale not started")]
    SaleNotStarted,
    #[msg("Sale not ended")]
    SaleNotEnded,
    #[msg("Sale has ended")]
    SaleEnded,
    #[msg("Exceeds total vesting amount")]
    ExceedsVestingAmount,
    #[msg("Code count overflow")]
    CodeCountOverflow,
    #[msg("Token is not listed yet")]
    TokenNotListed,
}
