import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { MiniBank } from "../target/types/mini_bank";
import { expect } from "chai";
import { PublicKey, SystemProgram, LAMPORTS_PER_SOL } from "@solana/web3.js";

describe("mini-bank", () => {
  // 1. 配置 Provider 环境
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.MiniBank as Program<MiniBank>;

  // 生成两个测试账号：Alice 和 Bob
  const alice = anchor.web3.Keypair.generate();
  const bob = anchor.web3.Keypair.generate();

  // 辅助函数：根据所有者公钥推导 BankAccount PDA
  const getBankPDA = (owner: PublicKey) => {
    return PublicKey.findProgramAddressSync(
        [Buffer.from("bank_account"), owner.toBuffer()],
        program.programId
    )[0];
  };

  it("准备测试环境：为空白账号空投 SOL", async () => {
    const signature = await provider.connection.requestAirdrop(alice.publicKey, 2 * LAMPORTS_PER_SOL);
    await provider.connection.confirmTransaction(signature);

    const signature2 = await provider.connection.requestAirdrop(bob.publicKey, 2 * LAMPORTS_PER_SOL);
    await provider.connection.confirmTransaction(signature2);
  });

  it("指令 1: Alice 成功开户", async () => {
    const bankPDA = getBankPDA(alice.publicKey);
    const accountName = "Alice_Savings";

    await program.methods
        .openAccount(accountName)
        .accounts({
          signer: alice.publicKey,
          // bankAccount: bankPDA, // Anchor 会根据 seeds 自动推导，此处可省略
          systemProgram: SystemProgram.programId,
        } as any) // 强制转换类型以匹配生成的 IDL
        .signers([alice])
        .rpc();

    const account = await program.account.bankAccount.fetch(bankPDA);
    expect(account.name).to.equal(accountName);
    expect(account.owner.toBase58()).to.equal(alice.publicKey.toBase58());
    expect(account.balance.toNumber()).to.equal(0);
  });

  it("指令 2: Alice 存款 1 SOL", async () => {
    const bankPDA = getBankPDA(alice.publicKey);
    const depositAmount = new anchor.BN(1 * LAMPORTS_PER_SOL);

    await program.methods
        .deposit(depositAmount)
        .accounts({
          signer: alice.publicKey,
          bankAccount: bankPDA,
          systemProgram: SystemProgram.programId,
        } as any)
        .signers([alice])
        .rpc();

    const account = await program.account.bankAccount.fetch(bankPDA);
    expect(account.balance.toString()).to.equal(depositAmount.toString());
  });

  it("指令 3: Alice 取款 0.4 SOL", async () => {
    const bankPDA = getBankPDA(alice.publicKey);
    const withdrawAmount = new anchor.BN(0.4 * LAMPORTS_PER_SOL);

    await program.methods
        .withdraw(withdrawAmount)
        .accounts({
          signer: alice.publicKey,
          bankAccount: bankPDA,
        } as any)
        .signers([alice])
        .rpc();

    const account = await program.account.bankAccount.fetch(bankPDA);
    // 检查逻辑余额是否更新（0.6 SOL = 600,000,000 lamports）
    expect(account.balance.toNumber()).to.equal(0.6 * LAMPORTS_PER_SOL);
  });

  it("指令 4: Alice 转账 0.2 SOL 给 Bob", async () => {
    // Bob 必须先开户才能接收转账（因为你的 seeds 依赖于 PDA）
    await program.methods
        .openAccount("Bob_Wallet")
        .accounts({ signer: bob.publicKey } as any)
        .signers([bob])
        .rpc();

    const aliceBank = getBankPDA(alice.publicKey);
    const bobBank = getBankPDA(bob.publicKey);
    const transferAmount = new anchor.BN(0.2 * LAMPORTS_PER_SOL);

    await program.methods
        .transfer(transferAmount)
        .accounts({
          signer: alice.publicKey,
          fromAccount: aliceBank,
          toAccount: bobBank,
        } as any)
        .signers([alice])
        .rpc();

    const aliceAcc = await program.account.bankAccount.fetch(aliceBank);
    const bobAcc = await program.account.bankAccount.fetch(bobBank);

    expect(aliceAcc.balance.toNumber()).to.equal(0.4 * LAMPORTS_PER_SOL);
    expect(bobAcc.balance.toNumber()).to.equal(0.2 * LAMPORTS_PER_SOL);
  });

  it("指令 5: 销户测试 (预期失败 & 成功情况)", async () => {
    const bobBank = getBankPDA(bob.publicKey);

    // 1. 尝试在有余额时销户 -> 应该报错 AccountNotEmpty
    try {
      await program.methods
          .closeAccount()
          .accounts({
            signer: bob.publicKey,
            bankAccount: bobBank,
          } as any)
          .signers([bob])
          .rpc();
      expect.fail("有余额时销户应当失败");
    } catch (err: any) {
      // 检查错误码（兼容 Anchor 的错误结构）
      const errMsg = err.error?.errorCode?.code || err.toString();
      expect(errMsg).to.include("AccountNotEmpty");
    }

    // 2. 取清余额后再销户 -> 应该成功
    const currentBalance = (await program.account.bankAccount.fetch(bobBank)).balance;
    await program.methods
        .withdraw(currentBalance)
        .accounts({ signer: bob.publicKey, bankAccount: bobBank } as any)
        .signers([bob])
        .rpc();

    await program.methods
        .closeAccount()
        .accounts({
          signer: bob.publicKey,
          bankAccount: bobBank,
        } as any)
        .signers([bob])
        .rpc();

    // 验证账户是否已销毁（fetch 应该报错）
    try {
      await program.account.bankAccount.fetch(bobBank);
      expect.fail("账户本应被销毁");
    } catch (err) {
      expect(err.toString()).to.include("Account does not exist");
    }
  });
});