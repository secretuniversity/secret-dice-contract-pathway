import { SecretNetworkClient, Wallet, } from "secretjs";
import { AminoWallet } from "secretjs/dist/wallet_amino";

export type jsEnv = {
  accounts: Account[];
  contracts: ContractInfo[];
}

export type Account = {
  address: string;
  mnemonic: string;
  walletAmino: AminoWallet;
  walletProto: Wallet;
  secretjs: SecretNetworkClient;
};

export type ContractInfo = {
  hash: string;
  address: string;
}

export type Balance = {
  address: string,
  amount: string,
};

export type InitMsg = {  }

export type JoinMsg = {
  join: {
    name: string,
    secret: string,
  }
}

export type RollDiceMsg = {
  roll_dice: {  }
}

export type LeaveMsg = {
  leave: {  }
}

export type HandleMsg = JoinMsg | RollDiceMsg | LeaveMsg;


export type WhoWonMsg = {
  who_won: {  }
}

export type QueryMsg = WhoWonMsg;

export type WhoWonResponse = { 
    name: string,
    addr: string,
    dice_roll: number,
};

export type QueryResponse = WhoWonResponse