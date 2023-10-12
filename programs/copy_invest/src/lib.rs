use anchor_lang::{prelude::*, solana_program::program::invoke_signed};
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{mint_to, Mint, MintTo, Token, TokenAccount},
};
use mpl_token_metadata::instruction::create_metadata_accounts_v3;
use pyth_sdk_solana::load_price_feed_from_account_info;
use std::str::FromStr;

declare_id!("6RyrNDYgawQU6e4UxfptWRSZkpnpgMmaXW2N1grkk6ro");

const STALENESS_THRESHOLD: u64 = 60; // seconds

const FUND_NAV_ORIGINAL: u64 = 1000000000; // 1 billion

pub const FUND_MINT_AUTHORITY_SEED: &[u8] = b"mint";
pub const SUPPORTED_ASSETS_PDA_SEED: &[u8] = b"supported_assets";

pub const WSOL_MINT_ID: &str = "So11111111111111111111111111111111111111112";

// solana devnet
pub const SOL_USD_ID: &str = "J83w4HKfqxwcq3BEMMkPFSppX3gqekLyLJBexebFVkix";

#[program]
mod copy_invest {
    use super::*;

    pub fn create_fund(ctx: Context<CreateFund>, metadata: CreateFundParams) -> Result<()> {
        let seeds = &[FUND_MINT_AUTHORITY_SEED, ctx.accounts.payer.key().to_string().as_bytes(),metadata.name.as_bytes(), &[*ctx.bumps.get("mint").unwrap()]];
        let signer = [&seeds[..]];

        let account_info = vec![
            ctx.accounts.metadata.to_account_info(),
            ctx.accounts.mint.to_account_info(),
            ctx.accounts.payer.to_account_info(),
            ctx.accounts.token_metadata_program.to_account_info(),
            ctx.accounts.token_program.to_account_info(),
            ctx.accounts.system_program.to_account_info(),
            ctx.accounts.rent.to_account_info(),
        ];

        invoke_signed(
            &create_metadata_accounts_v3(
                ctx.accounts.token_metadata_program.key(), // token metadata program
                ctx.accounts.metadata.key(),               // metadata account PDA for mint
                ctx.accounts.mint.key(),                   // mint account
                ctx.accounts.mint.key(),                   // mint authority
                ctx.accounts.payer.key(),                  // payer for transaction
                ctx.accounts.mint.key(),                   // update authority
                metadata.name,                             // name
                metadata.symbol,                           // symbol
                metadata.uri,                              // uri (offchain metadata)
                None,                                      // (optional) creators
                0,                                         // seller free basis points
                true,                                      // (bool) update authority is signer
                true,                                      // (bool) is mutable
                None,                                      // (optional) collection
                None,                                      // (optional) uses
                None,                                      // (optional) collection details
            ),
            account_info.as_slice(),
            &signer,
        )?;

        msg!("Fund token mint created successfully.");

        ctx.accounts.support_assets_account.assets = vec![
            Assets {
                mint_pkey: Pubkey::from_str(WSOL_MINT_ID).unwrap(),
                price_feed: Pubkey::from_str(SOL_USD_ID).unwrap(),
            },
        ];

        Ok(())
    }

    pub fn mint_tokens(ctx: Context<MintTokens>, quantity: u64) -> Result<()> {
        let seeds = &[FUND_MINT_AUTHORITY_SEED, &[*ctx.bumps.get("mint").unwrap()]];
        let signer = [&seeds[..]];

        mint_to(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                MintTo {
                    authority: ctx.accounts.mint.to_account_info(),
                    to: ctx.accounts.destination.to_account_info(),
                    mint: ctx.accounts.mint.to_account_info(),
                },
                &signer,
            ),
            quantity,
        )?;

        Ok(())
    }
}


/// Calculate the total value of the assets in the fund
fn cal_assets_value(assets: &Vec<Assets>) -> u64 {
    let mut total_value: u64 = 0;
    for asset in assets {
        let price_account_info = AccountInfo::new_readonly(asset.price_feed, false);
        let price_feed = load_price_feed_from_account_info(price_account_info)?.unwrap();
        let current_timestamp = Clock::get()?.unix_timestamp;
        let current_price = price_feed.get_price_no_older_than(current_timestamp, STALENESS_THRESHOLD).unwrap();

        let display_price = u64::try_from(current_price.price).unwrap() / 10u64.pow(u32::try_from(-current_price.expo).unwrap());
        // let display_confidence = u64::try_from(current_price.conf).unwrap() / 10u64.pow(u32::try_from(-current_price.expo).unwrap());
        total_value += display_price;
    }
    total_value
}

/// Calculate the fund token price
fn get_fund_token_price(fund_mint: &Pubkey,assets: &Vec<Assets>) -> u64 {
    let fund_mint_account_info = AccountInfo::new_readonly(*fund_mint, false);
    let fund_mint = Mint::unpack(&fund_mint_account_info.data.borrow())?;
    let fund_supply = fund_mint.supply;
    if fund_supply == 0 {
        return FUND_NAV_ORIGINAL;
    }
    let assets_value = cal_assets_value(assets);
    let fund_price = assets_value / fund_supply;
    // let fund_price = fund_supply / 10u64.pow(u32::try_from(fund_mint.decimals).unwrap());
    fund_price
}

fn add_asset(ctx: Context<AddAsset>, mint_pkey: Pubkey, price_feed: Pubkey) -> Result<()> {
    let seeds = &[SUPPORTED_ASSETS_PDA_SEED, &[*ctx.bumps.get("supported_assets").unwrap()]];
    let signer = [&seeds[..]];

    let account_info = vec![
        ctx.accounts.support_assets_account.to_account_info(),
        ctx.accounts.payer.to_account_info(),
        ctx.accounts.token_program.to_account_info(),
        ctx.accounts.system_program.to_account_info(),
        ctx.accounts.rent.to_account_info(),
    ];

    ctx.accounts.support_assets_account.assets.push(Assets {
        mint_pkey,
        price_feed,
    });

    Ok(())
}

#[derive(Accounts)]
#[instruction(
    params: CreateFundParams
)]
pub struct CreateFund<'info> {
    /// CHECK: New Metaplex Account being created
    #[account(mut)]
    pub metadata: UncheckedAccount<'info>,
    #[account(
        init,
        seeds = [FUND_MINT_AUTHORITY_SEED, payer.key().as_bytes(), params.name.as_bytes()],
        bump,
        payer = payer,
        mint::decimals = params.decimals,
        mint::authority = mint,
    )]
    pub mint: Account<'info, Mint>,
    #[account(
        init,
        seeds = [SUPPORTED_ASSETS_PDA_SEED, payer.key().as_bytes(), params.name.as_bytes()],
        space = 20000,
        bump,
        payer = payer,
    )]
    pub support_assets_account: Account<'info, SupportedAssets>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub rent: Sysvar<'info, Rent>,
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    /// CHECK: Metaplex program ID
    pub token_metadata_program: UncheckedAccount<'info>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Debug, Clone)]
pub struct CreateFundParams {
    pub name: String,
    pub symbol: String,
    pub uri: String,
    pub decimals: u8,
}

#[account]
pub struct SupportedAssets {
    pub assets: Vec<Assets>,
}

#[account]
pub struct Assets {
    pub mint_pkey: Pubkey,
    pub price_feed: Pubkey,
}

#[derive(Accounts)]
pub struct MintTokens<'info> {
    #[account(
        mut,
        seeds = [FUND_MINT_AUTHORITY_SEED],
        bump,
        mint::authority = mint,
    )]
    pub mint: Account<'info, Mint>,
    #[account(
        init_if_needed,
        payer = payer,
        associated_token::mint = mint,
        associated_token::authority = payer,
    )]
    pub destination: Account<'info, TokenAccount>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub rent: Sysvar<'info, Rent>,
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
}
