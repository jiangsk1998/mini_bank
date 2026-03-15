// ┌──────────────────────────────────────────────────────────────────┐
// │                    迷你银行合约 — 功能需求                         │
// │                                                                    │
// │  指令 1：open_account（开户）                                      │
// │    → 创建 BankAccount PDA                                         │
// │    → 记录：户主、账户名、开户时间                                  │
// │    → 初始余额为 0                                                  │
// │    → 每个用户只能开一个账户（PDA 保证）                            │
// │                                                                    │
// │  指令 2：deposit（存款）                                           │
// │    → 用户向自己的 BankAccount 存入 SOL                             │
// │    → 金额必须大于 0                                                │
// │    → 更新余额                                                      │
// │                                                                    │
// │  指令 3：withdraw（取款）                                          │
// │    → 用户从自己的 BankAccount 取出 SOL                             │
// │    → 余额不足时报错                                                │
// │    → 只有户主才能取款                                              │
// │                                                                    │
// │  指令 4：transfer（转账）                                          │
// │    → 从自己的 BankAccount 转 SOL 到别人的 BankAccount              │
// │    → 不能给自己转账                                                │
// │    → 余额不足时报错                                                │
// │    → 只有转出方户主才需要签名                                      │
// │                                                                    │
// │  指令 5：close_account（销户）                                     │
// │    → 关闭 BankAccount，退还租金                                    │
// │    → 余额必须为 0 才能销户                                         │
// │    → 只有户主才能销户                                              │
// │                                                                    │
// └──────────────────────────────────────────────────────────────────┘

// ==========================================
// 指令6：冻结银行账户
// 需要 admin（管理员）+ owner（账户拥有者）同时签名
// → 防止管理员单方面冻结用户资金
// → 也防止用户自己随意冻结逃避审查
// ==========================================

use anchor_lang::prelude::*;
use anchor_lang::system_program;
use serde::Deserialize;

declare_id!("8SSsAtiS7rprJeJ2LCshyWcp43pcdt6Ki8ijGQesdBtB");

#[program]
pub mod mini_bank {
    use super::*;

    // │  指令 1：open_account（开户）
    // │    → 创建 BankAccount PDA
    // │    → 记录：户主、账户名、开户时间
    // │    → 初始余额为 0
    // │    → 每个用户只能开一个账户（PDA 保证）
    pub fn open_account(ctx: Context<OpenAccount>, name: String) -> Result<()> {
        let account = &mut ctx.accounts.bank_account;

        account.owner = ctx.accounts.signer.key();

        account.bump = ctx.bumps.bank_account;

        account.balance = 0;

        account.name = name;

        account.create_at = Clock::get()?.unix_timestamp;

        msg!("开户成功成功,户主：{}", account.owner);
        Ok(())
    }

    // │  指令 2：deposit（存款）
    // │    → 用户向自己的 BankAccount 存入 SOL
    // │    → 金额必须大于 0
    // │    → 更新余额

    pub fn deposit(ctx: Context<Deposit>, amount: u64) -> Result<()> {
        require!(amount > 0, BankError::InvalidAmount);
        let account = &mut ctx.accounts.bank_account;
        // CPI调用System Program:
        system_program::transfer(
            CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                system_program::Transfer {
                    from: ctx.accounts.signer.to_account_info(),
                    to: ctx.accounts.bank_account.to_account_info(),
                },
            ),
            amount,
        );

        //更新余额
        let bank_account = &mut ctx.accounts.bank_account;
        bank_account.balance += amount;
        msg!(
            "存款成功！金额: {} lamports，余额: {}",
            amount,
            bank_account.balance
        );
        Ok(())
    }

    // │  指令 3：withdraw（取款）
    // │    → 用户从自己的 BankAccount 取出 SOL
    // │    → 余额不足时报错
    // │    → 只有户主才能取款

    pub fn withdraw(ctx: Context<Withdraw>, amount: u64) -> Result<()> {
        require!(amount > 0, BankError::InvalidAmount);

        // require!(ctx.accounts.bank_account.ower == ctx.accounts.signer.key(),)
        // require!(ctx.accounts.bank_account.balance>amount,BankError::InsufficientFunds);

        let user = &mut ctx.accounts.signer;
        let bank_account = &mut ctx.accounts.bank_account;

        //检查账户余额
        let new_balance = bank_account
            .balance
            .checked_sub(amount)
            .ok_or(BankError::InsufficientFunds)?;

        //lamports转移
        user.add_lamports(amount);

        bank_account.sub_lamports(amount);

        // 更新余额
        bank_account.balance = new_balance;

        msg!(
            "取款成功！金额: {} lamports，余额: {}",
            amount,
            bank_account.balance
        );

        Ok(())
    }
    // │  指令 4：transfer（转账）
    // │    → 从自己的 BankAccount 转 SOL 到别人的 BankAccount
    // │    → 不能给自己转账
    // │    → 余额不足时报错
    // │    → 只有转出方户主才需要签名

    pub fn transfer(ctx: Context<Transfer>, amount: u64) -> Result<()> {
        let from_account = &mut ctx.accounts.from_account;
        let to_account = &mut ctx.accounts.to_account;

        //金额验证
        require!(amount > 0, BankError::InvalidAmount);

        //余额验证
        let new_balance = from_account
            .balance
            .checked_sub(amount)
            .ok_or(BankError::InsufficientFunds)?;

        //不能给自己转账
        require!(
            from_account.owner != to_account.owner,
            BankError::SelfTransfer
        );

        from_account.sub_lamports(amount);
        to_account.add_lamports(amount);

        from_account.balance = new_balance;
        to_account.balance += amount;

        msg!(
            "转账成功！{} → {}，金额: {} lamports",
            ctx.accounts.from_account.owner,
            ctx.accounts.to_account.owner,
            amount
        );

        Ok(())
    }

    // │  指令 5：close_account（销户）
    // │    → 关闭 BankAccount，退还租金
    // │    → 余额必须为 0 才能销户
    // │    → 只有户主才能销户
    pub fn close_account(ctx: Context<CloseAccount>) -> Result<()> {
        require!(
            ctx.accounts.bank_account.balance == 0,
            BankError::AccountNotEmpty
        );

        // close = user 会自动：
        //   1. 把 bank_account 的所有 lamports 转给 user（退还租金）
        //   2. 把 bank_account 的 data 清零
        //   3. 把 bank_account 的 owner 设为 System Program
        Ok(())
    }

    // ==========================================
    // 指令6：冻结银行账户 FreezeAccount
    // 需要 admin（管理员）+ owner（账户拥有者）同时签名
    // → 防止管理员单方面冻结用户资金
    // → 也防止用户自己随意冻结逃避审查
    // ==========================================

    pub fn freeze_account(ctx: Context<FreezeAccount>) -> Result<()> {
        let bank_account = &mut ctx.accounts.bank_account;

        //不能冻结已冻结的号
        require!(
            bank_account.status != AccountStatus::Frozen,
            BankError::AccountFrozen
        );

        bank_account.status = AccountStatus::Frozen;

        Ok(())
    }

    ///指令7，初始化银行配置
    pub fn init_config(ctx: Context<InitBankConfig>, rate: u16) -> Result<()> {
        let bank_config = &mut ctx.accounts.bank_config;

        bank_config.admin = ctx.accounts.admin.key();

        bank_config.bump = ctx.bumps.bank_config;

        bank_config.fee_rate = rate;

        // bank_config.announcement = String::from("this is  a bank announcement ");

        bank_config.total_accounts = 0;

        Ok(())
    }

    ///指令7，更新配置
    pub fn update_config(
        ctx: Context<UpdateConfig>,
        rate: u16,
        announcement: String,
    ) -> Result<()> {
        let bank_config = &mut ctx.accounts.bank_config;

        bank_config.fee_rate = rate;

        bank_config.announcement = announcement;

        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(rate: u16,announcement:String)] //当需要用到指令参数时传入
pub struct UpdateConfig<'info> {
    #[account(mut)]
    pub admin: Signer<'info>, //银行付钱

    #[account(
        mut,
        constraint = bank_config.admin == admin.key() ,  //校验是不是管理员
        seeds=[b"bank_config"],                     //PDA seeds：固定字符串（全局唯一）
        realloc = BankConfig::space(announcement.len()),
        realloc::payer=admin,
        realloc::zero = true,
        bump=bank_config.bump
    )]
    pub bank_config: Box<Account<'info, BankConfig>>,

    pub system_program: Program<'info, System>, //realloc需要转移租金
}

#[account]
pub struct BankConfig {
    pub admin: Pubkey,
    pub fee_rate: u16,        // 手续费率，单位：基点（1 基点 = 0.01%）（2 字节）
    pub total_accounts: u32,  // 已开户总数（4 字节）
    pub announcement: String, // 公告（动态长度：4 + N 字节）
    pub bump: u8,             // PDA bump 值（1 字节）
}

impl BankConfig {

    // pub admin: Pubkey,
    // pub fee_rate: u16,        // 手续费率，单位：基点（1 基点 = 0.01%）（2 字节）
    // pub total_accounts: u32,  // 已开户总数（4 字节）
    // pub announcement: String, // 公告（动态长度：4 + N 字节）
    // pub bump: u8,             // PDA bump 值（1 字节）
    pub fn space(announcement: usize) -> usize {
        8 + 32 + 2 + 4 + (4 + announcement) + 1
    }
}

#[derive(Accounts)]
pub struct FreezeAccount<'info> {
    pub signer: Signer<'info>, // 账户所有者

    #[account(mut)]
    pub admin: Signer<'info>, //银行付钱

    #[account(
        constraint = bank_config.admin == admin.key() ,  //校验是不是管理员
        seeds=[b"bank_config"],                     //PDA seeds：固定字符串（全局唯一）
        bump
    )]
    pub bank_config: Account<'info, BankConfig>,

    #[account(
        mut,
        constraint = bank_account.owner == signer.key() @ BankError::AccountNotEmpty,
        seeds=[b"bank_account",signer.key().as_ref()],
        bump
    )]
    pub bank_account: Account<'info, BankAccount>,
}

#[derive(Accounts)]

pub struct InitBankConfig<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        init,
        payer = admin,
        space=BankConfig::space(0),
        seeds=[b"bank_config"],                     //PDA seeds：固定字符串（全局唯一）
        bump
    )]
    pub bank_config: Account<'info, BankConfig>,

    pub system_program: Program<'info, System>,
}



#[derive(Accounts)]
pub struct CloseAccount<'info> {
    #[account(mut)]
    pub signer: Signer<'info>, // 转出方

    #[account(
        mut,
        constraint = bank_account.owner == signer.key() @ BankError::AccountNotEmpty,
        seeds=[b"bank_account",signer.key().as_ref()],
        bump = bank_account.bump,
        close = signer
    )]
    pub bank_account: Account<'info, BankAccount>,
    // pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Transfer<'info> {
    #[account(mut)]
    pub signer: Signer<'info>, // 转出方

    // pub system_program: Program<'info, System>, //  合约有权检查PDA账户lamports
    #[account(
        mut,
        constraint = from_account.owner == signer.key() ,
        seeds=[b"bank_account",signer.key().as_ref()],
        bump,
    )]
    pub from_account: Account<'info, BankAccount>,

    #[account(
        mut,
        seeds=[b"bank_account",to_account.owner.as_ref()],
        bump,
    )]
    pub to_account: Account<'info, BankAccount>,
}

#[derive(Accounts)]
pub struct Withdraw<'info> {
    #[account(mut)]
    pub signer: Signer<'info>, //取款 bank-》钱包 账户 lamports 增加

    // pub system_program: Program<'info, System>, //  合约有权检查PDA账户lamports
    #[account(
        mut,
        // constraint = bank_account.owner == signer.key() , //收钱不需要同意
        seeds=[b"bank_account",signer.key().as_ref()],
        bump,
    )]
    pub bank_account: Account<'info, BankAccount>,
}

#[derive(Accounts)]
pub struct Deposit<'info> {
    #[account(mut)]
    pub signer: Signer<'info>, //存款 钱包 -》 账户 lamports 减少

    pub system_program: Program<'info, System>, //  CPI转 SOL需要

    #[account(
        mut,
        constraint = bank_account.owner == signer.key() ,
        seeds=[b"bank_account",signer.key().as_ref()],
        bump,
    )]
    pub bank_account: Account<'info, BankAccount>,
}

#[derive(Accounts)]
pub struct OpenAccount<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,

    pub system_program: Program<'info, System>,

    #[account(
        init,
        payer=signer,
        seeds=[b"bank_account",signer.key().as_ref()],
        space = 8 +32 +(4+50) +8 +1  +8 +1,
        bump,
    )]
    pub bank_account: Account<'info, BankAccount>,
}

#[account]
pub struct BankAccount {
    pub owner: Pubkey,
    pub name: String,
    pub balance: u64,
    pub status: AccountStatus,
    pub create_at: i64,
    pub bump: u8,
}

#[derive(AnchorDeserialize, AnchorSerialize, Clone, PartialEq, Eq)]
pub enum AccountStatus {
    Active, //可用
    Frozen, //冻结
}

#[error_code]
pub enum BankError {
    #[msg("存款金额必须大于 0")]
    InvalidAmount,

    #[msg("余额不足!")]
    InsufficientFunds,

    #[msg("账户1名称过长，最多50字节")]
    NameTooLong,

    #[msg("不能给自己转账")]
    SelfTransfer,

    #[msg("账户余额不为0,请先取出所有资金再销户")]
    AccountNotEmpty,

    #[msg("账户已冻结")]
    AccountFrozen,
}
