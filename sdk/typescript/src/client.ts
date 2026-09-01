/**
 * Read-only client and transaction-preparation helpers for the ProofOwl
 * contract.
 *
 * Design constraints (enforced here, not just documented):
 *  - The read client is constructed with NO `publicKey` and NO signer,
 *    so it can only simulate. It cannot sign or submit.
 *  - The `prepare*` helpers return an UNSIGNED `AssembledTransaction`.
 *    This SDK never signs, never submits, and never touches a keystore.
 *  - Two-party calls (`link_github`, `unlink_github`) are explicit: the
 *    helper returns the transaction AND the list of addresses that must
 *    still sign an auth entry, and refuses to pretend one signature is
 *    enough.
 *
 * See `docs/integration/contract-api-v1.md`.
 */

import type { AssembledTransaction } from "@stellar/stellar-sdk/contract";
import {
  Client as GeneratedClient,
  type Attestation as GeneratedAttestation,
} from "./generated/index.js";
import { assertConfig, type ProofOwlContractConfig } from "./config.js";

/** A single attestation, normalised for JS consumers. */
export interface AttestationView {
  /** `"<owner>/<repo>"` as stored on-chain. */
  repo: string;
  /** GitHub pull-request number. */
  prNumber: number;
  /** Stellar Wave issue id, or `0n` if not applicable. */
  issueId: bigint;
  /** One of `0`, `100`, `150`, `200`. */
  complexity: number;
  /** 32-byte canonical PR hash. */
  prHash: Uint8Array;
  /** Same as {@link prHash}, lowercase hex, 64 chars. */
  prHashHex: string;
  /** Ledger close time (Unix seconds) the contract recorded. */
  timestamp: bigint;
}

function toHex(bytes: Uint8Array): string {
  return Buffer.from(bytes).toString("hex");
}

function viewAttestation(a: GeneratedAttestation): AttestationView {
  const prHash = new Uint8Array(a.pr_hash);
  return {
    repo: a.repo,
    prNumber: a.pr_number,
    issueId: BigInt(a.issue_id),
    complexity: a.complexity,
    prHash,
    prHashHex: toHex(prHash),
    timestamp: BigInt(a.timestamp),
  };
}

/** Build the underlying generated client with no signing capability. */
function readOnlyGeneratedClient(config: ProofOwlContractConfig): GeneratedClient {
  assertConfig(config);
  return new GeneratedClient({
    contractId: config.contractId,
    rpcUrl: config.rpcUrl,
    networkPassphrase: config.networkPassphrase,
    allowHttp: config.allowHttp ?? false,
    // Intentionally omitted: publicKey, signTransaction, signAuthEntry.
  });
}

export interface ProofOwlReadClient {
  /** Current admin, or `null` if the instance is uninitialised/archived. */
  getAdmin(): Promise<string | null>;
  /** Current attestor, or `null`. */
  getAttestor(): Promise<string | null>;
  /** Wallet linked to `githubIdHash`, or `null`. */
  getWalletForGithub(githubIdHash: Uint8Array): Promise<string | null>;
  /** `github_id_hash` linked to `wallet` (32 bytes), or `null`. */
  getGithubForWallet(wallet: string): Promise<Uint8Array | null>;
  /** Full attestation history for `wallet`, oldest first. */
  getAttestations(wallet: string): Promise<AttestationView[]>;
  /** Summed reputation score for `wallet`. */
  getReputationScore(wallet: string): Promise<number>;
  /** Escape hatch: the raw generated client (still read-only). */
  readonly raw: GeneratedClient;
}

/**
 * Create a read-only client. Every method simulates against the RPC and
 * returns the decoded value. No signing, no submission, no fees.
 */
export function createReadClient(config: ProofOwlContractConfig): ProofOwlReadClient {
  const raw = readOnlyGeneratedClient(config);
  return {
    raw,
    async getAdmin() {
      return (await raw.get_admin()).result ?? null;
    },
    async getAttestor() {
      return (await raw.get_attestor()).result ?? null;
    },
    async getWalletForGithub(githubIdHash: Uint8Array) {
      assertHash32(githubIdHash, "githubIdHash");
      return (
        (await raw.get_wallet_for_github({ github_id_hash: Buffer.from(githubIdHash) })).result ??
        null
      );
    },
    async getGithubForWallet(wallet: string) {
      const r = (await raw.get_github_for_wallet({ wallet })).result;
      return r ? new Uint8Array(r) : null;
    },
    async getAttestations(wallet: string) {
      const list = (await raw.get_attestations({ wallet })).result;
      return list.map(viewAttestation);
    },
    async getReputationScore(wallet: string) {
      return (await raw.get_reputation_score({ wallet })).result;
    },
  };
}

// --- transaction preparation (unsigned) ---------------------------------

function assertHash32(bytes: Uint8Array, label: string): void {
  if (!(bytes instanceof Uint8Array) || bytes.length !== 32) {
    throw new TypeError(`${label} must be a 32-byte Uint8Array`);
  }
}

function assertGAddress(value: string, label: string): void {
  if (typeof value !== "string" || !/^G[A-Z2-7]{55}$/.test(value)) {
    throw new TypeError(`${label} must be a 'G...' account strkey`);
  }
}

const ALLOWED_COMPLEXITY = [0, 100, 150, 200] as const;
export type ComplexityTier = (typeof ALLOWED_COMPLEXITY)[number];

/**
 * The result of preparing a two-party call. `transaction` is UNSIGNED
 * and cannot be submitted until every address in `needsSignatureFrom`
 * has signed its auth entry (and the invoker has signed the envelope).
 */
export interface TwoPartyPreparedCall {
  transaction: AssembledTransaction<unknown>;
  /** Non-invoker addresses that still need to sign a Soroban auth entry. */
  needsSignatureFrom: string[];
  /** The invoker (transaction source) whose envelope + root auth must be signed. */
  invoker: string;
}

async function assembleTwoParty(
  config: ProofOwlContractConfig,
  invoker: string,
  build: (client: GeneratedClient) => Promise<AssembledTransaction<unknown>>,
): Promise<TwoPartyPreparedCall> {
  assertConfig(config);
  assertGAddress(invoker, "invoker");
  const client = new GeneratedClient({
    contractId: config.contractId,
    rpcUrl: config.rpcUrl,
    networkPassphrase: config.networkPassphrase,
    allowHttp: config.allowHttp ?? false,
    publicKey: invoker,
    // No signer: the transaction comes back unsigned on purpose.
  });
  const transaction = await build(client);
  const needsSignatureFrom = transaction.needsNonInvokerSigningBy();
  if (needsSignatureFrom.length === 0) {
    throw new Error(
      "expected a two-party call to require a second signature; got none. " +
        "This usually means the `attestor` argument equals the invoker, which is not a valid link flow.",
    );
  }
  return { transaction, needsSignatureFrom, invoker };
}

export interface LinkParams {
  /** Contributor wallet, `G...`. Also the transaction invoker. */
  wallet: string;
  /** Trusted attestor, `G...`. Must equal the on-chain attestor. */
  attestor: string;
  /** 32-byte canonical GitHub identity hash. */
  githubIdHash: Uint8Array;
}

/**
 * Prepare an UNSIGNED `link_github` transaction. Two-party: the returned
 * `needsSignatureFrom` will contain the attestor address. The caller
 * must collect the wallet's envelope/root signature AND the attestor's
 * auth-entry signature before submitting. This SDK does neither.
 */
export function prepareLinkGithub(
  config: ProofOwlContractConfig,
  params: LinkParams,
): Promise<TwoPartyPreparedCall> {
  assertGAddress(params.wallet, "wallet");
  assertGAddress(params.attestor, "attestor");
  assertHash32(params.githubIdHash, "githubIdHash");
  return assembleTwoParty(config, params.wallet, (c) =>
    c.link_github({
      wallet: params.wallet,
      attestor: params.attestor,
      github_id_hash: Buffer.from(params.githubIdHash),
    }),
  );
}

/** Prepare an UNSIGNED two-party `unlink_github` transaction. See {@link prepareLinkGithub}. */
export function prepareUnlinkGithub(
  config: ProofOwlContractConfig,
  params: LinkParams,
): Promise<TwoPartyPreparedCall> {
  assertGAddress(params.wallet, "wallet");
  assertGAddress(params.attestor, "attestor");
  assertHash32(params.githubIdHash, "githubIdHash");
  return assembleTwoParty(config, params.wallet, (c) =>
    c.unlink_github({
      wallet: params.wallet,
      attestor: params.attestor,
      github_id_hash: Buffer.from(params.githubIdHash),
    }),
  );
}

export interface SubmitAttestationParams {
  /** Attestor, `G...`. Invoker and only signer. */
  attestor: string;
  githubIdHash: Uint8Array;
  /** `"<owner>/<repo>"`, lowercased. */
  repo: string;
  prNumber: number;
  issueId: bigint | number;
  complexity: ComplexityTier;
  /** 32-byte canonical PR hash. */
  prHash: Uint8Array;
}

/**
 * Prepare an UNSIGNED single-party `submit_attestation` transaction
 * (attestor only). Validates `complexity` locally so the caller does
 * not waste a simulation on an `InvalidComplexity` reject.
 */
export function prepareSubmitAttestation(
  config: ProofOwlContractConfig,
  params: SubmitAttestationParams,
): Promise<AssembledTransaction<unknown>> {
  assertConfig(config);
  assertGAddress(params.attestor, "attestor");
  assertHash32(params.githubIdHash, "githubIdHash");
  assertHash32(params.prHash, "prHash");
  if (typeof params.repo !== "string" || !/^[a-z0-9._-]+\/[a-z0-9._-]+$/.test(params.repo)) {
    throw new TypeError('repo must be "<owner>/<repo>" (lowercase)');
  }
  if (!Number.isInteger(params.prNumber) || params.prNumber < 1 || params.prNumber > 0xffff_ffff) {
    throw new TypeError("prNumber must be a u32 >= 1");
  }
  if (!(ALLOWED_COMPLEXITY as readonly number[]).includes(params.complexity)) {
    throw new RangeError(`complexity must be one of ${ALLOWED_COMPLEXITY.join(", ")}`);
  }
  const client = new GeneratedClient({
    contractId: config.contractId,
    rpcUrl: config.rpcUrl,
    networkPassphrase: config.networkPassphrase,
    allowHttp: config.allowHttp ?? false,
    publicKey: params.attestor,
  });
  return client.submit_attestation({
    attestor: params.attestor,
    github_id_hash: Buffer.from(params.githubIdHash),
    repo: params.repo,
    pr_number: params.prNumber,
    issue_id: BigInt(params.issueId),
    complexity: params.complexity,
    pr_hash: Buffer.from(params.prHash),
  });
}

/**
 * Prepare an UNSIGNED `bump_wallet_ttl` transaction. Permissionless:
 * only the `caller` (transaction source) signs; the wallet owner does
 * not. Changes no data.
 */
export function prepareBumpWalletTtl(
  config: ProofOwlContractConfig,
  caller: string,
  wallet: string,
): Promise<AssembledTransaction<unknown>> {
  assertConfig(config);
  assertGAddress(caller, "caller");
  assertGAddress(wallet, "wallet");
  const client = new GeneratedClient({
    contractId: config.contractId,
    rpcUrl: config.rpcUrl,
    networkPassphrase: config.networkPassphrase,
    allowHttp: config.allowHttp ?? false,
    publicKey: caller,
  });
  return client.bump_wallet_ttl({ wallet });
}

/**
 * Prepare an UNSIGNED single-party `set_attestor` transaction
 * (admin only).
 */
export function prepareSetAttestor(
  config: ProofOwlContractConfig,
  admin: string,
  newAttestor: string,
): Promise<AssembledTransaction<unknown>> {
  assertConfig(config);
  assertGAddress(admin, "admin");
  assertGAddress(newAttestor, "newAttestor");
  const client = new GeneratedClient({
    contractId: config.contractId,
    rpcUrl: config.rpcUrl,
    networkPassphrase: config.networkPassphrase,
    allowHttp: config.allowHttp ?? false,
    publicKey: admin,
  });
  return client.set_attestor({ admin, new_attestor: newAttestor });
}
