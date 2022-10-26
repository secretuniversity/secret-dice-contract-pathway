import { SecretNetworkClient, Tx, Coin } from "secretjs";
import fs from "fs";
import assert from "assert";
import { getScrtBalance, initClient, } from "./int_helpers";
import { 
  Account, ContractInfo, jsEnv, 
  InitMsg, HandleMsg, JoinMsg, RollDiceMsg, LeaveMsg,
  QueryMsg, QueryResponse, WhoWonMsg, 
} from "./int_types";

/////////////////////////////////////////////////////////////////////////////////
// Global variables
/////////////////////////////////////////////////////////////////////////////////

const gasLimit = 200000;

/////////////////////////////////////////////////////////////////////////////////
// Upload contract and Init Message
/////////////////////////////////////////////////////////////////////////////////

/** Stores and instantiates a new contract in our network */
const initializeContract = async (
  client: SecretNetworkClient,
  contractPath: string,
  initMsg: InitMsg,
) => {
  // upload contract
  const wasmCode = fs.readFileSync(contractPath);
  console.log("Uploading contract");

  const uploadReceipt = await client.tx.compute.storeCode(
    {
      wasmByteCode: wasmCode,
      sender: client.address,
      source: "",
      builder: "",
    },
    {
      broadcastCheckIntervalMs: 100,
      gasLimit: 5000000,
    }
  );

  if (uploadReceipt.code !== 0) {
    console.log(
      `Failed to get code id: ${JSON.stringify(uploadReceipt.rawLog)}`
    );
    throw new Error(`Failed to upload contract`);
  }

  const codeIdKv = uploadReceipt.jsonLog![0].events[0].attributes.find(
    (a: any) => {
      return a.key === "code_id";
    }
  );

  const codeId = Number(codeIdKv!.value);
  console.log("Contract codeId: ", codeId);

  const contractCodeHash = await client.query.compute.codeHash(codeId);
  console.log(`Contract hash: ${contractCodeHash}`);

  // instantiate contract
  const contract = await client.tx.compute.instantiateContract(
    {
      sender: client.address,
      codeId,
      initMsg: initMsg, 
      codeHash: contractCodeHash,
      label: "Contract " + Math.ceil(Math.random() * 10000) + client.address.slice(6),
    },
    {
      broadcastCheckIntervalMs: 100,
      gasLimit: 1000000,
    }
  );

  if (contract.code !== 0) {
    throw new Error(
      `Failed to instantiate the contract with the following error ${contract.rawLog}`
    );
  }

  const contractAddress = contract.arrayLog!.find(
    (log) => log.type === "message" && log.key === "contract_address"
  )!.value;

  console.log(`Contract address: ${contractAddress}`);

  const contractInfo: [string, string] = [contractCodeHash, contractAddress];
  return contractInfo;
};

/** Initialization procedure: Initialize client, fund new accounts, and upload/instantiate contract */ 
async function initDefault(): Promise<jsEnv> {
  const accounts = await initClient();
  const { secretjs } = accounts[0];

  const initMsgDefault = {  };
  
  const [contractHash, contractAddress] = await initializeContract(
    secretjs,
    "contract.wasm.gz",
    initMsgDefault,
  );

  const contract: ContractInfo = {
    hash: contractHash,
    address: contractAddress
  };

  const env: jsEnv = {
    accounts,
    contracts: [contract],
  }; 

  return env;
}


/////////////////////////////////////////////////////////////////////////////////
// Handle Messages
/////////////////////////////////////////////////////////////////////////////////


async function execHandle(
  sender: Account,
  contract: ContractInfo,
  msg: HandleMsg,
  handleDescription?: string,
  sendAmount?: number,
): Promise<Tx> {
  let sentFunds: Coin[] = [];
  if (typeof sendAmount === 'number') {
    sentFunds = [{
      denom: "uscrt",
      amount: sendAmount.toString()
    }]
  }

  const { secretjs } = sender;
  const tx = await secretjs.tx.compute.executeContract(
    {
      sender: secretjs.address,
      contractAddress: contract.address,
      codeHash: contract.hash,
      msg,
      sentFunds,
    },
    {
      broadcastCheckIntervalMs: 100,
      gasLimit,
    }
  );

  if (handleDescription === undefined) { handleDescription = "handle"}
  console.log(`${handleDescription} used ${tx.gasUsed} gas`);
  return tx
}

async function execJoin(
  sender: Account,
  contract: ContractInfo,
  name: string,
  secret: number,
  deposit: number,
) {
  const msg: JoinMsg = {
    join: { 
      name,
      secret: secret.toString(),
    },
  };

  const tx = await execHandle(sender, contract, msg, "Join", deposit);
  return tx;
}

async function execRollDice(
  sender: Account,
  contract: ContractInfo,
) {
  const msg: RollDiceMsg = {
    roll_dice: {  },
  };

  const tx = await execHandle(sender, contract, msg, "Roll Dice");
  return tx;
}

async function execLeave(
  sender: Account,
  contract: ContractInfo,
) {
  const msg: LeaveMsg = {
    leave: {  },
  };

  const tx = await execHandle(sender, contract, msg, "Leave");
  return tx;
}

/////////////////////////////////////////////////////////////////////////////////
// Query Messages
/////////////////////////////////////////////////////////////////////////////////

async function execQuery(
  sender: Account,
  contract: ContractInfo,
  msg: QueryMsg,
): Promise<QueryResponse> {
  const { secretjs } = sender;

  const response: QueryResponse = (await secretjs.query.compute.queryContract({
    contractAddress: contract.address,
    codeHash: contract.hash,
    query: msg,
  }));

  if (JSON.stringify(response).includes('parse_err"')) {
    throw new Error(`Query parse_err: ${JSON.stringify(response)}`);
  }
  
  return response;
}

async function queryWhoWon(
  sender: Account,
  contract: ContractInfo,
) {
  const msg: WhoWonMsg = { who_won: {  } };
  
  const response = await execQuery(sender, contract, msg);
  return response;
}

/////////////////////////////////////////////////////////////////////////////////
// Tests
/////////////////////////////////////////////////////////////////////////////////

async function testSanity(
  env: jsEnv,
) {
  const player0 = env.accounts[0];
  const player1 = env.accounts[1];
  const player2 = env.accounts[2];
  const contract = env.contracts[0];

  let p0BalStart = parseInt(await getScrtBalance(player0));
  let p1BalStart = parseInt(await getScrtBalance(player1));

  // players join
  let txP0Join = await execJoin(player0, contract, "name0", 123, 1_000_000);
  assert(txP0Join.code === 0);

  let txP1Join = await execJoin(player1, contract, "name1", 321, 1_000_000);
  assert(txP1Join.code === 0);

  // 3rd player cannot join
  let tx = await execJoin(player2, contract, "name2", 111, 1_000_000);
  assert(tx.code !== 0 && tx.rawLog.includes('The game is full'));

  // Roll dice
  let txP0Roll = await execRollDice(player0, contract);
  assert(txP0Roll.code === 0);

  await new Promise(f => setTimeout(f, 6000));

  let qRes = await queryWhoWon(player0, contract);
  assert((
    ((qRes.name === "name0" && qRes.addr === player0.address) || (qRes.name === "name1" && qRes.addr === player1.address))
    && (qRes.dice_roll >= 0 || qRes.dice_roll <= 6)
  ));

  let p0BalEnd = parseInt(await getScrtBalance(player0));
  let p1BalEnd = parseInt(await getScrtBalance(player1));

  if (qRes.addr === player0.address) {
    assert(p0BalEnd === p0BalStart + 1_000_000 - gasLimit * 2 * 0.1);
    assert(p1BalEnd === p1BalStart - 1_000_000 - gasLimit * 0.1);
  } else if (qRes.addr === player1.address) {
    assert(p0BalEnd === p0BalStart - 1_000_000 - gasLimit * 2 * 0.1);
    assert(p1BalEnd === p1BalStart + 1_000_000 - gasLimit * 0.1);
  } else {
    throw Error("no winner")
  }

  // cannot leave after game is over
  tx = await execLeave(player0, contract);
  assert(tx.code !== 0);
  tx = await execLeave(player1, contract);
  assert(tx.code !== 0);

  // cannot roll dice again
  tx = await execRollDice(player0, contract);
  assert(tx.code !== 0);
  tx = await execRollDice(player1, contract);
  assert(tx.code !== 0);

  // cannot join once game is over
  tx = await execJoin(player0, contract, "test", 1024, 1_000_000);
  assert(tx.code !== 0 && tx.rawLog.includes('The game is already over'));

  // check contract has 0 scrt balance
  const contractBal = await getScrtBalance(player0, contract.address);
  assert(contractBal === "0");
}

async function testMustDeposit(
  env: jsEnv,
) {
  const player0 = env.accounts[0];
  const player1 = env.accounts[1];
  const contract = env.contracts[0];

  let tx = await execJoin(player0, contract, "name0", 123, 0);
  assert(tx.code !== 0);
  tx = await execJoin(player0, contract, "name0", 123, 500);
  assert(tx.code !== 0);
  tx = await execJoin(player0, contract, "name0", 123, 5_000_000);
  assert(tx.code !== 0);

  // player0 deposits correctly
  tx = await execJoin(player0, contract, "name0", 123, 1_000_000);
  assert(tx.code === 0);

  tx = await execJoin(player1, contract, "name0", 123, 0);
  assert(tx.code !== 0);
  tx = await execJoin(player1, contract, "name0", 123, 500);
  assert(tx.code !== 0);
  tx = await execJoin(player1, contract, "name0", 123, 5_000_000);
  assert(tx.code !== 0);

  const contractBal = await getScrtBalance(player0, contract.address);
  assert(contractBal === "1000000");
}

async function testLeave(
  env: jsEnv,
) {
  const player0 = env.accounts[0];
  const player1 = env.accounts[1];
  const contract = env.contracts[0];

  let p0BalStart = parseInt(await getScrtBalance(player0));

  // player0 joins then leave
  let tx = await execJoin(player0, contract, "name0", 123, 1_000_000);
  assert(tx.code === 0);
  tx = await execLeave(player0, contract);
  assert(tx.code === 0);
  assert(parseInt(await getScrtBalance(player0)) === p0BalStart - gasLimit * 2 * 0.1);
  
  // player0 joins...
  tx = await execJoin(player0, contract, "name00", 1234, 1_000_000);
  assert(tx.code === 0);

  // cannot roll dice yet
  tx = await execRollDice(player0, contract);
  assert(tx.code !== 0);
  
  // ...then player1 joins, but cannot leave! Hotel California
  tx = await execJoin(player1, contract, "name1", 1234, 1_000_000);
  assert(tx.code === 0);
  tx = await execLeave(player1, contract);
  assert(tx.code !== 0);

  // contract has 2 scrt 
  const contractBal = await getScrtBalance(player0, contract.address);
  assert(contractBal === "2000000");

}

async function testP1CanJoinTwice(
  env: jsEnv
) {
  const player0 = env.accounts[0];
  const contract = env.contracts[0];

  let tx = await execJoin(player0, contract, "name0", 123, 1_000_000);
  assert(tx.code === 0);
  tx = await execJoin(player0, contract, "name0", 123, 1_000_000);
  assert(tx.code === 0);
}

/////////////////////////////////////////////////////////////////////////////////
// Main
/////////////////////////////////////////////////////////////////////////////////

async function runTest(
  tester: (
    env: jsEnv,
  ) => void,
  env: jsEnv
) {
  console.log(`[TESTING...]: ${tester.name}`);
  await tester(env);
  console.log(`[SUCCESS] ${tester.name}`);
}

(async () => {
  let env: jsEnv;

  env = await initDefault();
  await runTest(testSanity, env);

  env = await initDefault();
  await runTest(testMustDeposit, env);

  env = await initDefault();
  await runTest(testLeave, env);

  env = await initDefault();
  await runTest(testP1CanJoinTwice, env);

  console.log("All tests COMPLETED SUCCESSFULLY");

})();
