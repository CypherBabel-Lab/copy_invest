import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { CopyInvest } from "../target/types/copy_invest";


describe("copy_invest", () => {
  // Configure the client to use the local cluster.
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.CopyInvest as Program<CopyInvest>;

  //create an account to store the price data
  const account = anchor.web3.Keypair.generate();

  it("Is initialized!", async () => {
    // Add your test here.
  });
});
