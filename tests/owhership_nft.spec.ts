// import { MPL_TOKEN_METADATA_PROGRAM_ID as MPL_ID_STR } from "@metaplex-foundation/mpl-token-metadata";

import { Keypair, SystemProgram, PublicKey, ComputeBudgetProgram } from "@solana/web3.js";
import { OwhershipNft } from "../target/types/owhership_nft";
import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { v4 as uuidv4 } from 'uuid';
import { parse } from 'uuid-parse';
import {
  getAssociatedTokenAddressSync,
  TOKEN_2022_PROGRAM_ID,
  ExtensionType,
  getMintLen,
  ASSOCIATED_TOKEN_PROGRAM_ID,
  getAccount,
  unpackMint,
  getMetadataPointerState,
} from "@solana/spl-token";
import { BN } from "bn.js";

jest.setTimeout(60000);

process.env.ANCHOR_PROVIDER_LOGS = "true";

describe("Initialize Ownership NFT", () => {

  const secret = Uint8Array.from(
    [206,241,118,125,82,225,7,5,234,9,67,209,37,100,26,183,190,244,124,81,227,190,190,180,237,2,24,70,14,131,36,186,196,49,119,241,84,72,174,21,39,203,148,43,111,97,189,117,219,157,187,242,107,205,96,30,175,144,175,16,189,127,73,85]
  );
   const kp = Keypair.fromSecretKey(secret);
   console.log("kp.publicKey.toBase58()", kp.publicKey.toBase58());

  // Массив из admin-keypair.json (64 байта)
  const ADMIN_SECRET_KEY = Uint8Array.from([
    206,241,118,125,82,225,7,5,234,9,67,209,37,100,26,183,190,244,124,81,227,190,190,180,237,2,24,70,14,131,36,186,196,49,119,241,84,72,174,21,39,203,148,43,111,97,189,117,219,157,187,242,107,205,96,30,175,144,175,16,189,127,73,85
  ]);
  const ADMIN_KEYPAIR = Keypair.fromSecretKey(ADMIN_SECRET_KEY);

  console.log("ADMIN_KEYPAIR pubkey:", ADMIN_KEYPAIR.publicKey.toBase58());
  console.log("kp.publicKey.toBase58():", kp.publicKey.toBase58());

  // Configure the client to use the local cluster.
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.OwhershipNft as Program<OwhershipNft>;

  // Keypairs
  const lotteryCreator = provider.wallet as anchor.Wallet; // Use provider wallet as payer

  // Participation Token Accounts (created manually in instruction)
  const participationTokenMintKp = Keypair.generate();

  it("Initializes ownership NFT with Token-2022, MetadataPointer, and custom metadata account", async () => {
    const ticketIdString = uuidv4();
    const buffer = Buffer.alloc(16);
    parse(ticketIdString, buffer);
    const ticketIdBytes = new Uint8Array(buffer);
    console.log("Ticket ID (String):", ticketIdString);
    console.log("Ticket ID (Bytes):", Buffer.from(ticketIdBytes).toString("hex"));

    // 1. Рассчитываем PDA для Mint аккаунта
    const [ownershipNftMintPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("lottery_nft_mint"), ticketIdBytes],
      program.programId
    );
    console.log("Ownership NFT Mint PDA:", ownershipNftMintPda.toBase58());

    // 2. Рассчитываем PDA для аккаунта Метаданных (принадлежит нашей программе)
    const [metadataPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("metadata"), ownershipNftMintPda.toBuffer()], // seeds: "metadata" + mint PDA key
      program.programId
    );
    console.log("Ownership NFT Metadata PDA:", metadataPda.toBase58());

    // 3. Рассчитываем адрес ATA для Payer'а
    const ownershipNftTokenAccount = getAssociatedTokenAddressSync(
      ownershipNftMintPda,          // mint PDA
      lotteryCreator.publicKey,     // owner (payer)
      false,                        // allowOwnerOffCurve
      TOKEN_2022_PROGRAM_ID,        // token program ID
      ASSOCIATED_TOKEN_PROGRAM_ID   // ATA program ID
    );
    console.log("Ownership NFT ATA for Payer:", ownershipNftTokenAccount.toBase58());

    // 4. Собираем объект Accounts для вызова инструкции
    const accounts = {
      ownershipNftMint: ownershipNftMintPda,
      ownershipNftMetadata: metadataPda, // Используем PDA метаданных
      ownershipNftTokenAccount: ownershipNftTokenAccount, // Используем ATA пейера
      payer: lotteryCreator.publicKey,
      admin: ADMIN_KEYPAIR.publicKey,
      systemProgram: SystemProgram.programId,
      tokenProgram: TOKEN_2022_PROGRAM_ID,
      associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
      rent: anchor.web3.SYSVAR_RENT_PUBKEY,
    };

    // 5. Participation Token Mint (создаем отдельно, т.к. не часть основного потока NFT)
    // Если он не нужен для теста NFT, можно закомментировать
    const participationTokenMintKp = Keypair.generate();
    const participationExtensions = [ExtensionType.TransferHook, ExtensionType.MetadataPointer]; // Пример расширений
    const participationMintLen = getMintLen(participationExtensions);
    const lamportsForParticipationMint = await provider.connection.getMinimumBalanceForRentExemption(participationMintLen);
    const createParticipationMintAccountIx = SystemProgram.createAccount({
        fromPubkey: lotteryCreator.publicKey,
        newAccountPubkey: participationTokenMintKp.publicKey,
        lamports: lamportsForParticipationMint,
        space: participationMintLen,
        programId: TOKEN_2022_PROGRAM_ID,
    });
    console.log("Create Participation Mint Ix created for:", participationTokenMintKp.publicKey.toBase58());

    // 6. Формируем инструкцию вызова нашей программы
    const initLotteryTokenInstruction = await program.methods
      .initOwnershipNft({
        ticketId: Array.from(ticketIdBytes),
      })
      .accounts(accounts)
      .instruction();
    console.log("Program instruction `initOwnershipNft` created.");

    // 7. Собираем транзакцию
    const transaction = new anchor.web3.Transaction();

    // Добавляем инструкцию для увеличения лимита CU
    const modifyComputeUnits = ComputeBudgetProgram.setComputeUnitLimit({ units: 500000 }); // Можно подбирать значение
    transaction.add(modifyComputeUnits);

    // Добавляем инструкцию создания participation mint (если нужна)
    transaction.add(createParticipationMintAccountIx);

    // Добавляем основную инструкцию нашей программы
    transaction.add(initLotteryTokenInstruction);
    console.log("Transaction created with instructions.");

    // 8. Назначаем плательщика и получаем blockhash
    transaction.feePayer = lotteryCreator.publicKey;
    transaction.recentBlockhash = (await provider.connection.getLatestBlockhash()).blockhash;
    console.log(`Fee payer: ${transaction.feePayer.toBase58()}, Blockhash: ${transaction.recentBlockhash}`);

    // 9. Подписываем транзакцию необходимыми ключами
    transaction.partialSign(ADMIN_KEYPAIR); // Admin подписывает
    transaction.partialSign(participationTokenMintKp); // KP для participation mint (если создаем)
    console.log("Transaction partially signed by Admin and Participation Mint KP.");

    // Финальная подпись кошельком плательщика (провайдером)
    const signedTx = await provider.wallet.signTransaction(transaction);
    console.log("Transaction fully signed by Payer.");

    // 10. Отправка и подтверждение транзакции
    console.log("Sending transaction...");
    let txSignature: string | undefined = undefined;
    try {
      // --- Симуляция для отладки ---
      // const simulation = await provider.connection.simulateTransaction(signedTx, { commitment: "confirmed" });
      // console.log('Simulation Result:', simulation);
      // if (simulation.value.err) {
      //     console.error("SIMULATION FAILED:", simulation.value.err);
      //     console.error("Simulation logs:", simulation.value.logs);
      //     throw new Error(`Simulation failed: ${simulation.value.err}`);
      // }
      // console.log('Simulation successful. Units Consumed:', simulation.value.unitsConsumed);
      // --- Конец Симуляции ---

      txSignature = await provider.connection.sendRawTransaction(signedTx.serialize());
      console.log("Transaction sent. Signature:", txSignature);

      const confirmation = await provider.connection.confirmTransaction({
            signature: txSignature,
            blockhash: transaction.recentBlockhash,
            lastValidBlockHeight: (await provider.connection.getLatestBlockhash()).lastValidBlockHeight
        }, 'confirmed');

      console.log("Transaction confirmation status:", confirmation);
      if (confirmation.value.err) {
        console.error("Transaction failed confirmation:", confirmation.value.err);
        // Попытка получить логи неудавшейся транзакции
        const failedTx = await provider.connection.getTransaction(txSignature!, {maxSupportedTransactionVersion: 0, commitment: "confirmed"});
        console.error("Failed transaction logs:", failedTx?.meta?.logMessages?.join('\n'));
        throw new Error(`Transaction Confirmation Failed: ${confirmation.value.err}`);
      }

      console.log(`Transaction successful! Explorer link: https://explorer.solana.com/tx/${txSignature}?cluster=custom&customUrl=${provider.connection.rpcEndpoint}`);

    } catch (error: any) {
      console.error("Error during transaction send/confirm:", error);
      if (txSignature && !(error.message?.includes("Confirmation Failed"))) {
         // Если ошибка не при подтверждении, а при отправке, но сигнатура есть
          const failedTx = await provider.connection.getTransaction(txSignature, {maxSupportedTransactionVersion: 0, commitment: "confirmed"});
          console.error("Failed transaction logs (from catch):", failedTx?.meta?.logMessages?.join('\n'));
      } else if (error.logs) {
         console.error("Error Logs:", error.logs);
      }
      throw error; // Перебрасываем ошибку, чтобы тест упал
    }

    // 11. Проверки после успешной транзакции
    console.log("Performing post-transaction checks...");

    // Проверка Mint аккаунта
    const mintAccInfo = await provider.connection.getAccountInfo(ownershipNftMintPda, 'confirmed');
    expect(mintAccInfo).not.toBeNull();
    expect(mintAccInfo?.owner.equals(TOKEN_2022_PROGRAM_ID)).toBe(true);
    console.log(`Mint account ${ownershipNftMintPda.toBase58()} exists and owned by Token-2022.`);
    // Дополнительно: распаковка минта для проверки расширений
    const mintData = unpackMint(ownershipNftMintPda, mintAccInfo, TOKEN_2022_PROGRAM_ID);
    const pointerState = getMetadataPointerState(mintData);
    expect(pointerState).not.toBeNull();
    expect(pointerState!.metadataAddress?.equals(metadataPda)).toBe(true); // Проверяем, что указатель ссылается на наш PDA метаданных
    console.log("MetadataPointer extension verified on mint.");

    // Проверка аккаунта метаданных
    const metadataAccInfo = await provider.connection.getAccountInfo(metadataPda, 'confirmed');
    expect(metadataAccInfo).not.toBeNull();
    expect(metadataAccInfo?.owner.equals(program.programId)).toBe(true); // Должен принадлежать нашей программе
    console.log(`Metadata account ${metadataPda.toBase58()} exists and owned by program ${program.programId}.`);
    // Распаковка данных метаданных
    const decodedMetadata = await program.account.ownershipNftMetadata.fetch(metadataPda);
    console.log("Decoded Metadata:", decodedMetadata);
    const expectedName = generateLotteryTokenNameForTest(ticketIdBytes); // Используем хелпер для генерации ожидаемого имени
    const expectedUri = generateLotteryMetadataUriForTest(ticketIdBytes);   // Используем хелпер для генерации ожидаемого URI
    expect(decodedMetadata.name).toEqual(expectedName);
    expect(decodedMetadata.symbol).toEqual("OWNER-NFT"); // Используем константу
    expect(decodedMetadata.uri).toEqual(expectedUri);
    console.log("Metadata content verified.");

    // Проверка ATA пейера
    const payerAtaInfo = await getAccount(provider.connection, ownershipNftTokenAccount, 'confirmed', TOKEN_2022_PROGRAM_ID);
    expect(payerAtaInfo).not.toBeNull();
    expect(payerAtaInfo.mint.equals(ownershipNftMintPda)).toBe(true);
    expect(payerAtaInfo.owner.equals(lotteryCreator.publicKey)).toBe(true);
    expect(payerAtaInfo.amount).toEqual(BigInt(1)); // Проверяем баланс BigInt -> number
    console.log(`Payer ATA ${ownershipNftTokenAccount.toBase58()} exists, owned by payer, mint matches, balance is 1.`);

    console.log("All checks passed!");
  });
});

// Хелперы для генерации ожидаемых значений в тесте (копия логики из lib.rs)
function generateLotteryTokenNameForTest(uuid_bytes: Uint8Array): string {
  return Buffer.from(uuid_bytes).toString('hex');
}

function generateLotteryMetadataUriForTest(uuid_bytes: Uint8Array): string {
  const ticket_id_str = generateLotteryTokenNameForTest(uuid_bytes);
  return `http://localhost:3000/api/metadata/test/${ticket_id_str}`;
}

