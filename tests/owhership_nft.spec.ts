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
  ASSOCIATED_TOKEN_PROGRAM_ID
} from "@solana/spl-token";

jest.setTimeout(60000);

process.env.ANCHOR_PROVIDER_LOGS = "true";

describe("Initialize Token", () => {

  
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

  it("Initializes ownership NFT with Token-2022", async () => {
    const ticketIdString = uuidv4();
    const buffer = Buffer.alloc(16);
    parse(ticketIdString, buffer);
    const ticketIdBytes = new Uint8Array(buffer);

    // Ownership NFT
    const [ownershipNftMintPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("lottery_nft_mint"), ticketIdBytes],
      program.programId
    );
    
    // Создаем отдельный аккаунт для метаданных (не PDA)
    const metadataAccount = Keypair.generate();
    
    // --- ЯВНЫЙ ВЫВОД ДЛЯ ДЕБАГА ---
    console.log("ticketIdBytes (hex):", Buffer.from(ticketIdBytes).toString("hex"));
    console.log("ownershipNftMintPda:", ownershipNftMintPda.toBase58());
    console.log("metadata account:", metadataAccount.publicKey.toBase58());
    // --- конец дебага ---
    
    const ownershipNftTokenAccount = getAssociatedTokenAddressSync(
      ownershipNftMintPda,      // Mint
      lotteryCreator.publicKey, // Owner
      false,                    // allowOwnerOffCurve - usually false
      TOKEN_2022_PROGRAM_ID,    // Token program ID
      ASSOCIATED_TOKEN_PROGRAM_ID // ATA program ID
    );
    
    // --- Call Instruction ---
    console.log("Calling initOwnershipNft...");
    console.log("Payer (Lottery Creator):", lotteryCreator.publicKey.toBase58());
    console.log("Admin Pubkey:", ADMIN_KEYPAIR.publicKey.toBase58());
    console.log("Ownership NFT Mint PDA:", ownershipNftMintPda.toBase58());
  
    // Добавляем подробные диагностические логи для сравнения с контрактом
    console.log("=== TEST EXPECTATION: ACCOUNT ADDRESSES ===");
    console.log("Expected metadata_address:", metadataAccount.publicKey.toBase58());
    console.log("Expected mint_address:", ownershipNftMintPda.toBase58());
    console.log("Expected authority_address (admin):", ADMIN_KEYPAIR.publicKey.toBase58());
    console.log("Expected payer_address:", lotteryCreator.publicKey.toBase58());
    console.log("Expected system_program_address:", SystemProgram.programId.toBase58());
    console.log("Expected token_program_address:", TOKEN_2022_PROGRAM_ID.toBase58());
    console.log("Expected associated_token_program_address:", ASSOCIATED_TOKEN_PROGRAM_ID.toBase58());
    
    try {
      // Создаем аккаунт для метаданных
      const createMetadataAccountIx = SystemProgram.createAccount({
        fromPubkey: lotteryCreator.publicKey,
        newAccountPubkey: metadataAccount.publicKey,
        lamports: await provider.connection.getMinimumBalanceForRentExemption(1000), // Выделяем достаточно места для метаданных
        space: 1000, // Размер для метаданных
        programId: SystemProgram.programId, // Владелец - system program (будет перенаправлено через метадата-поинтер)
      });
      
      // --- CREATE ACCOUNTS OBJECT ---
      const accounts = {
        // Ownership NFT
        ownershipNftMint: ownershipNftMintPda,
        ownershipNftMetadata: metadataAccount.publicKey,
        ownershipNftTokenAccount: ownershipNftTokenAccount,
        // Other Accounts
        payer: lotteryCreator.publicKey,
        admin: ADMIN_KEYPAIR.publicKey,
        systemProgram: SystemProgram.programId,
        tokenProgram: TOKEN_2022_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        rent: anchor.web3.SYSVAR_RENT_PUBKEY,
      };

      // 2. Participation Token Mint
      const participationExtensions = [ExtensionType.TransferHook, ExtensionType.MetadataPointer];
      const participationMintLen = getMintLen(participationExtensions);
      const lamportsForParticipationMint = await provider.connection.getMinimumBalanceForRentExemption(participationMintLen);
      console.log(`Calculated Participation mint size: ${participationMintLen}, Rent required: ${lamportsForParticipationMint}`);
      const createParticipationMintAccountIx = SystemProgram.createAccount({
          fromPubkey: lotteryCreator.publicKey, // Payer
          newAccountPubkey: participationTokenMintKp.publicKey, // New account KP
          lamports: lamportsForParticipationMint, // Rent
          space: participationMintLen, // Size
          programId: TOKEN_2022_PROGRAM_ID, // Owner - token program
      });
      console.log("Create Participation Mint Account instruction created.");

      // 3. Get the main program instruction
      const initLotteryTokenInstruction = await program.methods
          .initOwnershipNft({
            ticketId: Array.from(ticketIdBytes),
          })
          .accounts(accounts)
          .instruction();
      console.log("Main program instruction created.");

      // 4. Assemble the transaction with instructions
      const transaction = new anchor.web3.Transaction();
      
      // Увеличиваем лимит вычислительных единиц для транзакции
      const modifyComputeUnits = ComputeBudgetProgram.setComputeUnitLimit({
        units: 400000, // Увеличиваем в 2 раза (по умолчанию 200,000)
      });
      transaction.add(modifyComputeUnits);
      
      transaction.add(createMetadataAccountIx); // Создаем аккаунт для метаданных
      transaction.add(createParticipationMintAccountIx); // Создаем participation token mint
      transaction.add(initLotteryTokenInstruction); // Основная инструкция
      console.log("Transaction created with 4 instructions (ComputeBudget + 3 main instructions).");

      // 5. Set the fee payer and blockhash
      transaction.feePayer = lotteryCreator.publicKey;
      transaction.recentBlockhash = (await provider.connection.getLatestBlockhash()).blockhash;
      console.log(`Fee payer set to ${transaction.feePayer.toBase58()}, Blockhash: ${transaction.recentBlockhash}`);

      // 6. Sign the transaction with the required keys
      transaction.partialSign(metadataAccount); // Metadata account keypair
      transaction.partialSign(ADMIN_KEYPAIR);
      transaction.partialSign(participationTokenMintKp);
      console.log("Transaction partially signed by Admin, Metadata Account, and Participation Mint Kp.");

      // Final signing by the payer's wallet
      const signedTx = await provider.wallet.signTransaction(transaction);
      console.log("Transaction fully signed by Payer.");

      // <<< Add check for account owner >>>
      try {
        const accountInfo = await provider.connection.getAccountInfo(ownershipNftMintPda);
        if (accountInfo) {
          console.log(`Ownership NFT Mint Account (${ownershipNftMintPda.toBase58()}) exists. Owner: ${accountInfo.owner.toBase58()}`);
        } else {
          console.log(`Ownership NFT Mint Account (${ownershipNftMintPda.toBase58()}) does not exist yet.`);
        }
      } catch (e) {
        console.error("Error checking Ownership NFT Mint account:", e);
      }
      // <<< End check >>>

      // 7. Send and confirm the "raw" transaction
      console.log("Sending fully signed raw transaction...");
      let txSignature
      try {
        const simulation = await provider.connection.simulateTransaction(signedTx);

        // Анализ потребления ресурсов
        console.log('Simulation details:', {
          unitsConsumed: simulation.value.unitsConsumed,
          logs: simulation.value.logs,
        });
      } catch (e: any) {
        console.error("Transaction failed:", e.getLogs());
        throw e;
      }
      
      try {
        txSignature = await provider.connection.sendRawTransaction(signedTx.serialize())
        // .catch((e: any) => {
        //   console.error("Transaction failed:", e.getLogs());
        //   throw e;
        // });
      } catch (e: any) {
        // Improved error logging
        console.error("Error sending transaction:", e);
        if (e.logs) { // Check if logs exist
          console.error("Transaction logs:", e.logs);
        } else {
          console.error("No logs available for this error.")
        }
        throw e;
      }
     
      console.log("Raw transaction sent. Signature:", txSignature!);

      const confirmation = await provider.connection.confirmTransaction(
          txSignature!,
          provider.connection.commitment || 'confirmed'
      );

      if (confirmation.value.err) {
          console.error("Transaction failed confirmation:", confirmation.value.err);
          const failedTx = await provider.connection.getTransaction(txSignature!, {maxSupportedTransactionVersion: 0, commitment: "confirmed"});
          console.error("Failed transaction logs:", failedTx?.meta?.logMessages?.join('\n')); // Join logs for better readability
          throw new Error(`>>>> ERROR: ${confirmation.value.err}`) // No need for redundant logs here
        }
      } catch (error: any) { // Keep this first catch block
        console.error("Test failed:", error);
        if (error.logs) {
          console.error("Error Logs:", error.logs.join('\n'));
        }
        throw error; // Re-throw to fail the test
      }
      // Add assertions here to verify state if needed
      console.log("Transaction successful!");
  });
});

