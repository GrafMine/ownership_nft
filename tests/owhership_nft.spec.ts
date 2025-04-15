import { MPL_TOKEN_METADATA_PROGRAM_ID as MPL_ID_STR } from "@metaplex-foundation/mpl-token-metadata";
import { TOKEN_PROGRAM_ID as SPL_TOKEN_PROGRAM_ID } from "@solana/spl-token";
import { Keypair, SystemProgram, PublicKey } from "@solana/web3.js";
import { getAssociatedTokenAddressSync } from "@solana/spl-token";
import { OwhershipNft } from "../target/types/owhership_nft";
import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { v4 as uuidv4 } from 'uuid';
import { parse } from 'uuid-parse';
import {
  TOKEN_2022_PROGRAM_ID,
  ExtensionType,
  getMintLen,
  ASSOCIATED_TOKEN_PROGRAM_ID
} from "@solana/spl-token";

jest.setTimeout(60000);
const MPL_TOKEN_METADATA_PROGRAM_ID = new PublicKey(MPL_ID_STR);

describe("Initialize Token", () => {
  const ADMIN_KEYPAIR = Keypair.generate();

  // Configure the client to use the local cluster.
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.OwhershipNft as Program<OwhershipNft>;

  // Keypairs
  const lotteryCreator = provider.wallet as anchor.Wallet; // Use provider wallet as payer

  // Participation Token Accounts (created manually in instruction)
  const participationTokenMintKp = Keypair.generate();

  it("Initializes ownership NFT", async () => {
    const ticketIdString = uuidv4();
    const buffer = Buffer.alloc(16);
    parse(ticketIdString, buffer);
    const ticketIdBytes = new Uint8Array(buffer);

    // Ownership NFT
    const [ownershipNftMintPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("lottery_nft_mint"), ticketIdBytes],
      program.programId
    );
    const [ownershipNftMetadataPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("metadata"), MPL_TOKEN_METADATA_PROGRAM_ID.toBytes(), ownershipNftMintPda.toBytes()],
      MPL_TOKEN_METADATA_PROGRAM_ID // Pass the PublicKey object
    );
    const [ownershipNftMasterEditionPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("metadata"), MPL_TOKEN_METADATA_PROGRAM_ID.toBytes(), ownershipNftMintPda.toBytes(), Buffer.from("edition")],
      MPL_TOKEN_METADATA_PROGRAM_ID // Pass the PublicKey object
    );
    const ownershipNftTokenAccount = getAssociatedTokenAddressSync(ownershipNftMintPda, lotteryCreator.publicKey);
    // --- Call Instruction ---
    console.log("Calling initLotteryToken...");
    console.log("Payer (Lottery Creator):", lotteryCreator.publicKey.toBase58());
    console.log("Admin Pubkey:", ADMIN_KEYPAIR.publicKey.toBase58());
    console.log("Ownership NFT Mint PDA:", ownershipNftMintPda.toBase58());
  

    try {
      // --- CREATE ACCOUNTS OBJECT ---
      const accounts = {
        // Ownership NFT
        ownershipNftMint: ownershipNftMintPda,
        ownershipNftMetadata: ownershipNftMetadataPda,
        ownershipNftMasterEdition: ownershipNftMasterEditionPda,
        ownershipNftTokenAccount: ownershipNftTokenAccount,
        // Other Accounts
        payer: lotteryCreator.publicKey,
        admin: ADMIN_KEYPAIR.publicKey,
        updateAuthority: ADMIN_KEYPAIR.publicKey,
        systemProgram: SystemProgram.programId,
        tokenProgram: TOKEN_2022_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        tokenMetadataProgram: MPL_TOKEN_METADATA_PROGRAM_ID,
        rent: anchor.web3.SYSVAR_RENT_PUBKEY,
        splTokenProgram: SPL_TOKEN_PROGRAM_ID,
      };

      // --- CHANGE: Calculate size/rent for ONLY participation mint and create instructions ---

      // 1. Ownership NFT Mint - NO LONGER CREATED HERE
      // const ownershipExtensions = [ExtensionType.TransferHook, ExtensionType.MetadataPointer]; // Assuming same extensions for NFT for now
      // const ownershipMintLen = getMintLen(ownershipExtensions);
      // const lamportsForOwnershipMint = await provider.connection.getMinimumBalanceForRentExemption(ownershipMintLen);
      // console.log(`Calculated Ownership NFT mint size: ${ownershipMintLen}, Rent required: ${lamportsForOwnershipMint}`);
      // const createOwnershipNftMintAccountIx = SystemProgram.createAccount({
      //     fromPubkey: lotteryCreator.publicKey, // Payer
      //     newAccountPubkey: ownershipNftMintPda, // New account PDA
      //     lamports: lamportsForOwnershipMint, // Rent
      //     space: ownershipMintLen, // Size
      //     programId: TOKEN_2022_PROGRAM_ID, // Owner - MUST be token program
      // });
      // console.log("Create Ownership NFT Mint Account instruction created.");

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

      // 4. Assemble the transaction with TWO instructions (Order matters!)
      const transaction = new anchor.web3.Transaction();
      // transaction.add(createOwnershipNftMintAccountIx); // REMOVED: First, create ownership NFT mint
      transaction.add(createParticipationMintAccountIx); // FIRST, create participation token mint
      transaction.add(initLotteryTokenInstruction); // THEN, the main instruction
      console.log("Transaction created with 2 instructions.");

      // 5. Set the fee payer and blockhash
      transaction.feePayer = lotteryCreator.publicKey;
      transaction.recentBlockhash = (await provider.connection.getLatestBlockhash()).blockhash;
      console.log(`Fee payer set to ${transaction.feePayer.toBase58()}, Blockhash: ${transaction.recentBlockhash}`);

      // 6. Sign the transaction with the required keys:
      //    - lotteryCreator (payer) - signed last via provider.wallet
      //    - ADMIN_KEYPAIR - required by the main instruction
      //    - participationTokenMintKp - Required for createAccount

      // Sign with "additional" keys
      transaction.partialSign(ADMIN_KEYPAIR);
      transaction.partialSign(participationTokenMintKp); // Required for createAccount
      console.log("Transaction partially signed by Admin and Participation Mint Kp.");

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

